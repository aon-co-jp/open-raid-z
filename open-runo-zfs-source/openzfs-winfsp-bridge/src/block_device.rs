//! RAID-Zベクデバイス([`crate::vdev`])が読み書きする単一ディスク相当の抽象化。
//!
//! 実ディスク(VHDXアタッチ後の`\\.\PhysicalDriveN`等)とテスト用の固定サイズ
//! ファイルのどちらも同じ`BlockDevice`トレイトで扱えるようにし、上位のRAID-Z
//! ストライピングロジックがバックエンドの違いを意識しなくて済むようにする。

use crate::error::BridgeResult;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub trait BlockDevice {
    fn read_at(&mut self, offset: u64, len: usize) -> BridgeResult<Vec<u8>>;
    fn write_at(&mut self, offset: u64, data: &[u8]) -> BridgeResult<()>;
}

/// 通常ファイルをバックエンドとする`BlockDevice`実装。
///
/// テスト用の固定サイズファイルはもちろん、VHDXをアタッチして得られる
/// `\\.\PhysicalDriveN`のような生デバイスパスも(管理者権限があれば)同じ
/// 方法で開ける。
pub struct FileBackedDevice {
    file: File,
}

impl FileBackedDevice {
    pub fn open(path: impl AsRef<Path>) -> BridgeResult<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        Ok(Self { file })
    }

    /// テスト用: 指定サイズの新規ファイルを作成して開く。
    pub fn create_fixed_size(path: impl AsRef<Path>, size_bytes: u64) -> BridgeResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        file.set_len(size_bytes)?;
        Ok(Self { file })
    }
}

impl BlockDevice for FileBackedDevice {
    fn read_at(&mut self, offset: u64, len: usize) -> BridgeResult<Vec<u8>> {
        self.file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; len];
        self.file.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn write_at(&mut self, offset: u64, data: &[u8]) -> BridgeResult<()> {
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(data)?;
        Ok(())
    }
}

/// テスト・障害注入用: `failed`がtrueの間は読み書きともにエラーを返す
/// (実ディスク故障のシミュレーション)。
pub struct FaultInjectableDevice<D: BlockDevice> {
    inner: D,
    pub failed: bool,
}

impl<D: BlockDevice> FaultInjectableDevice<D> {
    pub fn new(inner: D) -> Self {
        Self {
            inner,
            failed: false,
        }
    }

    pub fn inner_mut(&mut self) -> &mut D {
        &mut self.inner
    }
}

impl<D: BlockDevice> BlockDevice for FaultInjectableDevice<D> {
    fn read_at(&mut self, offset: u64, len: usize) -> BridgeResult<Vec<u8>> {
        if self.failed {
            return Err(simulated_failure());
        }
        self.inner.read_at(offset, len)
    }

    fn write_at(&mut self, offset: u64, data: &[u8]) -> BridgeResult<()> {
        if self.failed {
            return Err(simulated_failure());
        }
        self.inner.write_at(offset, data)
    }
}

fn simulated_failure() -> crate::error::BridgeError {
    crate::error::BridgeError::Io(std::io::Error::other("simulated device failure"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("openruno_block_device_test_{name}"))
    }

    #[test]
    fn file_backed_device_round_trips_data() {
        let path = scratch_path("round_trip");
        let mut dev = FileBackedDevice::create_fixed_size(&path, 4096).unwrap();

        dev.write_at(128, b"hello raid-z").unwrap();
        let read_back = dev.read_at(128, b"hello raid-z".len()).unwrap();
        assert_eq!(read_back, b"hello raid-z");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn fault_injectable_device_errors_while_failed() {
        let path = scratch_path("fault_inject");
        let inner = FileBackedDevice::create_fixed_size(&path, 4096).unwrap();
        let mut dev = FaultInjectableDevice::new(inner);

        dev.write_at(0, b"ok").unwrap();
        dev.failed = true;
        assert!(dev.read_at(0, 2).is_err());
        assert!(dev.write_at(0, b"no").is_err());

        dev.failed = false;
        assert_eq!(dev.read_at(0, 2).unwrap(), b"ok");

        std::fs::remove_file(&path).ok();
    }
}
