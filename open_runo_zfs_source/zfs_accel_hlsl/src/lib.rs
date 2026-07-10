//! zfs_accel_hlsl
//!
//! ZFSのチェックサム計算・RAID-Zパリティ計算・圧縮処理を
//! NPU/GPU(DirectX 12 Compute + DirectML)へオフロードするための骨格。
//!
//! 【設計方針】
//! - ハードウェアの有無に関わらず動作すること(CPUフォールバック必須)
//! - NPU/GPUどちらであってもDirectMLの同一インターフェースから呼べること
//! - open_raid_z_core からは「バイト列を渡すとパリティ/チェックサムが
//!   返ってくる」という単純な関数として利用できること

pub mod benchmark;
pub mod bitmatrix;
#[cfg(feature = "gpu")]
pub mod compute;
#[cfg(feature = "gpu")]
pub mod dml_gemm;
pub mod device;
pub mod galois;
pub mod gf_matrix;
pub mod raidz23_parity;
pub mod raidz_parity;
#[cfg(feature = "vulkan")]
pub mod vulkan_compute;
#[cfg(feature = "vulkan")]
pub mod vulkan_device;

pub use device::{
    classify_vendor, detect_best_accelerator, list_all_accelerators, AccelDevice, AccelKind, DeviceError,
};
