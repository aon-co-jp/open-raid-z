mod copilot;
mod hardware;
mod zpool_wizard;

use copilot::{Advice, AdviceContext, Advisor, HeuristicAdvisor};
use hardware::{AcceleratorInfo, DiskInfo};
use zpool_wizard::{Raid10InitRequest, Raid10InitResult, ZpoolInitRequest, ZpoolInitResult};

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
