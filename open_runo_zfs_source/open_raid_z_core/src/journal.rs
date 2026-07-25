//! 切断耐性ジャーナル(Disconnect-tolerant WAL)。
//!
//! ZFSのZIL(ZFS Intent Log)/SLOGの考え方(同期書き込みをまず小さな
//! ログへ書き、後で本体ストレージへ反映する二段階方式)を参考にした、
//! `open_raid_z_core`独自のWAL実装。既存の`pool.rs`にはWAL/journalに
//! 相当する仕組みが無かった(2026-07-25調査で確認)ため、本モジュールで
//! 新規に追加する。
//!
//! 設計:
//! 1. 書き込み要求が来たら、まず`pending/<id>.entry`へ「データセット名・
//!    論理オフセット・データ本体・SHA-256チェックサム」をシリアライズして
//!    fsync付きで書く(この時点で電源断/切断が起きても、次回起動時に
//!    このエントリを再生すれば書き込みを完遂できる)。
//! 2. 本体(`Pool::write`)への反映に成功したら、そのエントリを
//!    `pending/`から取り除く(`committed`扱い)。
//! 3. 反映前に切断が起きた場合、`pending/`に残ったエントリが
//!    [`DisconnectJournal::replay_pending`]で列挙され、再接続後に
//!    呼び出し側([`crate::disaster_recovery`])が本体へ再適用できる。
//!
//! 冪等性: エントリ適用は「同じデータセット・同じ論理オフセットへ
//! 同じバイト列を書き込む」だけなので、二重適用しても結果は変わらない
//! (CoWの上書きは同一内容の再書き込みであれば安全)。よって自動復帰処理が
//! 中断されて再実行されても安全。
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{BridgeError, BridgeResult};

/// ジャーナル1エントリ(1回分の書き込み要求)。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JournalEntry {
    /// このジャーナル内での通し番号(ファイル名にも使う)。
    pub id: u64,
    /// 対象データセット名。
    pub dataset: String,
    /// 対象データセット内の論理オフセット。
    pub logical_offset: u64,
    /// 書き込むデータ本体。
    pub data: Vec<u8>,
    /// `data`のSHA-256(破損検知用。ジャーナル自体がディスク破損の
    /// 途中で切れた場合に不完全なエントリを再生しないようにする)。
    pub checksum: [u8; 32],
    /// エントリ作成時刻(UNIX epoch秒、診断用)。
    pub created_at_unix: u64,
}

impl JournalEntry {
    fn new(id: u64, dataset: &str, logical_offset: u64, data: Vec<u8>) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let checksum = hasher.finalize().into();
        let created_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            id,
            dataset: dataset.to_string(),
            logical_offset,
            data,
            checksum,
            created_at_unix,
        }
    }

    /// チェックサムが実データと一致するか(破損したジャーナルエントリの検知)。
    pub fn is_intact(&self) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(&self.data);
        let actual: [u8; 32] = hasher.finalize().into();
        actual == self.checksum
    }
}

/// 切断耐性ジャーナル本体。`dir`配下に`pending/`サブディレクトリを持ち、
/// 未反映のエントリをファイルとして保持する。
pub struct DisconnectJournal {
    dir: PathBuf,
    pending_dir: PathBuf,
    next_id: AtomicU64,
}

impl DisconnectJournal {
    /// `dir`(存在しなければ作成)をジャーナルの保存先として開く。
    /// 既存の`pending/`エントリがあれば、そのID群からnext_idを継続する。
    pub fn open_or_create(dir: impl Into<PathBuf>) -> BridgeResult<Self> {
        let dir = dir.into();
        let pending_dir = dir.join("pending");
        fs::create_dir_all(&pending_dir)
            .map_err(|e| BridgeError::JournalFailed(format!("ジャーナルディレクトリ作成失敗 {:?}: {e}", pending_dir)))?;

        let mut max_id = 0u64;
        for entry in fs::read_dir(&pending_dir)
            .map_err(|e| BridgeError::JournalFailed(format!("ジャーナルディレクトリ読み取り失敗: {e}")))?
        {
            let entry = entry.map_err(|e| BridgeError::JournalFailed(e.to_string()))?;
            if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                if let Ok(id) = stem.parse::<u64>() {
                    max_id = max_id.max(id);
                }
            }
        }

        Ok(Self { dir, pending_dir, next_id: AtomicU64::new(max_id + 1) })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn entry_path(&self, id: u64) -> PathBuf {
        self.pending_dir.join(format!("{id:020}.entry"))
    }

    /// 書き込み要求を、本体へ反映する前にまずジャーナルへ記録する
    /// (fsync込み。電源断/切断発生タイミングを問わず、この関数が
    /// 正常終了した時点でこのエントリは永続化されている)。
    pub fn append_pending(&self, dataset: &str, logical_offset: u64, data: Vec<u8>) -> BridgeResult<JournalEntry> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let entry = JournalEntry::new(id, dataset, logical_offset, data);
        let bytes = bincode::serialize(&entry)
            .map_err(|e| BridgeError::JournalFailed(format!("ジャーナルエントリのシリアライズ失敗: {e}")))?;

        let path = self.entry_path(id);
        let tmp_path = path.with_extension("entry.tmp");
        {
            let mut f = fs::File::create(&tmp_path)
                .map_err(|e| BridgeError::JournalFailed(format!("ジャーナル一時ファイル作成失敗: {e}")))?;
            f.write_all(&bytes).map_err(|e| BridgeError::JournalFailed(format!("ジャーナル書き込み失敗: {e}")))?;
            f.sync_all().map_err(|e| BridgeError::JournalFailed(format!("ジャーナルfsync失敗: {e}")))?;
        }
        // リネームはPOSIX/NTFS双方でアトミックな置換操作(途中状態が
        // 見えないため、中途半端なエントリがreplay対象に混じらない)。
        fs::rename(&tmp_path, &path).map_err(|e| BridgeError::JournalFailed(format!("ジャーナルrename失敗: {e}")))?;

        Ok(entry)
    }

    /// 本体への反映が完了したエントリを`pending`から取り除く
    /// (これでもう自動復帰時のreplay対象から外れる)。
    pub fn mark_committed(&self, id: u64) -> BridgeResult<()> {
        let path = self.entry_path(id);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(BridgeError::JournalFailed(format!("ジャーナルエントリ削除失敗: {e}"))),
        }
    }

    /// 未反映(=本体へ届く前に切断/電源断が起きた可能性がある)エントリを
    /// 全て列挙する。IDの昇順(=書き込まれた順序)でソート済み。
    /// 破損している(チェックサム不一致の)エントリは安全のため除外し、
    /// `tracing::warn!`で記録する。
    pub fn replay_pending(&self) -> BridgeResult<Vec<JournalEntry>> {
        let mut entries = Vec::new();
        for dir_entry in fs::read_dir(&self.pending_dir)
            .map_err(|e| BridgeError::JournalFailed(format!("ジャーナルディレクトリ読み取り失敗: {e}")))?
        {
            let dir_entry = dir_entry.map_err(|e| BridgeError::JournalFailed(e.to_string()))?;
            let path = dir_entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("entry") {
                continue;
            }
            let bytes = fs::read(&path).map_err(|e| BridgeError::JournalFailed(format!("ジャーナル読み取り失敗: {e}")))?;
            match bincode::deserialize::<JournalEntry>(&bytes) {
                Ok(entry) if entry.is_intact() => entries.push(entry),
                Ok(entry) => {
                    tracing::warn!(id = entry.id, "破損したジャーナルエントリを検知、リプレイから除外します");
                }
                Err(e) => {
                    tracing::warn!(?path, error = %e, "ジャーナルエントリのデシリアライズに失敗、リプレイから除外します");
                }
            }
        }
        entries.sort_by_key(|e| e.id);
        Ok(entries)
    }

    /// 現在の未反映エントリ件数(監視・診断用)。
    pub fn pending_len(&self) -> BridgeResult<usize> {
        Ok(self.replay_pending()?.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_then_replay_returns_entry_in_order() {
        let tmp = tempfile::tempdir().unwrap();
        let journal = DisconnectJournal::open_or_create(tmp.path()).unwrap();

        journal.append_pending("tank/data", 0, b"first".to_vec()).unwrap();
        journal.append_pending("tank/data", 4096, b"second".to_vec()).unwrap();

        let pending = journal.replay_pending().unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].data, b"first");
        assert_eq!(pending[1].data, b"second");
    }

    #[test]
    fn mark_committed_removes_entry_from_replay() {
        let tmp = tempfile::tempdir().unwrap();
        let journal = DisconnectJournal::open_or_create(tmp.path()).unwrap();

        let entry = journal.append_pending("tank/data", 0, b"payload".to_vec()).unwrap();
        journal.mark_committed(entry.id).unwrap();

        assert_eq!(journal.replay_pending().unwrap().len(), 0);
    }

    #[test]
    fn reopening_journal_continues_id_sequence_and_preserves_pending() {
        let tmp = tempfile::tempdir().unwrap();
        {
            let journal = DisconnectJournal::open_or_create(tmp.path()).unwrap();
            journal.append_pending("tank/data", 0, b"unflushed-before-power-loss".to_vec()).unwrap();
        }
        // ここで「電源断」に見立てて`DisconnectJournal`を作り直す。
        let journal = DisconnectJournal::open_or_create(tmp.path()).unwrap();
        let pending = journal.replay_pending().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].data, b"unflushed-before-power-loss");

        let entry = journal.append_pending("tank/data", 4096, b"after-restart".to_vec()).unwrap();
        assert!(entry.id > pending[0].id, "再起動後もID採番は継続するべき");
    }

    #[test]
    fn corrupted_entry_is_excluded_from_replay() {
        let tmp = tempfile::tempdir().unwrap();
        let journal = DisconnectJournal::open_or_create(tmp.path()).unwrap();
        let entry = journal.append_pending("tank/data", 0, b"payload".to_vec()).unwrap();

        // ディスク破損を模して、エントリファイルの中身を一部壊す。
        let path = journal.entry_path(entry.id);
        // 末尾8バイトは`created_at_unix`(u64)なので、そこは避けて
        // その直前(`checksum: [u8; 32]`の最後の1バイト)を破壊する。
        // これにより「bincodeとしてはデシリアライズできるが、実データと
        // チェックサムが一致しない」という現実的な破損パターンを再現する。
        let mut bytes = fs::read(&path).unwrap();
        let corrupt_index = bytes.len() - 9;
        bytes[corrupt_index] ^= 0xFF;
        fs::write(&path, bytes).unwrap();

        let pending = journal.replay_pending().unwrap();
        assert_eq!(pending.len(), 0, "破損エントリはreplay対象から除外されるべき");
    }
}
