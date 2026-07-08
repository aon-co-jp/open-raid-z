//! RAID-Z2/Z3 vdev: 複数の[`BlockDevice`]をストライプ+パリティで束ね、
//! 一定数までのディスク故障からデータを読み出し・復旧できるようにする層。
//!
//! ストライプ設計は単純化のため「1ストライプ内の1チャンクは必ず同じ1台の
//! ディスクへ書く」固定割り当てとする(OpenZFSの実際のRAID-Zも同様の考え方)。
//! パリティ計算は[`zfs_accel_hlsl::raidz23_parity`]に委譲する。

use crate::block_device::BlockDevice;
use crate::error::{BridgeError, BridgeResult};
use zfs_accel_hlsl::galois::GaloisTables;
use zfs_accel_hlsl::raidz23_parity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaidLevel {
    Z2,
    Z3,
}

impl RaidLevel {
    fn parity_count(self) -> usize {
        match self {
            RaidLevel::Z2 => 2,
            RaidLevel::Z3 => 3,
        }
    }
}

pub struct RaidZVdev<D: BlockDevice> {
    devices: Vec<D>,
    parity_count: usize,
    chunk_size: usize,
    gf: GaloisTables,
}

impl<D: BlockDevice> RaidZVdev<D> {
    pub fn new(devices: Vec<D>, level: RaidLevel, chunk_size: usize) -> Self {
        let parity_count = level.parity_count();
        assert!(
            devices.len() > parity_count,
            "データディスクが最低1台は必要です(devices.len() > parity_count)"
        );
        Self {
            devices,
            parity_count,
            chunk_size,
            gf: GaloisTables::new(),
        }
    }

    pub fn num_data_disks(&self) -> usize {
        self.devices.len() - self.parity_count
    }

    pub fn num_total_disks(&self) -> usize {
        self.devices.len()
    }

    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    pub fn devices_mut(&mut self) -> &mut [D] {
        &mut self.devices
    }

    fn compute_parity(&self, chunks: &[&[u8]]) -> Vec<Vec<u8>> {
        if self.parity_count == 2 {
            let (p, q) = raidz23_parity::compute_pq(chunks, &self.gf);
            vec![p, q]
        } else {
            let (p, q, r) = raidz23_parity::compute_pqr(chunks, &self.gf);
            vec![p, q, r]
        }
    }

    /// 1ストライプ分のデータ(`num_data_disks() * chunk_size`バイト)を書き込む。
    pub fn write_stripe(&mut self, stripe_index: u64, data: &[u8]) -> BridgeResult<()> {
        let num_data = self.num_data_disks();
        assert_eq!(
            data.len(),
            num_data * self.chunk_size,
            "書き込みデータ長がストライプ幅と一致しません"
        );

        let chunks: Vec<&[u8]> = data.chunks(self.chunk_size).collect();
        let parity = self.compute_parity(&chunks);
        let offset = stripe_index * self.chunk_size as u64;

        for (i, chunk) in chunks.iter().enumerate() {
            self.devices[i].write_at(offset, chunk)?;
        }
        for (i, par) in parity.iter().enumerate() {
            self.devices[num_data + i].write_at(offset, par)?;
        }
        Ok(())
    }

    /// 1ストライプ分のデータを読み出す。読めないディスクが`parity_count`台以内
    /// なら、パリティから自動的に復旧して返す。
    pub fn read_stripe(&mut self, stripe_index: u64) -> BridgeResult<Vec<u8>> {
        self.read_stripe_forcing_missing(stripe_index, &[])
    }

    /// `read_stripe`と同様だが、`force_missing`に含まれるディスクは実際に
    /// 読めたかどうかに関わらず「欠損」として扱う。
    ///
    /// これは`resilver`が交換直後(まだ正しいデータが書かれていない)ディスクを、
    /// たまたま読めてしまう(壊れていない・オンラインである)という理由だけで
    /// 誤って信頼しないようにするために必要。実運用でも「交換した新品ディスクは
    /// 読めるが中身は空(信用できない)」という状況は同じであり、この関数は
    /// それを正しくモデル化する。
    fn read_stripe_forcing_missing(
        &mut self,
        stripe_index: u64,
        force_missing: &[usize],
    ) -> BridgeResult<Vec<u8>> {
        let num_data = self.num_data_disks();
        let offset = stripe_index * self.chunk_size as u64;
        let chunk_size = self.chunk_size;

        let reads: Vec<Option<Vec<u8>>> = self
            .devices
            .iter_mut()
            .enumerate()
            .map(|(i, dev)| {
                if force_missing.contains(&i) {
                    None
                } else {
                    dev.read_at(offset, chunk_size).ok()
                }
            })
            .collect();

        let missing: Vec<usize> = reads
            .iter()
            .enumerate()
            .filter(|(_, r)| r.is_none())
            .map(|(i, _)| i)
            .collect();

        if missing.is_empty() {
            let mut out = Vec::with_capacity(num_data * chunk_size);
            for read in reads.iter().take(num_data) {
                out.extend_from_slice(read.as_ref().unwrap());
            }
            return Ok(out);
        }

        if missing.len() > self.parity_count {
            return Err(BridgeError::Io(std::io::Error::other(format!(
                "{}台のディスクが同時に失われ、パリティ{}台では復旧できません",
                missing.len(),
                self.parity_count
            ))));
        }

        let missing_data: Vec<usize> = missing.iter().copied().filter(|&i| i < num_data).collect();

        let known: Vec<(usize, &[u8])> = reads
            .iter()
            .enumerate()
            .filter(|(i, r)| *i < num_data && r.is_some())
            .map(|(i, r)| (i, r.as_ref().unwrap().as_slice()))
            .collect();

        // 生き残っているパリティを(種別: 0=P,1=Q,2=R, データ)のペアとして集める。
        // Pが故障していてもQ・Rが生きていれば復旧できるよう、固定でPを要求せず
        // 「生きている分」だけを渡す(`reconstruct_missing_data_generic`参照)。
        let available_parity: Vec<(u8, &[u8])> = reads[num_data..]
            .iter()
            .enumerate()
            .filter_map(|(i, r)| r.as_deref().map(|d| (i as u8, d)))
            .collect();

        if available_parity.len() < missing_data.len() {
            return Err(BridgeError::Io(std::io::Error::other(
                "生存しているパリティの数が復旧に必要な数を下回っています",
            )));
        }

        let recovered = raidz23_parity::reconstruct_missing_data_generic(
            &known,
            &missing_data,
            &available_parity,
            &self.gf,
        );

        let mut full: Vec<Vec<u8>> = vec![Vec::new(); num_data];
        for (i, d) in known {
            full[i] = d.to_vec();
        }
        for (i, d) in recovered {
            full[i] = d;
        }

        let mut out = Vec::with_capacity(num_data * chunk_size);
        for chunk in full {
            out.extend_from_slice(&chunk);
        }
        Ok(out)
    }

    /// resilver(自動復旧): `target_index`のディスクを、他ディスク+パリティ
    /// から再構築した内容で`num_stripes`分すべて上書きする。
    /// データディスク・パリティディスクのどちらでも動作する。
    pub fn resilver(&mut self, target_index: usize, num_stripes: u64) -> BridgeResult<()> {
        let num_data = self.num_data_disks();
        let chunk_size = self.chunk_size;

        for stripe in 0..num_stripes {
            // target_indexは(たとえ現在読めても)常に「欠損」として扱い、
            // 交換直後ディスクの古い/空の中身を誤って信頼しないようにする。
            let data = self.read_stripe_forcing_missing(stripe, &[target_index])?;
            let offset = stripe * chunk_size as u64;

            if target_index < num_data {
                let chunk = &data[target_index * chunk_size..(target_index + 1) * chunk_size];
                self.devices[target_index].write_at(offset, chunk)?;
            } else {
                let chunks: Vec<&[u8]> = data.chunks(chunk_size).collect();
                let parity = self.compute_parity(&chunks);
                let parity_idx = target_index - num_data;
                self.devices[target_index].write_at(offset, &parity[parity_idx])?;
            }
        }
        Ok(())
    }
}
