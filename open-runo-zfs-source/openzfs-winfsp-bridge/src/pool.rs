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

    /// データセットへストライプ境界単位で書き込む。
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

        let physical_stripes: Vec<u64> = {
            let ds = self.datasets.get(name).ok_or_else(|| not_found(name))?;
            if start + count > ds.stripes.len() {
                return Err(BridgeError::Io(std::io::Error::other(format!(
                    "データセット'{name}'の割当容量を超える書き込みです(grow_datasetが必要)"
                ))));
            }
            ds.stripes[start..start + count].to_vec()
        };

        for (i, &phys_stripe) in physical_stripes.iter().enumerate() {
            let chunk_start = i * chunk_bytes as usize;
            let chunk = &data[chunk_start..chunk_start + chunk_bytes as usize];
            self.vdev.write_stripe(phys_stripe, chunk)?;
        }
        Ok(())
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
