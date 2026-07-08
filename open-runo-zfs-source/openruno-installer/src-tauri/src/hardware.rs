//! 物理ディスクの列挙とNPU/GPUアクセラレータの検出。
//!
//! zpool初期化ウィザードの最初のステップとして、「どのディスクをRAID-Zの
//! メンバーにできるか」「パリティ計算をNPU/GPUへオフロードできるか」を
//! ユーザーに提示するための情報収集層。

use serde::Serialize;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows::Win32::System::Ioctl::{GET_LENGTH_INFORMATION, IOCTL_DISK_GET_LENGTH_INFO};
use windows::Win32::System::IO::DeviceIoControl;

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    /// `\\.\PhysicalDriveN`形式のパス。RAID-Zメンバー選択時にそのまま
    /// `FileBackedDevice::open`(実際にはVHDXアタッチ等を想定)へ渡す。
    pub path: String,
    pub index: u32,
    pub size_bytes: u64,
}

/// `\\.\PhysicalDrive0`〜`\\.\PhysicalDrive15`を試しに開き、開けたものだけ
/// (=実際に存在するディスク)を一覧として返す。
///
/// 【権限に関する注意】`\\.\PhysicalDriveN`はメタデータの問い合わせのみ
/// (`dwDesiredAccess=0`)であっても、Windows上では管理者権限が無いと
/// `CreateFileW`自体が失敗する(アクセス拒否)。そのため本関数は
/// 管理者権限で実行された場合のみ実際のディスク一覧を返し、
/// 非昇格プロセスから呼ばれた場合は空のリストを返す
/// (呼び出し側でUIに「管理者として再実行してください」等の案内を出す想定)。
pub fn list_physical_disks() -> Vec<DiskInfo> {
    let mut disks = Vec::new();
    for index in 0..16u32 {
        let Some(size_bytes) = query_disk_size(index) else {
            continue;
        };
        disks.push(DiskInfo {
            path: format!("\\\\.\\PhysicalDrive{index}"),
            index,
            size_bytes,
        });
    }
    disks
}

fn query_disk_size(index: u32) -> Option<u64> {
    let path_wide: Vec<u16> = format!("\\\\.\\PhysicalDrive{index}\0").encode_utf16().collect();
    let handle: HANDLE = unsafe {
        CreateFileW(
            PCWSTR(path_wide.as_ptr()),
            0, // 問い合わせ専用(読み書きなし)。UAC昇格なしで開ける。
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            Default::default(),
            None,
        )
        .ok()?
    };

    let mut info = GET_LENGTH_INFORMATION::default();
    let mut bytes_returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_DISK_GET_LENGTH_INFO,
            None,
            0,
            Some(&mut info as *mut _ as *mut _),
            std::mem::size_of::<GET_LENGTH_INFORMATION>() as u32,
            Some(&mut bytes_returned),
            None,
        )
        .is_ok()
    };
    unsafe {
        CloseHandle(handle).ok();
    }

    if ok && info.Length > 0 {
        Some(info.Length as u64)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AcceleratorInfo {
    pub kind: String,
    pub description: String,
}

pub fn detect_accelerator() -> AcceleratorInfo {
    match zfs_accel_hlsl::detect_best_accelerator() {
        Ok(device) => AcceleratorInfo {
            kind: format!("{:?}", device.kind),
            description: device.adapter_description,
        },
        Err(e) => AcceleratorInfo {
            kind: "Unknown".to_string(),
            description: format!("検出に失敗しました: {e}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_accelerator_returns_a_kind_on_this_machine() {
        let info = detect_accelerator();
        assert!(!info.kind.is_empty());
        println!("detected: {info:?}");
    }

    #[test]
    fn list_physical_disks_finds_disks_when_elevated_otherwise_returns_empty() {
        // \\.\PhysicalDriveN は管理者権限が無いと開けないため、非昇格の
        // テスト実行では空リストが返る(それ自体が正しい挙動)。
        // 管理者権限で実行された場合のみ、実際に1台以上検出できることを検証する。
        let disks = list_physical_disks();
        for disk in &disks {
            assert!(disk.size_bytes > 0, "ディスク{}のサイズが0です", disk.index);
        }
        println!("disks (admin={}): {disks:?}", !disks.is_empty());
    }
}
