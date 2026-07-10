//! open_raid_z_core
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
#[cfg(feature = "foreign_fs")]
pub mod foreign_fs;
// `foreign_fs`をLinux/macOS上へ実際にマウント可能にする層。既存の
// RAID-Zプール用`fuse_mount`と同じ`fuser`クレートを使うため、両方の
// featureが有効な場合のみビルドする。
#[cfg(all(any(target_os = "linux", target_os = "macos"), feature = "fuse_backend", feature = "foreign_fs"))]
pub mod foreign_fuse_mount;
pub mod fs_ops;
pub mod id_mapping;
pub mod migrate;
#[cfg(feature = "winfsp_backend")]
pub mod mount;
// Linux・macOS(macFUSE/FUSE-T経由)で同じFUSEマウント実装を共有する。
// Androidは`fuser`クレート側の制約により現状未対応(Cargo.tomlの
// コメント、およびMULTIPLATFORM_ROADMAP.md参照)。
#[cfg(all(any(target_os = "linux", target_os = "macos"), feature = "fuse_backend"))]
pub mod fuse_mount;
pub mod partition;
pub mod pool;
pub mod raid10;
pub mod vdev;

pub use error::BridgeError;

/// ブリッジ全体のバージョン情報(デバッグ表示用)
pub const BRIDGE_VERSION: &str = env!("CARGO_PKG_VERSION");
