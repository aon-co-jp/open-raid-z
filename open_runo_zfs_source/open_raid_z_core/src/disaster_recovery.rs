//! 切断耐性ディザスタリカバリ・オーケストレーション。
//!
//! [`crate::journal`](WAL的ジャーナル)と[`crate::offsite_backup`]
//! (Email/Googleドライブ/SFTPへの一時退避)を組み合わせ、以下3つの
//! ユーザー要望に対応する:
//!
//! 1. **切断耐性**: 書き込みは必ずまずローカルジャーナルへ記録してから
//!    本体へ反映する([`DisasterRecoveryManager::protect_write`])。
//! 2. **一時退避先の事前設定**: TOML設定ファイル([`DisasterRecoveryConfig`])
//!    で複数の退避先を登録できる。**初回セットアップ(プール初期化)時に
//!    [`DisasterRecoveryManager::run_first_time_setup`]を呼び、退避先の
//!    作成・接続確認を済ませてから実際の読み書きを開始する**、という
//!    順序を既定の推奨フローとする(ユーザー補足指示、2026-07-25)。
//!    認証情報が未準備の退避先はその場でスキップでき、後から
//!    設定ファイルへ追記するだけで有効化できる。
//! 3. **未設定時のフォールバック**: 退避先が1つも設定/成功しなくても、
//!    ローカルの別ディスク領域・OS一時ディレクトリへの緊急退避
//!    ([`DisasterRecoveryManager::emergency_local_fallback`])で
//!    完結させる(外部の未知サーバーへは絶対に送信しない)。
//!
//! さらに、再接続・電源復旧後に一時退避データを自動的に取り込んで
//! 切断前の状態へ復元する**自動復帰モード**
//! ([`DisasterRecoveryManager::spawn_background_auto_recovery`])を持つ。
//! 通常の読み書き経路(既存の`Pool::write`/`Pool::read`)を一切
//! ブロックしないよう、専用のバックグラウンドスレッドで実行し、
//! セグメント間に短い間隔を空けることで通常運用中のCPU/ネットワーク
//! 帯域への体感できる影響を避ける(ユーザー補足指示、2026-07-25)。
//! 復旧処理が失敗・中断しても、通常の読み書き機能自体には一切
//! 影響を与えない(独立して失敗できる設計、このエコシステムの
//! 既存の一貫方針を踏襲)。
#![cfg(feature = "offsite_backup")]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::accel::{self, AccelBackend};
use crate::error::{BridgeError, BridgeResult};
use crate::journal::{DisconnectJournal, JournalEntry};
use crate::offsite_backup::{
    EmailBackupTarget, EmailBackupTargetConfig, GoogleDriveBackupTarget, GoogleDriveBackupTargetConfig,
    OffsiteBackupTarget, SftpBackupTarget, SftpBackupTargetConfig,
};

/// TOML設定ファイル全体(このエコシステムの既存パターンに合わせた形式)。
///
/// 例:
/// ```toml
/// journal_dir = "F:/raidz-journal"
/// fallback_dirs = ["D:/emergency-fallback"]
/// auto_recover_on_reconnect = true
/// cleanup_offsite_after_replay = true
///
/// [[email]]
/// smtp_host = "smtp.example.com"
/// smtp_port = 587
/// smtp_username = "backup@example.com"
/// smtp_password_env = "RAIDZ_SMTP_PASSWORD"
/// from_address = "backup@example.com"
/// to_address = "admin@example.com"
///
/// [[sftp]]
/// host = "vps.example.com"
/// port = 22
/// username = "raidz"
/// password_env = "RAIDZ_SFTP_PASSWORD"
/// remote_backup_dir = "/home/raidz/offsite-backup"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisasterRecoveryConfig {
    /// ローカルジャーナル(WAL)の保存先ディレクトリ。
    pub journal_dir: PathBuf,
    /// 退避先が一つも設定/成功していない場合に使う、ローカルの
    /// 緊急退避候補ディレクトリ(優先順、存在確認できたものを使う)。
    /// 空でもOS一時ディレクトリへ自動フォールバックする。
    #[serde(default)]
    pub fallback_dirs: Vec<PathBuf>,
    /// 再接続後の自動復帰モードを有効にするか
    /// (データ消失を防ぐ方向の機能のため既定で有効、ユーザー補足指示)。
    #[serde(default = "default_true")]
    pub auto_recover_on_reconnect: bool,
    /// 自動復帰でローカルへ反映が完了した後、退避先に残ったセグメントを
    /// 削除するか(運用ポリシー。既定で有効=退避先を無期限に汚さない)。
    #[serde(default = "default_true")]
    pub cleanup_offsite_after_replay: bool,
    /// 自動復帰モードがセグメントを1件処理するごとに空けるスリープ時間
    /// (ミリ秒)。既定500msは、通常運用中のI/O・ネットワーク帯域との
    /// 競合を避けるための簡易スロットリング(ユーザー補足指示、
    /// 2026-07-25:「バックグラウンドで、現状の稼働中のシステム動作に
    /// 影響しない様にして」への対応)。
    #[serde(default = "default_throttle_ms")]
    pub auto_recovery_throttle_ms: u64,
    /// 退避先へ送信する前の圧縮/展開に使うハードウェアバックエンド
    /// (`src/accel.rs`参照)。既定はCPU。GPU/NPUを指定しても2026-07時点では
    /// 安全にCPUへフォールバックする(正直な開示、過剰実装回避)。
    #[serde(default)]
    pub accel_backend: AccelBackend,

    #[serde(default)]
    pub email: Vec<EmailBackupTargetConfig>,
    #[serde(default)]
    pub google_drive: Vec<GoogleDriveBackupTargetConfig>,
    #[serde(default)]
    pub sftp: Vec<SftpBackupTargetConfig>,
}

fn default_true() -> bool {
    true
}

fn default_throttle_ms() -> u64 {
    500
}

impl DisasterRecoveryConfig {
    pub fn load_from_toml_file(path: impl AsRef<Path>) -> BridgeResult<Self> {
        let text = std::fs::read_to_string(path.as_ref())
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("設定ファイル読み込み失敗: {e}")))?;
        toml::from_str(&text).map_err(|e| BridgeError::OffsiteBackupFailed(format!("設定ファイル解析失敗: {e}")))
    }

    /// 退避先を一切設定しない、ローカル完結の既定構成
    /// (「設定していなくても切断時の回避策を取れる」要件に対応)。
    pub fn local_only(journal_dir: impl Into<PathBuf>) -> Self {
        Self {
            journal_dir: journal_dir.into(),
            fallback_dirs: Vec::new(),
            auto_recover_on_reconnect: true,
            cleanup_offsite_after_replay: true,
            auto_recovery_throttle_ms: default_throttle_ms(),
            accel_backend: AccelBackend::Cpu,
            email: Vec::new(),
            google_drive: Vec::new(),
            sftp: Vec::new(),
        }
    }
}

/// 初回セットアップ([`DisasterRecoveryManager::run_first_time_setup`])の
/// 結果報告。ユーザーが認証情報未準備の退避先をスキップしても
/// 使用開始を妨げないため、成功/スキップ/失敗を退避先ごとに記録する。
#[derive(Debug, Clone, Serialize)]
pub struct TargetSetupOutcome {
    pub target_name: String,
    pub ready: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FirstTimeSetupReport {
    pub outcomes: Vec<TargetSetupOutcome>,
    /// 1つ以上の退避先が実際に使用可能になったか。falseの場合、
    /// ローカル完結のフォールバックのみで運用されることを意味する
    /// (それでも起動・使用は妨げない)。
    pub any_offsite_target_ready: bool,
}

/// 切断耐性ディザスタリカバリの司令塔。
pub struct DisasterRecoveryManager {
    journal: DisconnectJournal,
    fallback_dirs: Vec<PathBuf>,
    targets: Vec<Arc<dyn OffsiteBackupTarget>>,
    cleanup_offsite_after_replay: bool,
    auto_recover_on_reconnect: bool,
    auto_recovery_throttle: Duration,
    accel_backend: AccelBackend,
}

impl DisasterRecoveryManager {
    pub fn new(config: DisasterRecoveryConfig) -> BridgeResult<Self> {
        let journal = DisconnectJournal::open_or_create(config.journal_dir.clone())?;

        let mut targets: Vec<Arc<dyn OffsiteBackupTarget>> = Vec::new();
        for c in config.email {
            targets.push(Arc::new(EmailBackupTarget::new(c)));
        }
        for c in config.google_drive {
            targets.push(Arc::new(GoogleDriveBackupTarget::new(c)));
        }
        for c in config.sftp {
            targets.push(Arc::new(SftpBackupTarget::new(c)));
        }

        Ok(Self {
            journal,
            fallback_dirs: config.fallback_dirs,
            targets,
            cleanup_offsite_after_replay: config.cleanup_offsite_after_replay,
            auto_recover_on_reconnect: config.auto_recover_on_reconnect,
            auto_recovery_throttle: Duration::from_millis(config.auto_recovery_throttle_ms),
            accel_backend: config.accel_backend,
        })
    }

    /// **初回セットアップ(プール初期化/初回マウント)時に呼ぶ**推奨手順。
    /// 設定済みの退避先それぞれについて`ensure_ready`(フォルダ作成・
    /// 接続確認)を試み、失敗した退避先は「スキップ」として記録するだけで
    /// エラーにしない(実際の読み書き開始を妨げない)。
    ///
    /// 呼び出し側(CLIのセットアップウィザード等)は、この結果を
    /// ユーザーへ提示し、「退避先を1つも準備できていません。後から
    /// 設定ファイルへ追記して再実行できます」等の日英案内を出すことを
    /// 想定する(実際の文言はUI側の責務)。
    pub fn run_first_time_setup(&self) -> FirstTimeSetupReport {
        let mut outcomes = Vec::new();
        let mut any_ready = false;

        for target in &self.targets {
            let name = target.target_name();
            match target.ensure_ready() {
                Ok(()) => {
                    any_ready = true;
                    outcomes.push(TargetSetupOutcome {
                        target_name: name,
                        ready: true,
                        message: "退避先の準備が完了しました".to_string(),
                    });
                }
                Err(e) => {
                    tracing::warn!(target = %name, error = %e, "退避先の初回セットアップをスキップします");
                    outcomes.push(TargetSetupOutcome {
                        target_name: name,
                        ready: false,
                        message: format!("スキップ(未設定または接続失敗、後から再設定可能): {e}"),
                    });
                }
            }
        }

        if self.targets.is_empty() {
            outcomes.push(TargetSetupOutcome {
                target_name: "(退避先なし)".to_string(),
                ready: false,
                message: "退避先が1つも設定されていません。ローカル完結のフォールバックのみで運用されます。\
                          後から設定ファイルへEmail/Googleドライブ/SFTPを追記し、このセットアップを再実行することを推奨します。"
                    .to_string(),
            });
        }

        FirstTimeSetupReport { outcomes, any_offsite_target_ready: any_ready }
    }

    /// 1回分の書き込みを「ジャーナルへ記録→本体へ反映」の二段階で保護する。
    /// `apply` は実際の`Pool::write`相当の処理を渡すクロージャ。
    ///
    /// - `apply`が成功すればジャーナルエントリをcommitted扱いにし、
    ///   設定済みの退避先へベストエフォートで複製する(失敗しても
    ///   この関数の戻り値には影響しない=書き込み自体は成功のまま)。
    /// - `apply`が失敗した場合(電源断/USB切断/SATA切断等で本体への
    ///   反映ができなかったことを表す)、ローカル緊急退避
    ///   ([`Self::emergency_local_fallback`])を試みる。
    pub fn protect_write<F>(&self, dataset: &str, logical_offset: u64, data: Vec<u8>, apply: F) -> BridgeResult<()>
    where
        F: FnOnce(&[u8]) -> BridgeResult<()>,
    {
        let entry = self.journal.append_pending(dataset, logical_offset, data.clone())?;

        match apply(&data) {
            Ok(()) => {
                self.journal.mark_committed(entry.id)?;
                self.best_effort_offsite_backup(&entry);
                Ok(())
            }
            Err(apply_err) => {
                tracing::warn!(
                    dataset,
                    logical_offset,
                    error = %apply_err,
                    "本体への書き込みに失敗(切断の可能性)。ジャーナルは既に永続化済みのため、\
                     再接続後の自動復帰またはリプレイで復旧可能です。"
                );
                // ジャーナル自体は既にローカルディスク上で永続化済みなので、
                // 追加のローカル緊急退避(fallback_dirs/OS一時ディレクトリ)は
                // 「ジャーナル保存先ディスク自体も道連れで失われた」場合の
                // 保険として行う(例: ジャーナルとプールが同一物理ディスク上に
                // ある構成でのディスク単位の障害)。
                let _ = self.emergency_local_fallback(&entry);
                Err(apply_err)
            }
        }
    }

    /// 設定済みの退避先へジャーナルエントリを複製する(ベストエフォート、
    /// 失敗してもログのみでエラーを上へ伝播しない=主経路をブロックしない)。
    fn best_effort_offsite_backup(&self, entry: &JournalEntry) {
        if self.targets.is_empty() {
            return;
        }
        let label = format!("{:020}.entry.gz", entry.id);
        let bytes = match bincode::serialize(entry) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(error = %e, "退避用シリアライズに失敗");
                return;
            }
        };
        let compressed = match accel::compress(self.accel_backend, &bytes) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "退避用データの圧縮に失敗");
                return;
            }
        };
        for target in &self.targets {
            if let Err(e) = target.upload_segment(&label, &compressed) {
                tracing::warn!(target = %target.target_name(), error = %e, "退避先へのアップロードに失敗(ローカルジャーナルは健全)");
            }
        }
    }

    /// ローカルの別ディスク領域・OS一時ディレクトリへの緊急退避書き込み。
    /// 外部の未知サーバーへは絶対に送信しない(セキュリティ・プライバシー
    /// 上の理由、既存方針)。`fallback_dirs`の先頭から順に書き込みを試み、
    /// 全て失敗すれば`std::env::temp_dir()`を最終手段として使う。
    pub fn emergency_local_fallback(&self, entry: &JournalEntry) -> BridgeResult<PathBuf> {
        let label = format!("{:020}.emergency", entry.id);
        let bytes = bincode::serialize(entry)
            .map_err(|e| BridgeError::JournalFailed(format!("緊急退避用シリアライズ失敗: {e}")))?;

        let mut candidates: Vec<PathBuf> = self.fallback_dirs.clone();
        candidates.push(std::env::temp_dir().join("open_raid_z_emergency_fallback"));

        let mut last_err = None;
        for dir in candidates {
            match std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(dir.join(&label), &bytes)) {
                Ok(()) => return Ok(dir.join(&label)),
                Err(e) => last_err = Some(e),
            }
        }
        Err(BridgeError::JournalFailed(format!(
            "全ての緊急退避先への書き込みに失敗しました: {:?}",
            last_err
        )))
    }

    /// ローカルジャーナルにまだ残っている未反映エントリを再生する
    /// (退避先の設定有無に関わらず常に行う、通常のWALリプレイ)。
    /// 冪等(同じデータセット・同じオフセットへ同じ内容を書くだけ)なので、
    /// 途中で中断されても再実行して問題ない。
    pub fn replay_local_journal<F>(&self, mut apply: F) -> BridgeResult<usize>
    where
        F: FnMut(&JournalEntry) -> BridgeResult<()>,
    {
        let pending = self.journal.replay_pending()?;
        let mut applied = 0;
        for entry in &pending {
            apply(entry)?;
            self.journal.mark_committed(entry.id)?;
            applied += 1;
        }
        Ok(applied)
    }

    /// 自動復帰モード: 設定済みの退避先を巡回し、まだローカルへ反映して
    /// いない可能性のあるセグメントをダウンロード・再生する。
    /// 退避先が1つも設定されていない場合は何もしない(ローカルWALの
    /// 通常リプレイのみで十分、というユーザー補足指示どおりの扱い)。
    ///
    /// `already_applied_ids`には、直前の[`Self::replay_local_journal`]で
    /// 既にローカルジャーナルから復元済みのIDを渡し、二重適用を避ける
    /// (冪等な設計だが、無駄なダウンロード・書き込みを避けるため)。
    pub fn auto_recover_from_offsite<F>(&self, already_applied_ids: &HashSet<u64>, mut apply: F) -> BridgeResult<usize>
    where
        F: FnMut(&JournalEntry) -> BridgeResult<()>,
    {
        if !self.auto_recover_on_reconnect || self.targets.is_empty() {
            return Ok(0);
        }

        let mut recovered = 0;
        for target in &self.targets {
            let labels = match target.list_segments() {
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!(target = %target.target_name(), error = %e, "自動復帰: 退避先の一覧取得に失敗、次の退避先へ");
                    continue;
                }
            };

            for label in labels {
                std::thread::sleep(self.auto_recovery_throttle);

                let compressed = match target.download_segment(&label) {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!(target = %target.target_name(), label, error = %e, "自動復帰: ダウンロード失敗、スキップ");
                        continue;
                    }
                };
                let bytes = match accel::decompress(self.accel_backend, &compressed) {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!(target = %target.target_name(), label, error = %e, "自動復帰: 展開に失敗、スキップ");
                        continue;
                    }
                };
                let entry: JournalEntry = match bincode::deserialize(&bytes) {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!(target = %target.target_name(), label, error = %e, "自動復帰: デシリアライズ失敗、スキップ");
                        continue;
                    }
                };
                if !entry.is_intact() || already_applied_ids.contains(&entry.id) {
                    continue;
                }

                if let Err(e) = apply(&entry) {
                    tracing::warn!(target = %target.target_name(), label, error = %e, "自動復帰: ローカルへの反映に失敗、退避先には残す");
                    continue;
                }
                recovered += 1;

                if self.cleanup_offsite_after_replay {
                    if let Err(e) = target.delete_segment(&label) {
                        tracing::warn!(target = %target.target_name(), label, error = %e, "自動復帰: 反映後の退避先クリーンアップに失敗(致命的ではない)");
                    }
                }
            }
        }
        Ok(recovered)
    }

    /// 自動復帰モードを**バックグラウンドスレッドで**起動する(ユーザー
    /// 補足指示、2026-07-25:「バックグラウンドで、現状の稼働中の
    /// システム動作に影響しない様にして」への対応)。
    ///
    /// 非ブロッキング設計の要点:
    /// - `std::thread::spawn`による独立スレッドで実行し、呼び出し元
    ///   (通常はマウント直後のメインスレッド)を一切ブロックしない。
    /// - `apply`クロージャ(Pool側への実際の書き込み)は`Send`である
    ///   ことが必要——通常の読み書き経路とは別に、専用のロック/
    ///   チャネル経由でPoolへアクセスする実装を呼び出し側が用意する
    ///   想定(本クレートは汎用の司令塔のみを提供し、Pool固有の
    ///   排他制御は呼び出し側の責務とする)。
    /// - `auto_recovery_throttle_ms`(既定500ms)によりセグメント間へ
    ///   スリープを挟む簡易スロットリングで、通常運用中のI/O・
    ///   ネットワーク帯域との競合を避ける。
    /// - 復旧処理が失敗・panicしても、`JoinHandle`をdropするだけで
    ///   良く、通常の読み書き機能(呼び出し元スレッド)には影響しない
    ///   (このエコシステムの「補助機能の失敗は権威パスをブロックしない」
    ///   という既存方針を踏襲)。
    pub fn spawn_background_auto_recovery<F>(self: &Arc<Self>, already_applied_ids: HashSet<u64>, apply: F) -> JoinHandle<BridgeResult<usize>>
    where
        F: FnMut(&JournalEntry) -> BridgeResult<()> + Send + 'static,
    {
        let manager = Arc::clone(self);
        let apply = Mutex::new(apply);
        std::thread::spawn(move || {
            let mut apply = apply.lock().unwrap();
            manager.auto_recover_from_offsite(&already_applied_ids, |entry| apply(entry))
        })
    }

    pub fn journal(&self) -> &DisconnectJournal {
        &self.journal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn protect_write_success_marks_journal_committed() {
        let tmp = tempfile::tempdir().unwrap();
        let config = DisasterRecoveryConfig::local_only(tmp.path().join("journal"));
        let manager = DisasterRecoveryManager::new(config).unwrap();

        manager
            .protect_write("tank/data", 0, b"hello".to_vec(), |data| {
                assert_eq!(data, b"hello");
                Ok(())
            })
            .unwrap();

        assert_eq!(manager.journal().pending_len().unwrap(), 0, "成功時はpendingに残らないべき");
    }

    #[test]
    fn protect_write_failure_leaves_entry_in_journal_for_replay() {
        let tmp = tempfile::tempdir().unwrap();
        let config = DisasterRecoveryConfig::local_only(tmp.path().join("journal"));
        let manager = DisasterRecoveryManager::new(config).unwrap();

        let result = manager.protect_write("tank/data", 0, b"disconnected-write".to_vec(), |_data| {
            Err(BridgeError::Io(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "simulated disconnect")))
        });
        assert!(result.is_err());
        assert_eq!(manager.journal().pending_len().unwrap(), 1, "失敗時はリプレイのためpendingに残るべき");

        // 再接続後の通常リプレイで復旧できることを検証(冪等性)。
        let applied_count = Arc::new(AtomicUsize::new(0));
        let applied_count2 = Arc::clone(&applied_count);
        let replayed = manager
            .replay_local_journal(move |entry| {
                assert_eq!(entry.data, b"disconnected-write");
                applied_count2.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .unwrap();
        assert_eq!(replayed, 1);
        assert_eq!(applied_count.load(Ordering::SeqCst), 1);
        assert_eq!(manager.journal().pending_len().unwrap(), 0, "リプレイ後はpendingから消えるべき");
    }

    #[test]
    fn no_offsite_targets_configured_means_auto_recover_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let config = DisasterRecoveryConfig::local_only(tmp.path().join("journal"));
        let manager = DisasterRecoveryManager::new(config).unwrap();

        let recovered = manager.auto_recover_from_offsite(&HashSet::new(), |_entry| Ok(())).unwrap();
        assert_eq!(recovered, 0, "退避先未設定時は自動復帰は何もしないべき");
    }

    #[test]
    fn first_time_setup_reports_no_targets_without_failing() {
        let tmp = tempfile::tempdir().unwrap();
        let config = DisasterRecoveryConfig::local_only(tmp.path().join("journal"));
        let manager = DisasterRecoveryManager::new(config).unwrap();

        let report = manager.run_first_time_setup();
        assert!(!report.any_offsite_target_ready);
        assert_eq!(report.outcomes.len(), 1);
        assert!(!report.outcomes[0].ready);
    }

    #[test]
    fn emergency_local_fallback_writes_to_temp_dir_when_no_fallback_dirs_configured() {
        let tmp = tempfile::tempdir().unwrap();
        let config = DisasterRecoveryConfig::local_only(tmp.path().join("journal"));
        let manager = DisasterRecoveryManager::new(config).unwrap();

        let entry = manager.journal().append_pending("tank/data", 0, b"payload".to_vec()).unwrap();
        let written_path = manager.emergency_local_fallback(&entry).unwrap();
        assert!(written_path.exists());
    }

    #[test]
    fn background_auto_recovery_does_not_block_caller() {
        let tmp = tempfile::tempdir().unwrap();
        let config = DisasterRecoveryConfig::local_only(tmp.path().join("journal"));
        let manager = Arc::new(DisasterRecoveryManager::new(config).unwrap());

        let started = std::time::Instant::now();
        let handle = manager.spawn_background_auto_recovery(HashSet::new(), |_entry| Ok(()));
        // 退避先が無いため即noopだが、呼び出し自体が同期ブロックしない
        // (スレッド起動のみ)ことを検証する。
        assert!(started.elapsed() < Duration::from_millis(200));
        let recovered = handle.join().unwrap().unwrap();
        assert_eq!(recovered, 0);
    }
}
