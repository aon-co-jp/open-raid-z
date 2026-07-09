//! 既存のファイルシステム(NTFS等)上のディレクトリツリーを、[`Pool`]のデータセット群へ
//! コピーして取り込むための移行ツール。
//!
//! 【できること】
//! ソースディレクトリの各ファイルを読み込み、`Pool`内に同名相当のデータセットとして
//! 書き込み、書き込み直後にその場でチャンクごと読み戻して内容が一致することを
//! 検証する。**コピー元には一切書き込まない**ため、通常のファイルコピーツール
//! (robocopy等)と同じ安全性であり、Windowsが起動したまま安全に実行できる。
//!
//! 【できないこと・意図的にやらないこと】
//! - **現在起動中のWindowsシステムドライブ(C:等)を、無停止でその場RAID-Z/RAID6形式へ
//!   変換することはできない**。OS自身が使用中のボリュームを、そのOS上で動く
//!   ソフトウェアが書き換えることは原理的に不可能なため(実在するどのファイル
//!   システム変換ツールも、対象ボリュームのアンマウント/オフライン化を必須と
//!   している)。このツールはあくまで「(起動中のシステムドライブではない)
//!   既存のNTFSボリュームから、別の場所(open-raid-zのプール)へコピーする」
//!   ものであり、ソース側のディスク・ファイルシステムそのものは一切変更しない。
//! - `Pool`/`mount.rs`は現状ルート直下のフラットな名前空間のみ対応
//!   (サブディレクトリ未対応)なので、サブディレクトリを含む階層は
//!   `flatten_separator`で指定した文字を使って1階層のデータセット名へ平坦化する
//!   (例: `docs/readme.txt` -> `docs_readme.txt`)。平坦化の結果、名前が
//!   Windowsのファイル名として不正になる、または他のファイルと衝突する場合は
//!   そのファイルだけを安全にスキップして報告する([`MigrationOutcome::Skipped`])。

use crate::error::{BridgeError, BridgeResult};
use crate::pool::Pool;
use crate::vdev::Vdev;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Windowsのファイル名として不正な文字。`mount.rs`の同名の制約と揃えている
/// (平坦化後のデータセット名を、WinFspマウント経由でファイルとして
/// 公開できるようにするため)。
const INVALID_NAME_CHARS: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

/// ストリーミングコピー時のチャンクサイズ(既定値)。巨大なファイルでも
/// メモリを使い切らないよう、一度にこの単位でしか読み書き・検証しない。
pub const DEFAULT_MIGRATION_CHUNK_BYTES: usize = 8 * 1024 * 1024;

/// 移行1件ぶんの結果。
#[derive(Debug)]
pub enum MigrationOutcome {
    /// コピーと検証(書き込み直後の読み戻し比較)に成功した。
    Migrated { source: PathBuf, dataset_name: String, bytes: u64 },
    /// 何らかの理由でスキップした(このファイルについてはプールへ一切書き込んでいない。
    /// 途中まで書き込んで失敗した場合は、そのデータセットを片付けたうえでスキップ扱いにする)。
    Skipped { source: PathBuf, reason: String },
}

/// [`migrate_directory_into_pool`]全体の結果。
#[derive(Debug, Default)]
pub struct MigrationReport {
    pub outcomes: Vec<MigrationOutcome>,
}

impl MigrationReport {
    pub fn migrated_count(&self) -> usize {
        self.outcomes.iter().filter(|o| matches!(o, MigrationOutcome::Migrated { .. })).count()
    }

    pub fn skipped_count(&self) -> usize {
        self.outcomes.iter().filter(|o| matches!(o, MigrationOutcome::Skipped { .. })).count()
    }
}

/// `source_dir`以下を再帰的に走査し、各ファイルを`pool`内の新しいデータセットとして
/// コピーする(既定のチャンクサイズを使う版。詳細は[`migrate_directory_into_pool_with_chunk_size`]参照)。
pub fn migrate_directory_into_pool<V: Vdev>(
    pool: &mut Pool<V>,
    source_dir: &Path,
    flatten_separator: char,
) -> BridgeResult<MigrationReport> {
    migrate_directory_into_pool_with_chunk_size(pool, source_dir, flatten_separator, DEFAULT_MIGRATION_CHUNK_BYTES)
}

/// [`migrate_directory_into_pool`]と同じだが、ストリーミングコピーのチャンクサイズを
/// 指定できる(テストで小さい値を使い、複数チャンクにまたがるコピーを高速に検証するため)。
pub fn migrate_directory_into_pool_with_chunk_size<V: Vdev>(
    pool: &mut Pool<V>,
    source_dir: &Path,
    flatten_separator: char,
    chunk_bytes: usize,
) -> BridgeResult<MigrationReport> {
    assert!(chunk_bytes > 0, "chunk_bytesは1以上である必要があります");

    let mut report = MigrationReport::default();
    let mut used_names: HashSet<String> = pool.dataset_names().into_iter().collect();

    let mut relative_paths = Vec::new();
    collect_files(source_dir, source_dir, &mut relative_paths)?;

    for relative in relative_paths {
        let source = source_dir.join(&relative);
        let dataset_name = flatten_name(&relative, flatten_separator);

        if dataset_name.is_empty() || dataset_name.contains(INVALID_NAME_CHARS) {
            report.outcomes.push(MigrationOutcome::Skipped {
                source,
                reason: format!("平坦化後の名前がマウント上で使えません: '{dataset_name}'"),
            });
            continue;
        }
        if !used_names.insert(dataset_name.clone()) {
            report.outcomes.push(MigrationOutcome::Skipped {
                source,
                reason: format!("平坦化後の名前が別のファイルと衝突しました: '{dataset_name}'"),
            });
            continue;
        }

        match migrate_one_file(pool, &source, &dataset_name, chunk_bytes) {
            Ok(bytes) => report.outcomes.push(MigrationOutcome::Migrated { source, dataset_name, bytes }),
            Err(e) => {
                // 失敗時は中途半端なデータセットを残さない(プールの空き容量も返却される)。
                let _ = pool.destroy_dataset(&dataset_name);
                report.outcomes.push(MigrationOutcome::Skipped { source, reason: e.to_string() });
            }
        }
    }

    Ok(report)
}

/// 1ファイルをチャンク単位でストリーミングコピーする。各チャンクは書き込み直後に
/// 同じ範囲を読み戻して内容を比較し、コピー中に何かがおかしくなっても検出できる
/// ようにする(全体を読み終えるまでメモリに保持し続けることはしない)。
fn migrate_one_file<V: Vdev>(
    pool: &mut Pool<V>,
    source: &Path,
    dataset_name: &str,
    chunk_bytes: usize,
) -> BridgeResult<u64> {
    let mut file = std::fs::File::open(source)?;
    pool.create_dataset(dataset_name)?;

    let mut offset = 0u64;
    let mut buf = vec![0u8; chunk_bytes];
    loop {
        let n = read_fill(&mut file, &mut buf)?;
        if n == 0 {
            break;
        }
        let chunk = &buf[..n];
        pool.write_unaligned_growing(dataset_name, offset, chunk)?;
        let read_back = pool.read_unaligned(dataset_name, offset, n as u64)?;
        if read_back != chunk {
            return Err(BridgeError::Io(std::io::Error::other(format!(
                "コピー直後の検証で内容が一致しませんでした(データセット'{dataset_name}', offset={offset})"
            ))));
        }
        offset += n as u64;
    }
    Ok(offset)
}

/// `buf`が一杯になるか、ファイル終端に達するまで読み込む。`File::read`は
/// 必ずしも要求したバイト数ぶん一度に返すとは限らないため必要。
fn read_fill(file: &mut std::fs::File, buf: &mut [u8]) -> BridgeResult<usize> {
    let mut total = 0;
    while total < buf.len() {
        let n = file.read(&mut buf[total..])?;
        if n == 0 {
            break;
        }
        total += n;
    }
    Ok(total)
}

/// `source_dir`以下の全ファイル(ディレクトリ・シンボリックリンク等は除く)を
/// `source_dir`からの相対パスとして収集する。
fn collect_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> BridgeResult<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(root, &path, out)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|e| BridgeError::Io(std::io::Error::other(e.to_string())))?;
            out.push(relative.to_path_buf());
        }
        // シンボリックリンク等(is_file()もis_dir()もfalse)は対象外として自然にスキップされる。
    }
    Ok(())
}

/// 相対パスの各階層を`separator`で連結し、ルート直下の1つの名前へ平坦化する。
fn flatten_name(relative: &Path, separator: char) -> String {
    relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(&separator.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_device::FileBackedDevice;
    use crate::vdev::{RaidLevel, RaidZVdev};

    const CHUNK_SIZE: usize = 64;
    const NUM_STRIPES: u64 = 64;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("open_runo_migrate_it_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn build_pool(dir: &Path) -> Pool<RaidZVdev<FileBackedDevice>> {
        let devices: Vec<FileBackedDevice> = (0..6)
            .map(|i| {
                let path = dir.join(format!("disk{i}.img"));
                FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * NUM_STRIPES).unwrap()
            })
            .collect();
        let vdev = RaidZVdev::new(devices, RaidLevel::Z2, CHUNK_SIZE);
        Pool::new(vdev, NUM_STRIPES)
    }

    /// テスト用の「移行元」ディレクトリ(実際のE:/F:等ではなく、あくまで一時
    /// スクラッチディレクトリ)にサンプルのファイルツリーを作る。
    fn make_source_tree(dir: &Path) {
        std::fs::write(dir.join("readme.txt"), b"top level file").unwrap();
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        std::fs::write(dir.join("docs").join("guide.txt"), b"nested file content").unwrap();
        std::fs::write(dir.join("empty.txt"), b"").unwrap();
    }

    #[test]
    fn migrates_a_directory_tree_into_flattened_datasets_and_verifies_contents() {
        let pool_dir = scratch_dir("basic_pool");
        let source_dir = scratch_dir("basic_source");
        make_source_tree(&source_dir);

        let mut pool = build_pool(&pool_dir);
        let report = migrate_directory_into_pool(&mut pool, &source_dir, '_').unwrap();

        assert_eq!(report.migrated_count(), 3);
        assert_eq!(report.skipped_count(), 0);

        assert_eq!(pool.read_unaligned("readme.txt", 0, 14).unwrap(), b"top level file");
        assert_eq!(pool.read_unaligned("docs_guide.txt", 0, 19).unwrap(), b"nested file content");
        assert_eq!(pool.dataset_size("empty.txt").unwrap(), 0);

        std::fs::remove_dir_all(&pool_dir).ok();
        std::fs::remove_dir_all(&source_dir).ok();
    }

    #[test]
    fn does_not_modify_the_source_files_at_all() {
        let pool_dir = scratch_dir("readonly_pool");
        let source_dir = scratch_dir("readonly_source");
        make_source_tree(&source_dir);
        let before = std::fs::read(source_dir.join("readme.txt")).unwrap();
        let before_modified = std::fs::metadata(source_dir.join("readme.txt")).unwrap().modified().unwrap();

        let mut pool = build_pool(&pool_dir);
        migrate_directory_into_pool(&mut pool, &source_dir, '_').unwrap();

        let after = std::fs::read(source_dir.join("readme.txt")).unwrap();
        let after_modified = std::fs::metadata(source_dir.join("readme.txt")).unwrap().modified().unwrap();
        assert_eq!(before, after, "コピー元のファイル内容が変化してはいけない");
        assert_eq!(before_modified, after_modified, "コピー元の更新日時が変化してはいけない(=書き込みが一切発生していない)");

        std::fs::remove_dir_all(&pool_dir).ok();
        std::fs::remove_dir_all(&source_dir).ok();
    }

    #[test]
    fn streams_a_file_spanning_multiple_chunks_and_reassembles_it_exactly() {
        let pool_dir = scratch_dir("multi_chunk_pool");
        let source_dir = scratch_dir("multi_chunk_source");
        std::fs::create_dir_all(&source_dir).unwrap();
        // 小さいチャンクサイズ(16バイト)に対して十分大きい(5チャンクぶんの)ファイル。
        let payload: Vec<u8> = (0..77u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(source_dir.join("video.bin"), &payload).unwrap();

        let mut pool = build_pool(&pool_dir);
        let report = migrate_directory_into_pool_with_chunk_size(&mut pool, &source_dir, '_', 16).unwrap();

        assert_eq!(report.migrated_count(), 1);
        assert_eq!(pool.read_unaligned("video.bin", 0, payload.len() as u64).unwrap(), payload);

        std::fs::remove_dir_all(&pool_dir).ok();
        std::fs::remove_dir_all(&source_dir).ok();
    }

    #[test]
    fn flattening_collisions_are_skipped_without_touching_the_other_file() {
        let pool_dir = scratch_dir("collision_pool");
        let source_dir = scratch_dir("collision_source");
        std::fs::create_dir_all(source_dir.join("a")).unwrap();
        // "a_x.txt"というファイル名自体と、"a/x.txt"というネストしたファイルは、
        // 区切り文字"_"での平坦化後どちらも"a_x.txt"になり衝突する。
        // `std::fs::read_dir`の列挙順はOS依存でどちらが先に処理されるか保証
        // されないため、「衝突した片方だけが生き残り、もう片方はデータを
        // 破壊せずスキップされる」ことだけを検証する(勝者の内容までは固定しない)。
        std::fs::write(source_dir.join("a_x.txt"), b"top level").unwrap();
        std::fs::write(source_dir.join("a").join("x.txt"), b"nested").unwrap();

        let mut pool = build_pool(&pool_dir);
        let report = migrate_directory_into_pool(&mut pool, &source_dir, '_').unwrap();

        assert_eq!(report.migrated_count(), 1, "衝突した2件目はスキップされ、1件だけ移行される");
        assert_eq!(report.skipped_count(), 1);
        let survivor_size = pool.dataset_size("a_x.txt").unwrap();
        let survivor = pool.read_unaligned("a_x.txt", 0, survivor_size).unwrap();
        assert!(
            survivor == b"top level" || survivor == b"nested",
            "生き残った方の内容が、どちらのソースファイルとも一致しない"
        );

        std::fs::remove_dir_all(&pool_dir).ok();
        std::fs::remove_dir_all(&source_dir).ok();
    }

    #[test]
    fn invalid_dataset_names_are_skipped_without_stopping_the_whole_migration() {
        let pool_dir = scratch_dir("invalid_name_pool");
        let source_dir = scratch_dir("invalid_name_source");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(source_dir.join("ok.txt"), b"fine").unwrap();
        // 区切り文字を":"にすると、平坦化結果がWindowsで使えない文字を含む名前になる。
        std::fs::create_dir_all(source_dir.join("weird")).unwrap();
        std::fs::write(source_dir.join("weird").join("file.txt"), b"bad").unwrap();

        let mut pool = build_pool(&pool_dir);
        let report = migrate_directory_into_pool(&mut pool, &source_dir, ':').unwrap();

        assert_eq!(report.migrated_count(), 1);
        assert_eq!(report.skipped_count(), 1);
        assert_eq!(pool.read_unaligned("ok.txt", 0, 4).unwrap(), b"fine");

        std::fs::remove_dir_all(&pool_dir).ok();
        std::fs::remove_dir_all(&source_dir).ok();
    }
}
