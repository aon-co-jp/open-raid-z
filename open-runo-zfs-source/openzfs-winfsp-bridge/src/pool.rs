//! ZFSのストレージプール(複数ディスクを1つの記憶領域にまとめ、その中から
//! 必要な容量を切り出して複数のファイルシステム(データセット)を作成できる)
//! を模した層。
//!
//! [`crate::vdev::RaidZVdev`]が提供する総ストライプ数を「プールの容量」として
//! 扱い、その中から複数の名前付きデータセットへストライプ単位で動的に
//! 容量を割り当てる。個々のディスクの容量に縛られず、プール全体の空き容量の
//! 範囲で複数のファイルシステムを柔軟に運用できる、というZFSの特徴を模している。
//!
//! 【現状の制約】
//! - 1プール = 1vdevのみ対応(複数vdevをまたぐプールの容量拡張は未実装)
//! - ストライプ境界単位での粗い割り当て(ZFSのメタスラブ/SPAほど細かい
//!   バイト単位のアロケータではない)
//!
//! ## コピーオンライト(CoW)
//!
//! `Dataset.stripes`は「論理ストライプ番号 -> 物理ストライプ番号」の間接参照
//! テーブルであり、これがそのままCoWの実装基盤になる。[`Pool::write`]は
//! 既存の物理ストライプを上書きするのではなく、
//!
//! 1. 新しいデータを**空き**物理ストライプへ書き込み、
//! 2. 書き込みが成功して初めて、論理ストライプ番号が指す物理ストライプ番号を
//!    新しい方へ差し替え、
//! 3. (スナップショット等の他参照が無ければ)古い物理ストライプを空き領域へ
//!    返却する
//!
//! という順序で行う。書き込み中に電源断等でクラッシュしても、ステップ2の
//! 参照切り替えが完了するまでは古いデータが指されたままなので、
//! データが破壊されることはない(ZFSのCoWと同じ考え方)。

use crate::block_device::BlockDevice;
use crate::error::{BridgeError, BridgeResult};
use crate::vdev::RaidZVdev;
use std::collections::HashMap;

pub struct Pool<D: BlockDevice> {
    vdev: RaidZVdev<D>,
    total_stripes: u64,
    /// 空きストライプのインデックス集合(スタックとして扱う: popで払い出す)
    free_stripes: Vec<u64>,
    datasets: HashMap<String, Dataset>,
}

#[derive(Debug, Clone, Default)]
struct Dataset {
    /// このデータセットが保持する物理ストライプのインデックス列(論理順)
    stripes: Vec<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolUsage {
    pub total_stripes: u64,
    pub used_stripes: u64,
    pub free_stripes: u64,
}

fn not_found(name: &str) -> BridgeError {
    BridgeError::Io(std::io::Error::other(format!(
        "データセット'{name}'が見つかりません"
    )))
}

impl<D: BlockDevice> Pool<D> {
    /// `total_stripes`個のストライプ(=`vdev`の総容量)を持つプールを作成する。
    pub fn new(vdev: RaidZVdev<D>, total_stripes: u64) -> Self {
        let free_stripes = (0..total_stripes).rev().collect();
        Self {
            vdev,
            total_stripes,
            free_stripes,
            datasets: HashMap::new(),
        }
    }

    pub fn usage(&self) -> PoolUsage {
        PoolUsage {
            total_stripes: self.total_stripes,
            used_stripes: self.total_stripes - self.free_stripes.len() as u64,
            free_stripes: self.free_stripes.len() as u64,
        }
    }

    fn chunk_bytes(&self) -> u64 {
        (self.vdev.num_data_disks() * self.vdev.chunk_size()) as u64
    }

    pub fn create_dataset(&mut self, name: &str) -> BridgeResult<()> {
        if self.datasets.contains_key(name) {
            return Err(BridgeError::Io(std::io::Error::other(format!(
                "データセット'{name}'は既に存在します"
            ))));
        }
        self.datasets.insert(name.to_string(), Dataset::default());
        Ok(())
    }

    /// データセットを破棄し、割り当てていたストライプをプールへ返却する。
    pub fn destroy_dataset(&mut self, name: &str) -> BridgeResult<()> {
        let dataset = self.datasets.remove(name).ok_or_else(|| not_found(name))?;
        self.free_stripes.extend(dataset.stripes);
        Ok(())
    }

    pub fn dataset_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.datasets.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn dataset_size(&self, name: &str) -> BridgeResult<u64> {
        let ds = self.datasets.get(name).ok_or_else(|| not_found(name))?;
        Ok(ds.stripes.len() as u64 * self.chunk_bytes())
    }

    /// データセットの割当容量を`additional_bytes`ぶん拡張する
    /// (プールに十分な空きがある場合のみ)。ZFSの「必要な分だけ動的に
    /// 確保する」プール容量管理を模す。
    pub fn grow_dataset(&mut self, name: &str, additional_bytes: u64) -> BridgeResult<()> {
        if !self.datasets.contains_key(name) {
            return Err(not_found(name));
        }
        let chunk_bytes = self.chunk_bytes();
        let additional_stripes = additional_bytes.div_ceil(chunk_bytes);

        if additional_stripes > self.free_stripes.len() as u64 {
            return Err(BridgeError::Io(std::io::Error::other(format!(
                "プールの空き容量が不足しています(必要{additional_stripes}ストライプ、空き{}ストライプ)",
                self.free_stripes.len()
            ))));
        }

        let mut newly_allocated = Vec::with_capacity(additional_stripes as usize);
        for _ in 0..additional_stripes {
            newly_allocated.push(self.free_stripes.pop().unwrap());
        }

        self.datasets.get_mut(name).unwrap().stripes.extend(newly_allocated);
        Ok(())
    }

    /// データセットへストライプ境界単位でコピーオンライト書き込みを行う。
    ///
    /// 論理ストライプ位置ごとに新しい空き物理ストライプへデータを書き、
    /// 成功した場合のみそのストライプの参照先を新しい物理ストライプへ
    /// 切り替える。既存の物理ストライプの中身は(参照が切り替わるまで)
    /// 一切変更しない。
    pub fn write(&mut self, name: &str, logical_offset: u64, data: &[u8]) -> BridgeResult<()> {
        let chunk_bytes = self.chunk_bytes();
        assert_eq!(
            logical_offset % chunk_bytes,
            0,
            "現状の実装ではストライプ境界への書き込みのみサポート"
        );
        assert_eq!(
            data.len() as u64 % chunk_bytes,
            0,
            "書き込みサイズはストライプ境界の倍数である必要があります"
        );

        let start = (logical_offset / chunk_bytes) as usize;
        let count = (data.len() as u64 / chunk_bytes) as usize;

        {
            let ds = self.datasets.get(name).ok_or_else(|| not_found(name))?;
            if start + count > ds.stripes.len() {
                return Err(BridgeError::Io(std::io::Error::other(format!(
                    "データセット'{name}'の割当容量を超える書き込みです(grow_datasetが必要)"
                ))));
            }
        }

        for i in 0..count {
            let logical_idx = start + i;
            let chunk_start = i * chunk_bytes as usize;
            let chunk = &data[chunk_start..chunk_start + chunk_bytes as usize];

            // 1. まず空き物理ストライプへ新データを書く(既存データには一切触れない)
            let new_phys = self.free_stripes.pop().ok_or_else(|| {
                BridgeError::Io(std::io::Error::other(
                    "CoW書き込み用の空きストライプがプールにありません",
                ))
            })?;
            if let Err(e) = self.vdev.write_stripe(new_phys, chunk) {
                // 書き込みに失敗した場合は確保したストライプを空きに戻し、
                // 参照テーブルには一切触れない(=古いデータは無傷のまま)
                self.free_stripes.push(new_phys);
                return Err(e);
            }

            // 2. 書き込み成功後にのみ、参照(論理->物理)を新ストライプへ切り替える
            let old_phys = {
                let ds = self.datasets.get_mut(name).unwrap();
                let old = ds.stripes[logical_idx];
                ds.stripes[logical_idx] = new_phys;
                old
            };

            // 3. 古い物理ストライプは(他に参照者が無いため)空き領域へ返却する。
            //    スナップショットを実装する際は、ここで「他に参照しているか」を
            //    確認してから返却するかどうかを判断する形へ拡張する。
            self.free_stripes.push(old_phys);
        }
        Ok(())
    }

    /// 物理ストライプ番号を直接指定して読み出す(CoWの検証・デバッグ用。
    /// 通常のデータアクセスは`read`/`write`をデータセット経由で使うこと)。
    pub fn read_physical_stripe(&mut self, physical_stripe: u64) -> BridgeResult<Vec<u8>> {
        self.vdev.read_stripe(physical_stripe)
    }

    /// データセットの論理ストライプ番号が指す物理ストライプ番号を返す
    /// (CoWの検証・デバッグ用)。
    pub fn physical_stripe_for(&self, name: &str, logical_stripe: u64) -> BridgeResult<u64> {
        let ds = self.datasets.get(name).ok_or_else(|| not_found(name))?;
        ds.stripes
            .get(logical_stripe as usize)
            .copied()
            .ok_or_else(|| {
                BridgeError::Io(std::io::Error::other(format!(
                    "データセット'{name}'の論理ストライプ{logical_stripe}は未割当です"
                )))
            })
    }

    /// データセットからストライプ境界単位で読み込む。
    pub fn read(&mut self, name: &str, logical_offset: u64, len: u64) -> BridgeResult<Vec<u8>> {
        let chunk_bytes = self.chunk_bytes();
        assert_eq!(
            logical_offset % chunk_bytes,
            0,
            "現状の実装ではストライプ境界への読み込みのみサポート"
        );
        assert_eq!(
            len % chunk_bytes,
            0,
            "読み込みサイズはストライプ境界の倍数である必要があります"
        );

        let start = (logical_offset / chunk_bytes) as usize;
        let count = (len / chunk_bytes) as usize;

        let physical_stripes: Vec<u64> = {
            let ds = self.datasets.get(name).ok_or_else(|| not_found(name))?;
            if start + count > ds.stripes.len() {
                return Err(BridgeError::Io(std::io::Error::other(format!(
                    "データセット'{name}'の割当容量を超える読み込みです"
                ))));
            }
            ds.stripes[start..start + count].to_vec()
        };

        let mut out = Vec::with_capacity(len as usize);
        for phys_stripe in physical_stripes {
            out.extend_from_slice(&self.vdev.read_stripe(phys_stripe)?);
        }
        Ok(out)
    }
}
