//! Tauriコマンドの薄いラッパー。実際のロジック(ディスク検出・構成助言・
//! zpoolプレビュー)は`open_runo_installer_core`クレートにある
//! (Tauriに依存しない独立クレートへ切り出した理由は同クレートの
//! Cargo.tomlコメント参照)。

use open_runo_installer_core::copilot::{Advice, AdviceContext, Advisor, HeuristicAdvisor};
use open_runo_installer_core::hardware::{self, AcceleratorInfo, DiskInfo};
use open_runo_installer_core::zpool_wizard::{
    self, Raid10InitRequest, Raid10InitResult, ZpoolInitRequest, ZpoolInitResult,
};

#[tauri::command]
fn detect_accelerator() -> AcceleratorInfo {
    hardware::detect_accelerator()
}

#[tauri::command]
fn list_physical_disks() -> Vec<DiskInfo> {
    hardware::list_physical_disks()
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
            get_disk_advice
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
