//! RAID-Zベクデバイス([`crate::vdev`])が読み書きする単一ディスク相当の抽象化。
//!
//! 実ディスク(VHDXアタッチ後の`\\.\PhysicalDriveN`等)とテスト用の固定サイズ
//! ファイルのどちらも同じ`BlockDevice`トレイトで扱えるようにし、上位のRAID-Z
//! ストライピングロジックがバックエンドの違いを意識しなくて済むようにする。

use crate::error::{BridgeError, BridgeResult};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// 指定パス(通常ファイル、または生ブロックデバイス)の実容量をバイト単位で返す。
///
/// 通常ファイル(テスト用のfile-backed device)は`std::fs::metadata`で得られる
/// ファイルサイズをそのまま使う。生ブロックデバイス(Linuxの`/dev/sdX`、
/// Windowsの`\\.\PhysicalDriveN`等)は`metadata`ではサイズが取れない
/// (Linuxではstat結果のst_sizeが0、Windowsではそもそもファイルとして
/// 開けない)ため、OSごとの方法で問い合わせる。
pub fn device_size_bytes(path: impl AsRef<Path>) -> BridgeResult<u64> {
    let path = path.as_ref();
    let metadata = std::fs::metadata(path)?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    platform_device_size_bytes(path)
}

/// Linux: `/sys/class/block/<dev>/size`(512バイトセクタ単位)を読む。
/// `ioctl(BLKGETSIZE64)`と異なり追加の依存クレートを要さず`std::fs`のみで
/// 完結するため、こちらを優先する。
#[cfg(target_os = "linux")]
fn platform_device_size_bytes(path: &Path) -> BridgeResult<u64> {
    let real_path = std::fs::canonicalize(path)?;
    let dev_name = real_path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
        BridgeError::InvalidConfig(format!("デバイス名を取得できません: '{}'", real_path.display()))
    })?;

    let sysfs_path = format!("/sys/class/block/{dev_name}/size");
    let sectors_raw = std::fs::read_to_string(&sysfs_path).map_err(|e| {
        BridgeError::InvalidConfig(format!(
            "'{}'の容量取得に失敗しました('{sysfs_path}'が読めません、通常ファイルでもブロック\
            デバイスでもない可能性があります): {e}",
            path.display()
        ))
    })?;
    let sectors: u64 = sectors_raw.trim().parse().map_err(|_| {
        BridgeError::InvalidConfig(format!(
            "'{sysfs_path}'の内容が不正です(セクタ数として解釈できません): '{}'",
            sectors_raw.trim()
        ))
    })?;
    // sysfsの`size`は常に512バイトセクタ単位(実ブロックサイズがそれと
    // 異なる4Kn等のディスクでも同様)。
    Ok(sectors * 512)
}

/// Windows: `DeviceIoControl`の`IOCTL_DISK_GET_LENGTH_INFO`で問い合わせる。
#[cfg(target_os = "windows")]
fn platform_device_size_bytes(path: &Path) -> BridgeResult<u64> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Ioctl::{GET_LENGTH_INFORMATION, IOCTL_DISK_GET_LENGTH_INFO};
    use windows::Win32::System::IO::DeviceIoControl;

    let file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|e| BridgeError::InvalidConfig(format!("'{}'を開けませんでした: {e}", path.display())))?;
    let handle = HANDLE(file.as_raw_handle());

    let mut info = GET_LENGTH_INFORMATION::default();
    let mut bytes_returned: u32 = 0;
    unsafe {
        DeviceIoControl(
            handle,
            IOCTL_DISK_GET_LENGTH_INFO,
            None,
            0,
            Some(&mut info as *mut _ as *mut core::ffi::c_void),
            std::mem::size_of::<GET_LENGTH_INFORMATION>() as u32,
            Some(&mut bytes_returned),
            None,
        )
        .map_err(|e| {
            BridgeError::InvalidConfig(format!(
                "'{}'の容量取得(IOCTL_DISK_GET_LENGTH_INFO)に失敗しました: {e}",
                path.display()
            ))
        })?;
    }
    Ok(info.Length as u64)
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn platform_device_size_bytes(_path: &Path) -> BridgeResult<u64> {
    Err(BridgeError::NotImplemented(
        "このOSでは生ブロックデバイスの容量自動検出に対応していません(通常ファイルのみ利用可能)",
    ))
}

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
        std::env::temp_dir().join(format!("open_runo_block_device_test_{name}"))
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

    #[test]
    fn device_size_bytes_returns_regular_file_length() {
        let path = scratch_path("size_regular_file");
        FileBackedDevice::create_fixed_size(&path, 12345).unwrap();

        assert_eq!(device_size_bytes(&path).unwrap(), 12345);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn device_size_bytes_fails_cleanly_for_missing_path() {
        let path = scratch_path("size_does_not_exist");
        std::fs::remove_file(&path).ok();

        assert!(device_size_bytes(&path).is_err());
    }
}
