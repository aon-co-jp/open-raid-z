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

    /// 既存フォーマット(FAT32等)のブリッジ機能([`crate::foreign_fs`])での失敗。
    #[error("既存フォーマットの読み書きに失敗しました: {0}")]
    ForeignFsFailed(String),

    #[error("未実装の機能です: {0}")]
    NotImplemented(&'static str),

    #[error("I/Oエラー: {0}")]
    Io(#[from] std::io::Error),

    /// 切断耐性ジャーナル([`crate::journal`])関連の失敗
    /// (ジャーナルディレクトリの作成・エントリ書き込み・リプレイ等)。
    #[error("切断耐性ジャーナルの操作に失敗しました: {0}")]
    JournalFailed(String),

    /// 一時退避先([`crate::offsite_backup`])への送信・取得・初期セットアップの失敗。
    /// 単体では致命的ではなく、フォールバック(ローカル緊急退避)が別途動く前提。
    #[error("一時退避先の操作に失敗しました: {0}")]
    OffsiteBackupFailed(String),
}

pub type BridgeResult<T> = Result<T, BridgeError>;
