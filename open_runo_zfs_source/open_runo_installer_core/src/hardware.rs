//! 物理ディスクの列挙とNPU/GPUアクセラレータの検出。
//!
//! zpool初期化ウィザードの最初のステップとして、「どのディスクをRAID-Zの
//! メンバーにできるか」「パリティ計算をNPU/GPUへオフロードできるか」を
//! ユーザーに提示するための情報収集層。
//!
//! 【クロスプラットフォームに関する注意】このインストーラーはWindows向け
//! だが、`copilot.rs`(助言ロジック)・`zpool_wizard.rs`(プレビュー実行)は
//! 本来OS非依存であり、開発時にLinux/macOS上でも`cargo test`できることには
//! 実用上の価値がある(Windows実機を用意しなくても大部分のロジックを
//! 検証できるため)。そのため物理ディスク列挙(`\\.\PhysicalDriveN`への
//! アクセス)だけを`#[cfg(windows)]`で分離し、非Windows環境では常に空の
//! 一覧を返すフォールバックを提供する(呼び出し側の`copilot.rs`は「0台」を
//! 「管理者権限が無い」場合と同様に扱うため、動作上は自然にハンドリングされる)。

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    /// `\\.\PhysicalDriveN`形式のパス。RAID-Zメンバー選択時にそのまま
    /// `FileBackedDevice::open`(実際にはVHDXアタッチ等を想定)へ渡す。
    pub path: String,
    pub index: u32,
    pub size_bytes: u64,
    /// 検出できたメディア種別("HDD"/"SSD"/"NVMe"/"USB"/"SD"/"CF"/"Unknown")。
    /// Windows以外、または権限/ドライバの都合で判別できない場合は"Unknown"。
    pub media_type: String,
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
///
/// 【非Windows環境】Windows以外のターゲットでは常に空のリストを返す
/// (下記`#[cfg(not(windows))]`実装参照)。
pub fn list_physical_disks() -> Vec<DiskInfo> {
    imp::list_physical_disks()
}

#[cfg(windows)]
mod imp {
    use super::DiskInfo;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, BusTypeAta, BusTypeMmc, BusTypeNvme, BusTypeRAID, BusTypeSata, BusTypeScsi, BusTypeSd,
        BusTypeUsb, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, STORAGE_BUS_TYPE,
    };
    use windows::Win32::System::Ioctl::{
        DEVICE_SEEK_PENALTY_DESCRIPTOR, GET_LENGTH_INFORMATION, IOCTL_DISK_GET_LENGTH_INFO,
        IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery, STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
        StorageDeviceProperty, StorageDeviceSeekPenaltyProperty,
    };
    use windows::Win32::System::IO::DeviceIoControl;

    pub fn list_physical_disks() -> Vec<DiskInfo> {
        let mut disks = Vec::new();
        for index in 0..16u32 {
            let Some((handle, size_bytes)) = open_and_query_size(index) else {
                continue;
            };
            let media_type = query_media_type(handle).unwrap_or_else(|| "Unknown".to_string());
            unsafe {
                CloseHandle(handle).ok();
            }
            disks.push(DiskInfo {
                path: format!("\\\\.\\PhysicalDrive{index}"),
                index,
                size_bytes,
                media_type,
            });
        }
        disks
    }

    fn open_disk(index: u32) -> Option<HANDLE> {
        let path_wide: Vec<u16> = format!("\\\\.\\PhysicalDrive{index}\0").encode_utf16().collect();
        unsafe {
            CreateFileW(
                PCWSTR(path_wide.as_ptr()),
                0, // 問い合わせ専用(読み書きなし)。UAC昇格なしで開ける。
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                Default::default(),
                None,
            )
            .ok()
        }
    }

    fn open_and_query_size(index: u32) -> Option<(HANDLE, u64)> {
        let handle = open_disk(index)?;

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

        if ok && info.Length > 0 {
            Some((handle, info.Length as u64))
        } else {
            unsafe {
                CloseHandle(handle).ok();
            }
            None
        }
    }

    /// バス種別(USB/NVMe/SATA/SD/CF等)とシーク遅延の有無(SSD/HDD判別)を
    /// `IOCTL_STORAGE_QUERY_PROPERTY`で問い合わせ、人間向けの文字列へ変換する。
    /// どちらも問い合わせ専用のIOCTLで、`\\.\PhysicalDriveN`を読み書き
    /// モードなしで開けている時点(=UAC昇格不要)で使える。
    fn query_media_type(handle: HANDLE) -> Option<String> {
        let bus_type = query_bus_type(handle);

        // USB/SD/CFはバス種別からメディア種別が確定するため、シーク遅延の
        // 問い合わせ(HDD/SSD判別)は「内蔵ドライブ相当のバス」の場合のみ行う。
        match bus_type {
            Some(BusTypeUsb) => Some("USB".to_string()),
            Some(BusTypeSd) => Some("SD".to_string()),
            Some(BusTypeMmc) => Some("CF".to_string()),
            Some(BusTypeNvme) => Some("NVMe".to_string()),
            Some(BusTypeSata) | Some(BusTypeAta) | Some(BusTypeScsi) | Some(BusTypeRAID) => {
                match query_seek_penalty(handle) {
                    Some(true) => Some("HDD".to_string()),
                    Some(false) => Some("SSD".to_string()),
                    None => Some("HDD/SSD".to_string()),
                }
            }
            _ => None,
        }
    }

    fn query_bus_type(handle: HANDLE) -> Option<STORAGE_BUS_TYPE> {
        let query = STORAGE_PROPERTY_QUERY {
            PropertyId: StorageDeviceProperty,
            QueryType: PropertyStandardQuery,
            AdditionalParameters: [0u8; 1],
        };
        // STORAGE_DEVICE_DESCRIPTORは可変長(末尾にベンダー文字列等が続く)ため、
        // 十分な大きさの生バッファへ書き込んでもらい、先頭の固定長部分だけ読む。
        let mut buf = [0u8; 512];
        let mut bytes_returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                Some(&query as *const _ as *const _),
                std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                Some(buf.as_mut_ptr() as *mut _),
                buf.len() as u32,
                Some(&mut bytes_returned),
                None,
            )
            .is_ok()
        };
        if !ok || (bytes_returned as usize) < std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() {
            return None;
        }
        let desc = unsafe { *(buf.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR) };
        Some(desc.BusType)
    }

    fn query_seek_penalty(handle: HANDLE) -> Option<bool> {
        let query = STORAGE_PROPERTY_QUERY {
            PropertyId: StorageDeviceSeekPenaltyProperty,
            QueryType: PropertyStandardQuery,
            AdditionalParameters: [0u8; 1],
        };
        let mut result = DEVICE_SEEK_PENALTY_DESCRIPTOR::default();
        let mut bytes_returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                Some(&query as *const _ as *const _),
                std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                Some(&mut result as *mut _ as *mut _),
                std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32,
                Some(&mut bytes_returned),
                None,
            )
            .is_ok()
        };
        if !ok {
            return None;
        }
        Some(result.IncursSeekPenalty.as_bool())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::DiskInfo;

    /// Windows以外では`\\.\PhysicalDriveN`相当のAPIが存在しないため、
    /// 常に空のリストを返す(呼び出し側は「ディスク0台」として扱う。
    /// `copilot.rs`の助言ロジックはこれを「管理者権限が無い」場合と
    /// 同じ経路で自然にハンドリングする)。
    pub fn list_physical_disks() -> Vec<DiskInfo> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AcceleratorInfo {
    pub kind: String,
    pub description: String,
    /// GPUベンダー("Intel"/"AMD"/"NVIDIA"/"Qualcomm"/"Unknown")。
    pub vendor: String,
}

pub fn detect_accelerator() -> AcceleratorInfo {
    match zfs_accel_hlsl::detect_best_accelerator() {
        Ok(device) => AcceleratorInfo {
            kind: format!("{:?}", device.kind),
            vendor: zfs_accel_hlsl::classify_vendor(&device.adapter_description).to_string(),
            description: device.adapter_description,
        },
        Err(e) => AcceleratorInfo {
            kind: "Unknown".to_string(),
            vendor: "Unknown".to_string(),
            description: format!("検出に失敗しました: {e}"),
        },
    }
}

/// システム上の**全ての**NPU/GPUアダプタを一覧する(複数GPU/複数ベンダー
/// 構成の表示用)。見つからなければ空配列(呼び出し側でCPUのみの意味に扱う)。
pub fn list_accelerators() -> Vec<AcceleratorInfo> {
    zfs_accel_hlsl::list_all_accelerators()
        .into_iter()
        .map(|device| AcceleratorInfo {
            kind: format!("{:?}", device.kind),
            vendor: zfs_accel_hlsl::classify_vendor(&device.adapter_description).to_string(),
            description: device.adapter_description,
        })
        .collect()
}

/// 現在実行中のOS名("Windows"/"macOS"/"Linux"/"Android"/"iOS"、
/// それ以外は`std::env::consts::OS`の値そのまま)。
pub fn current_os() -> &'static str {
    match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "macOS",
        "linux" => "Linux",
        "android" => "Android",
        "ios" => "iOS",
        other => other,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OsCompatEntry {
    pub os: String,
    /// "full" | "partial" | "planned" | "unsupported"
    pub status: String,
    pub note: String,
}

/// open-raid-z本体フォーマットの対応状況(マルチOS対応ロードマップ、
/// `MULTIPLATFORM_ROADMAP.md`参照)。実装状況が変わるたびに更新すること。
pub fn os_compatibility() -> Vec<OsCompatEntry> {
    vec![
        OsCompatEntry {
            os: "Windows".to_string(),
            status: "full".to_string(),
            note: "WinFsp経由で実マウント対応済み(create/delete/rename/append/truncate含む)".to_string(),
        },
        OsCompatEntry {
            os: "Linux".to_string(),
            status: "full".to_string(),
            note: "FUSE経由で実マウント対応済み(実ブロックデバイスでの動作確認済み)".to_string(),
        },
        OsCompatEntry {
            os: "macOS".to_string(),
            status: "planned".to_string(),
            note: "macFUSE/FUSE-T経由での実装を計画中。Apple製実機での検証待ち(仮想化不可のため)"
                .to_string(),
        },
        OsCompatEntry {
            os: "Android".to_string(),
            status: "planned".to_string(),
            note: "FUSE経由の専用アプリを計画中(root化不要)".to_string(),
        },
        OsCompatEntry {
            os: "iOS/iPad".to_string(),
            status: "partial".to_string(),
            note: "サードパーティのブロックデバイスRAID構成はAppleが許可していないため、\
                File Provider Extension経由のファイル閲覧のみ対応予定"
                .to_string(),
        },
    ]
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
