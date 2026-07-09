//! open-raid-z**以外**の既存フォーマット(まずはFAT32/FAT16)を読み書きする
//! ためのブリッジ。
//!
//! `crate::pool`/`crate::vdev`はopen-raid-z独自のRAID-Zプールを扱うのに
//! 対し、本モジュールは「USBメモリ/microSD/CFカード等に既に存在する、
//! 他OS(Windows/Mac/Linux/Android等)が作った通常のFAT32/FAT16ボリューム」
//! を、open-raid-zのツール群からそのまま読み書きできるようにする。
//!
//! 【設計方針】自前でFATの仕様を実装するのではなく、実績のある純Rust実装
//! (`fatfs`クレート)をラップする。`fatfs`は`Read + Write + Seek`を実装した
//! 任意のバックエンドに対して動作するため、ここでは通常の`std::fs::File`
//! (実デバイスパス・イメージファイルどちらも同じ方法で開ける)を
//! `fscommon::BufStream`でラップして渡す。ネイティブライブラリへの依存が
//! 無いため、Windows/Linux/Mac/Androidのいずれでも同じコードでビルドできる
//! (このfeatureが`foreign_fs`という名前になっているのはこのため)。

use crate::error::{BridgeError, BridgeResult};
use fscommon::BufStream;
use std::fs::OpenOptions;
use std::io;
use std::path::Path;

type Backend = BufStream<std::fs::File>;
type FatFs = fatfs::FileSystem<Backend>;

/// FAT32/FAT16ボリューム内の1エントリ(ファイルまたはディレクトリ)。
#[derive(Debug, Clone)]
pub struct ForeignDirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
}

/// 既存のFAT32/FAT16ボリューム(実デバイスまたはイメージファイル)への
/// 読み書きハンドル。
pub struct ForeignFatVolume {
    fs: FatFs,
}

impl ForeignFatVolume {
    /// 既存のFAT32/FAT16ボリュームを開く。`path`は実デバイス
    /// (`/dev/sdX`、`\\.\PhysicalDriveN`等、パーティション自体を指す
    /// パスを想定)、またはループバックイメージファイルのいずれでもよい。
    pub fn open(path: impl AsRef<Path>) -> BridgeResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(BridgeError::Io)?;
        let backend = BufStream::new(file);
        let fs = fatfs::FileSystem::new(backend, fatfs::FsOptions::new())
            .map_err(|e| BridgeError::ForeignFsFailed(format!("FATボリュームとして開けませんでした: {e}")))?;
        Ok(Self { fs })
    }

    /// ボリューム内のディレクトリ(`"/"`がルート)の内容を一覧する。
    pub fn list_dir(&self, dir_path: &str) -> BridgeResult<Vec<ForeignDirEntry>> {
        let root = self.fs.root_dir();
        let normalized = normalize(dir_path);

        let mut entries = Vec::new();
        // `fatfs`ではルート自体は`open_dir("")`できないため、空パス(=ルート
        // 指定)は`root_dir()`をそのまま使う。
        if normalized.is_empty() {
            for entry in root.iter() {
                let entry = entry.map_err(|e| BridgeError::ForeignFsFailed(format!("ディレクトリ読み取りに失敗: {e}")))?;
                entries.push(ForeignDirEntry {
                    name: entry.file_name(),
                    is_dir: entry.is_dir(),
                    size_bytes: entry.len(),
                });
            }
            return Ok(entries);
        }

        let dir = root
            .open_dir(normalized)
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{dir_path}'を開けませんでした: {e}")))?;

        for entry in dir.iter() {
            let entry = entry.map_err(|e| BridgeError::ForeignFsFailed(format!("ディレクトリ読み取りに失敗: {e}")))?;
            entries.push(ForeignDirEntry {
                name: entry.file_name(),
                is_dir: entry.is_dir(),
                size_bytes: entry.len(),
            });
        }
        Ok(entries)
    }

    /// ボリューム内のファイルを丸ごと読み取る。
    pub fn read_file(&self, file_path: &str) -> BridgeResult<Vec<u8>> {
        let mut file = self
            .fs
            .root_dir()
            .open_file(normalize(file_path))
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{file_path}'を開けませんでした: {e}")))?;
        let mut buf = Vec::new();
        io::Read::read_to_end(&mut file, &mut buf)
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{file_path}'の読み取りに失敗: {e}")))?;
        Ok(buf)
    }

    /// ボリューム内へファイルを新規作成(または既存を上書き)して書き込む。
    pub fn write_file(&self, file_path: &str, data: &[u8]) -> BridgeResult<()> {
        let mut file = self
            .fs
            .root_dir()
            .create_file(normalize(file_path))
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{file_path}'を作成できませんでした: {e}")))?;
        file.truncate().map_err(|e| BridgeError::ForeignFsFailed(format!("既存内容の切り詰めに失敗: {e}")))?;
        io::Write::write_all(&mut file, data)
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{file_path}'への書き込みに失敗: {e}")))?;
        io::Write::flush(&mut file).map_err(|e| BridgeError::ForeignFsFailed(format!("flushに失敗: {e}")))?;
        Ok(())
    }
}

/// `fatfs`はパス区切りに`/`を使い、先頭の`/`は不要(ルート相対)。
/// Windows形式のパス(`\`区切りや先頭`/`付き)を渡されても動くように正規化する。
fn normalize(path: &str) -> &str {
    path.trim_start_matches('/').trim_start_matches('\\')
}
