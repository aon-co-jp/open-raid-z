//! WinFspがOSから受け取るファイルシステム操作要求を、ZFSの操作へ
//! マッピングするための抽象化層。
//!
//! 実際のWinFsp連携では `winfsp::filesystem::FileSystemContext` トレイト
//! (crateバージョンにより名称が異なる)を実装しますが、ここでは
//! ライブラリの詳細に依存しない独自トレイトとして定義し、
//! 後段でwinfsp-rsの実トレイトへ委譲するアダプタを書く想定です。

use crate::acl_emulation::{NtfsAce, ZfsAce};
use crate::error::BridgeResult;
use crate::id_mapping::IdMappingTable;

/// ZFSバックエンドが実装すべき最小限の操作セット。
/// 実装は libzfs への FFI、または zfs(8)/zpool(8) 相当のコマンド呼び出し、
/// もしくは将来的な pure-Rust ZFS 実装のいずれかを想定。
pub trait ZfsBackend {
    fn open_dataset(&self, name: &str) -> BridgeResult<DatasetHandle>;
    fn read(&self, handle: &DatasetHandle, path: &str, offset: u64, len: u32) -> BridgeResult<Vec<u8>>;
    fn write(&self, handle: &DatasetHandle, path: &str, offset: u64, data: &[u8]) -> BridgeResult<u32>;
    fn get_acl(&self, handle: &DatasetHandle, path: &str) -> BridgeResult<Vec<ZfsAce>>;
    fn set_acl(&self, handle: &DatasetHandle, path: &str, aces: &[ZfsAce]) -> BridgeResult<()>;
}

#[derive(Debug, Clone)]
pub struct DatasetHandle {
    pub pool_name: String,
    pub dataset_name: String,
}

/// WinFsp側から呼ばれる想定のGetSecurity実装イメージ
/// (実際のシグネチャはwinfsp-rsのトレイトに合わせて調整が必要)
pub fn handle_get_security(
    backend: &dyn ZfsBackend,
    handle: &DatasetHandle,
    path: &str,
    mapping: &IdMappingTable,
) -> BridgeResult<Vec<NtfsAce>> {
    let zfs_aces = backend.get_acl(handle, path)?;
    zfs_aces
        .iter()
        .map(|ace| crate::acl_emulation::zfs_ace_to_ntfs(ace, mapping))
        .collect()
}
