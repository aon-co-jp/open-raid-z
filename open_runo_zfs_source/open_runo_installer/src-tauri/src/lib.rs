//! Tauriコマンドの薄いラッパー。実際のロジック(ディスク検出・構成助言・
//! zpoolプレビュー)は`open_runo_installer_core`クレートにある
//! (Tauriに依存しない独立クレートへ切り出した理由は同クレートの
//! Cargo.tomlコメント参照)。

use open_runo_installer_core::copilot::{Advice, AdviceContext, Advisor, HeuristicAdvisor};
use open_runo_installer_core::hardware::{self, AcceleratorInfo, DiskInfo, OsCompatEntry};
use open_runo_installer_core::zpool_wizard::{
    self, Raid10InitRequest, Raid10InitResult, ZpoolApplyRequest, ZpoolInitRequest,
    ZpoolInitResult,
};
use serde::Serialize;

#[tauri::command]
fn detect_accelerator() -> AcceleratorInfo {
    hardware::detect_accelerator()
}

#[tauri::command]
fn list_physical_disks() -> Vec<DiskInfo> {
    hardware::list_physical_disks()
}

/// 「対応状況」パネル(開閉可能)向けの一括取得コマンド。現在のOS・
/// OSごとの対応状況・検出できた全GPU/NPU・検出できたディスクの
/// メディア種別を1回の呼び出しでまとめて返す。
#[derive(Debug, Clone, Serialize)]
struct SystemStatus {
    current_os: String,
    os_compatibility: Vec<OsCompatEntry>,
    accelerators: Vec<AcceleratorInfo>,
    disks: Vec<DiskInfo>,
}

#[tauri::command]
fn get_system_status() -> SystemStatus {
    SystemStatus {
        current_os: hardware::current_os().to_string(),
        os_compatibility: hardware::os_compatibility(),
        accelerators: hardware::list_accelerators(),
        disks: hardware::list_physical_disks(),
    }
}

#[tauri::command]
fn init_zpool_preview(req: ZpoolInitRequest) -> Result<ZpoolInitResult, String> {
    zpool_wizard::init_zpool_preview(req)
}

#[tauri::command]
fn init_raid10_preview(req: Raid10InitRequest) -> Result<Raid10InitResult, String> {
    zpool_wizard::init_raid10_preview(req)
}

#[tauri::command]
fn init_zpool_apply(req: ZpoolApplyRequest) -> Result<ZpoolInitResult, String> {
    zpool_wizard::init_zpool_apply(req)
}

#[tauri::command]
fn get_disk_advice() -> Vec<Advice> {
    let context = AdviceContext::scan_current_machine();
    HeuristicAdvisor.advise(&context)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            detect_accelerator,
            list_physical_disks,
            init_zpool_preview,
            init_raid10_preview,
            init_zpool_apply,
            get_disk_advice,
            get_system_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
