use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("ZFSプールが見つかりません: {0}")]
    PoolNotFound(String),

    #[error("WinFspマウントに失敗しました: {0}")]
    MountFailed(String),

    #[error("ACL変換に失敗しました (POSIX ACE -> NTFS ACE): {0}")]
    AclTranslationFailed(String),

    #[error("未実装の機能です: {0}")]
    NotImplemented(&'static str),

    #[error("I/Oエラー: {0}")]
    Io(#[from] std::io::Error),
}

pub type BridgeResult<T> = Result<T, BridgeError>;
