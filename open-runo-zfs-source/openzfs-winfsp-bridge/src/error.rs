use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("ZFSプールが見つかりません: {0}")]
    PoolNotFound(String),

    /// 指定した名前のデータセットが`Pool`に存在しない。
    #[error("データセットが見つかりません: {0}")]
    DatasetNotFound(String),

    /// 指定した名前のスナップショットが`Pool`に存在しない
    /// (`"データセット名@スナップショット名"`形式)。
    #[error("スナップショットが見つかりません: {0}")]
    SnapshotNotFound(String),

    /// データセット・スナップショット・クローンなど、同名のものが既に存在する。
    #[error("既に存在します: {0}")]
    AlreadyExists(String),

    /// プールの空き容量、またはデータセットの割当容量(`grow_dataset`で
    /// 確保済みの範囲)を超える要求だった。
    #[error("容量が不足しています: {0}")]
    CapacityExceeded(String),

    /// vdev構築時のパラメータが不正(例: `Raid10Vdev::new`のミラー幅指定ミス)。
    #[error("設定が不正です: {0}")]
    InvalidConfig(String),

    /// 同時に失われたディスク数がパリティ(冗長性)の許容範囲を超えており、
    /// データを復旧できない(ZFSでいう`DEGRADED`を超えて`FAULTED`になった状態)。
    #[error("冗長性を超えた同時故障のため復旧できません: {0}")]
    Unrecoverable(String),

    #[error("WinFspマウントに失敗しました: {0}")]
    MountFailed(String),

    #[error("ACL変換に失敗しました (POSIX ACE -> NTFS ACE): {0}")]
    AclTranslationFailed(String),

    #[error("exFAT属性/タイムスタンプの変換に失敗しました: {0}")]
    ExFatConversionFailed(String),

    #[error("未実装の機能です: {0}")]
    NotImplemented(&'static str),

    #[error("I/Oエラー: {0}")]
    Io(#[from] std::io::Error),
}

pub type BridgeResult<T> = Result<T, BridgeError>;
