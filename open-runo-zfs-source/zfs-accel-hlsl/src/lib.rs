//! zfs-accel-hlsl
//!
//! ZFSのチェックサム計算・RAID-Zパリティ計算・圧縮処理を
//! NPU/GPU(DirectX 12 Compute + DirectML)へオフロードするための骨格。
//!
//! 【設計方針】
//! - ハードウェアの有無に関わらず動作すること(CPUフォールバック必須)
//! - NPU/GPUどちらであってもDirectMLの同一インターフェースから呼べること
//! - openzfs-winfsp-bridge からは「バイト列を渡すとパリティ/チェックサムが
//!   返ってくる」という単純な関数として利用できること

pub mod compute;
pub mod device;
pub mod galois;
pub mod gf_matrix;
pub mod raidz23_parity;
pub mod raidz_parity;

pub use device::{detect_best_accelerator, AccelDevice, AccelKind, DeviceError};
