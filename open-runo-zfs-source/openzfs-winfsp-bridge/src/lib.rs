//! openzfs-winfsp-bridge
//!
//! ZFS on Windows のI/OをWinFsp経由でフックし、
//! NTFSのACL/タイムスタンプ/代替データストリーム相当のセマンティクスを
//! エミュレーションする互換層。
//!
//! 【重要な前提】
//! これは実運用可能な完成ドライバではなく、設計を検証するための
//! スキャフォールディング(骨組み)です。実際にZFSのオンディスク構造を
//! 読み書きする部分(zpool/ZIL/uberblock解析等)は含まれていません。
//! 本物のOpenZFSライブラリ(libzfs/libzpool相当)へのFFIバインディングを
//! 後続で差し込む前提の「型とインターフェース設計」がこのファイルの役割です。

pub mod acl_emulation;
pub mod block_device;
pub mod checksum;
pub mod error;
pub mod exfat_emulation;
pub mod fs_ops;
pub mod id_mapping;
pub mod pool;
pub mod raid10;
pub mod vdev;

pub use error::BridgeError;

/// ブリッジ全体のバージョン情報(デバッグ表示用)
pub const BRIDGE_VERSION: &str = env!("CARGO_PKG_VERSION");
