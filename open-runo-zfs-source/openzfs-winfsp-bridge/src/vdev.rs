//! RAID-Z2/Z3 vdev: 複数の[`BlockDevice`]をストライプ+パリティで束ね、
//! 一定数までのディスク故障からデータを読み出し・復旧できるようにする層。
//!
//! ストライプ設計は単純化のため「1ストライプ内の1チャンクは必ず同じ1台の
//! ディスクへ書く」固定割り当てとする(OpenZFSの実際のRAID-Zも同様の考え方)。
//! パリティ計算は[`zfs_accel_hlsl::raidz23_parity`]に委譲する。
//!
//! ZFSの「全書き込みへのチェックサム付与+読み込み時検証+自己修復」も
//! [`crate::checksum`]を使って実装している。読み込んだチャンクが実際には
//! ディスク上で読めた(エラーにならなかった)場合でも、チェックサムが
//! 記録済みの値と一致しなければ「サイレント破損(ビットロット)」として扱い、
//! パリティから再構築した上で該当ディスクへ書き戻す(自己修復)。

use crate::block_device::BlockDevice;
use crate::checksum::{compute_checksum, Checksum};
use crate::error::{BridgeError, BridgeResult};
use std::collections::HashMap;
use zfs_accel_hlsl::device::AccelDevice;
use zfs_accel_hlsl::galois::GaloisTables;
use zfs_accel_hlsl::raidz23_parity;

/// 対応するRAIDレベル。
///
/// 【Z2/Z3とRaid6/Raid5の関係】RAID6の二重パリティ(P/Q)とRAID-Z2は
/// 数学的に同一(GF(2^8)上のReed-Solomon)であり、`Raid6`はそのまま`Z2`と
/// 同じ`parity_count=2`として扱う(業界標準の呼び方を選びたいユーザ向けの
/// 別名という位置づけ)。RAID5はRAID-Z1相当(単一XORパリティ、
/// `parity_count=1`)。
///
/// 【Raid1(ミラー)の実装】N面ミラーは、実は「データディスク1台+パリティ
/// N-1台」のRAID-Z計算の退化形と数学的に完全に一致する: データディスクが
/// 1台だけの場合、P=D0(そのままXOR)、Q=D0*2^0=D0、R=D0*4^0=D0となり、
/// 全パリティがデータの単純コピーになる(= ミラー)。そのため既存の
/// P/Q/R計算・復旧ロジックをそのまま流用でき、専用実装は不要。
/// `parity_count`はディスク総数に応じて動的に決まる(`devices.len() - 1`)ため、
/// [`RaidZVdev::new`]側で特別扱いする。
///
/// 【Raid0(ストライプのみ)】パリティ無し(`parity_count=0`)。冗長性が
/// 一切無いため、1台でも故障すると復旧不能(通常のRAID0と同じ)。
///
/// 【Raid10は未対応】ストライプ+ミラーの入れ子構成はストライプ単位で
/// 全ディスクへ書く現在のモデルに乗らないため(各ストライプがミラー
/// ペアのうち1組だけを使う構成が必要)、本vdevでは表現できない。
/// 複数の`RaidZVdev`(各々`Raid1`)をラウンドロビンで束ねる別レイヤーが必要。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaidLevel {
    Raid0,
    Raid1,
    Raid5,
    Raid6,
    Z2,
    Z3,
}

impl RaidLevel {
    /// `total_disks`(vdevに参加する全ディスク数)を渡すことで決まる
    /// パリティディスク数。`Raid1`のみディスク総数に依存する
    /// (残り全台をミラーコピーとして扱うため)。
    pub fn parity_count(self, total_disks: usize) -> usize {
        match self {
            RaidLevel::Raid0 => 0,
            RaidLevel::Raid5 => 1,
            RaidLevel::Raid6 | RaidLevel::Z2 => 2,
            RaidLevel::Z3 => 3,
            RaidLevel::Raid1 => total_disks.saturating_sub(1),
        }
    }
}

/// [`RaidZVdev::scrub`]の結果サマリ。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScrubReport {
    pub stripes_scanned: u64,
    pub corruptions_healed: usize,
}

pub struct RaidZVdev<D: BlockDevice> {
    devices: Vec<D>,
    parity_count: usize,
    chunk_size: usize,
    gf: GaloisTables,
    /// (ディスクインデックス, ストライプ番号) -> そのチャンクを書いた時点のチェックサム。
    /// 本物のZFSはこれをブロックポインタ木としてディスク上に永続化するが、
    /// 本層はメモリ上のテーブルとして保持する簡易実装。
    checksums: HashMap<(usize, u64), Checksum>,
    /// 設定されていれば、Z2のP/QパリティをGPU/NPU(D3D12 Compute)へオフロードする
    /// (`zfs_accel_hlsl::raidz23_parity::compute_pq_accelerated`)。
    /// 未設定、またはZ3の場合は常にCPU実装(`compute_pq`/`compute_pqr`)を使う
    /// (RAID-Z3のR用シェーダは未実装のため)。
    accel: Option<AccelDevice>,
}

impl<D: BlockDevice> RaidZVdev<D> {
    pub fn new(devices: Vec<D>, level: RaidLevel, chunk_size: usize) -> Self {
        let parity_count = level.parity_count(devices.len());
        assert!(
            devices.len() > parity_count,
            "データディスクが最低1台は必要です(devices.len() > parity_count)"
        );
        Self {
            devices,
            parity_count,
            chunk_size,
            gf: GaloisTables::new(),
            checksums: HashMap::new(),
            accel: None,
        }
    }

    /// GPU/NPUアクセラレータを設定する(RAID-Z2のP/Q計算のみ対象)。
    pub fn with_accelerator(mut self, accel: AccelDevice) -> Self {
        self.accel = Some(accel);
        self
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
        // ミラー(データディスク1台)の場合、P=Q=R=データそのもの(GF数式が
        // 1台のみへ退化するため)なので、パリティ数に関わらず単純コピーで
        // 済む(3台を超えるミラーコピーにも対応できる一般化)。
        if self.num_data_disks() == 1 {
            return (0..self.parity_count).map(|_| chunks[0].to_vec()).collect();
        }
        match self.parity_count {
            0 => vec![],
            1 => vec![raidz23_parity::compute_p(chunks)],
            2 => {
                let (p, q) = match &self.accel {
                    Some(accel) => raidz23_parity::compute_pq_accelerated(accel, chunks, &self.gf),
                    None => raidz23_parity::compute_pq(chunks, &self.gf),
                };
                vec![p, q]
            }
            3 => {
                let (p, q, r) = raidz23_parity::compute_pqr(chunks, &self.gf);
                vec![p, q, r]
            }
            other => unreachable!(
                "parity_count={other}はミラー(データディスク1台)以外では未対応です"
            ),
        }
    }

    /// 1ストライプ分のデータ(`num_data_disks() * chunk_size`バイト)を書き込む。
    /// 書き込んだ各チャンク(データ・パリティ双方)のチェックサムも記録する。
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
            self.checksums.insert((i, stripe_index), compute_checksum(chunk));
        }
        for (i, par) in parity.iter().enumerate() {
            let disk_idx = num_data + i;
            self.devices[disk_idx].write_at(offset, par)?;
            self.checksums.insert((disk_idx, stripe_index), compute_checksum(par));
        }
        Ok(())
    }

    /// 1ストライプ分のデータを読み出す。読めない、またはチェックサムが
    /// 一致しない(サイレント破損)ディスクが`parity_count`台以内なら、
    /// パリティから自動的に復旧して返す(破損していたディスクへは
    /// 復旧結果を書き戻して自己修復する)。
    pub fn read_stripe(&mut self, stripe_index: u64) -> BridgeResult<Vec<u8>> {
        self.read_stripe_forcing_missing(stripe_index, &[]).map(|(data, _)| data)
    }

    /// [`Self::read_stripe`]と同じだが、実際に検知・修復した破損ディスクの
    /// インデックス一覧も返す([`Self::scrub`]用)。
    pub fn read_stripe_with_report(
        &mut self,
        stripe_index: u64,
    ) -> BridgeResult<(Vec<u8>, Vec<usize>)> {
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
    ///
    /// 戻り値の`Vec<usize>`は、チェックサム不一致(サイレント破損)を検知して
    /// 実際に自己修復(書き戻し)したディスクのインデックス一覧。
    fn read_stripe_forcing_missing(
        &mut self,
        stripe_index: u64,
        force_missing: &[usize],
    ) -> BridgeResult<(Vec<u8>, Vec<usize>)> {
        let num_data = self.num_data_disks();
        let offset = stripe_index * self.chunk_size as u64;
        let chunk_size = self.chunk_size;

        let mut reads: Vec<Option<Vec<u8>>> = self
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

        // チェックサム検証: 読めたチャンクでも、記録済みチェックサムと
        // 一致しなければ「サイレント破損」として欠損扱いに切り替える。
        let mut corrupted: Vec<usize> = Vec::new();
        for (i, read) in reads.iter_mut().enumerate() {
            if let Some(data) = read {
                if let Some(expected) = self.checksums.get(&(i, stripe_index)) {
                    if compute_checksum(data) != *expected {
                        corrupted.push(i);
                        *read = None;
                    }
                }
            }
        }

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
            return Ok((out, Vec::new()));
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
        for (i, d) in &recovered {
            full[*i] = d.clone();
        }

        // 自己修復: チェックサム不一致で欠損扱いにしたディスク(=物理的には
        // まだオンライン)には、正しいデータを書き戻す。
        // force_missingで意図的に欠損扱いにしただけのディスク(resilver対象等)は
        // 対象外。データディスクは復旧結果をそのまま書き戻し、パリティ
        // ディスクは(復旧済みの)全データから改めて計算し直して書き戻す。
        let mut healed: Vec<usize> = Vec::new();
        for (i, data) in &recovered {
            if corrupted.contains(i) {
                if self.devices[*i].write_at(offset, data).is_ok() {
                    self.checksums.insert((*i, stripe_index), compute_checksum(data));
                    healed.push(*i);
                }
            }
        }
        let corrupted_parity: Vec<usize> = corrupted
            .iter()
            .copied()
            .filter(|&i| i >= num_data)
            .collect();
        if !corrupted_parity.is_empty() {
            let full_refs: Vec<&[u8]> = full.iter().map(|c| c.as_slice()).collect();
            let recomputed_parity = self.compute_parity(&full_refs);
            for parity_disk in corrupted_parity {
                let parity_idx = parity_disk - num_data;
                let correct = &recomputed_parity[parity_idx];
                if self.devices[parity_disk].write_at(offset, correct).is_ok() {
                    self.checksums.insert((parity_disk, stripe_index), compute_checksum(correct));
                    healed.push(parity_disk);
                }
            }
        }

        let mut out = Vec::with_capacity(num_data * chunk_size);
        for chunk in full {
            out.extend_from_slice(&chunk);
        }
        Ok((out, healed))
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
            let (data, _) = self.read_stripe_forcing_missing(stripe, &[target_index])?;
            let offset = stripe * chunk_size as u64;

            if target_index < num_data {
                let chunk = &data[target_index * chunk_size..(target_index + 1) * chunk_size];
                self.devices[target_index].write_at(offset, chunk)?;
                self.checksums.insert((target_index, stripe), compute_checksum(chunk));
            } else {
                let chunks: Vec<&[u8]> = data.chunks(chunk_size).collect();
                let parity = self.compute_parity(&chunks);
                let parity_idx = target_index - num_data;
                self.devices[target_index].write_at(offset, &parity[parity_idx])?;
                self.checksums
                    .insert((target_index, stripe), compute_checksum(&parity[parity_idx]));
            }
        }
        Ok(())
    }

    /// scrub: 全ストライプを読み込み、チェックサム不一致(サイレント破損)を
    /// 検知・修復する。ZFSの`zpool scrub`に相当する。
    pub fn scrub(&mut self, num_stripes: u64) -> BridgeResult<ScrubReport> {
        let mut report = ScrubReport::default();
        for stripe in 0..num_stripes {
            let (_, healed) = self.read_stripe_with_report(stripe)?;
            report.stripes_scanned += 1;
            report.corruptions_healed += healed.len();
        }
        Ok(report)
    }
}

#[cfg(test)]
mod accel_tests {
    use super::*;
    use crate::block_device::FileBackedDevice;

    fn scratch_disk(name: &str) -> FileBackedDevice {
        let path = std::env::temp_dir().join(format!("openruno_vdev_accel_test_{name}"));
        FileBackedDevice::create_fixed_size(&path, 4096).unwrap()
    }

    /// GPU/NPUが実際に検出できる環境でのみ実行する。RaidZVdevへ
    /// `with_accelerator`でアクセラレータを設定した場合でも、書き込んだ
    /// データがCPU実装と同じように正しく読み出せる(=実際にディスパッチされ、
    /// かつ計算結果がCPU参照実装と一致する)ことを、pool.rs/vdev.rsの
    /// 実際の書き込み・読み出しパス経由で検証する。
    #[test]
    fn raidz2_write_read_round_trips_with_gpu_accelerator_when_available() {
        let accel = match zfs_accel_hlsl::device::detect_best_accelerator() {
            Ok(a) if a.kind != zfs_accel_hlsl::device::AccelKind::CpuFallback => a,
            _ => {
                eprintln!("GPU/NPUが見つからないためテストをスキップします");
                return;
            }
        };

        let devices = vec![
            scratch_disk("d0"),
            scratch_disk("d1"),
            scratch_disk("d2"),
            scratch_disk("p"),
            scratch_disk("q"),
        ];
        let mut vdev = RaidZVdev::new(devices, RaidLevel::Z2, 4).with_accelerator(accel);

        let data = vec![0xAAu8, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        vdev.write_stripe(0, &data).unwrap();
        let read_back = vdev.read_stripe(0).unwrap();
        assert_eq!(read_back, data);
    }
}
