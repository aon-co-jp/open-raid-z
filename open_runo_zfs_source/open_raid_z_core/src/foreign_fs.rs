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

type ExfatFile = exfat_fs::dir::entry::fs::file::File<std::fs::File>;
type ExfatDir = exfat_fs::dir::entry::fs::directory::Directory<std::fs::File>;
type ExfatElement = exfat_fs::dir::entry::fs::FsElement<std::fs::File>;
type ExfatRoot = exfat_fs::dir::Root<std::fs::File>;

fn exfat_element_name(e: &ExfatElement) -> &str {
    match e {
        exfat_fs::dir::entry::fs::FsElement::F(f) => f.name(),
        exfat_fs::dir::entry::fs::FsElement::D(d) => d.name(),
    }
}

fn summarize_exfat_items(items: &[ExfatElement]) -> Vec<ForeignDirEntry> {
    items
        .iter()
        .map(|e| match e {
            exfat_fs::dir::entry::fs::FsElement::F(f) => {
                ForeignDirEntry { name: f.name().to_string(), is_dir: false, size_bytes: f.len() }
            }
            exfat_fs::dir::entry::fs::FsElement::D(d) => {
                ForeignDirEntry { name: d.name().to_string(), is_dir: true, size_bytes: 0 }
            }
        })
        .collect()
}

enum ExfatResolved {
    Dir(Vec<ForeignDirEntry>),
    FileBytes(Vec<u8>),
}

enum ExfatWant {
    List,
    Read,
}

fn open_exfat_dir(dir: &ExfatDir) -> BridgeResult<Vec<ExfatElement>> {
    dir.open().map_err(|e| BridgeError::ForeignFsFailed(format!("'{}'を開けませんでした: {e:?}", dir.name())))
}

fn read_exfat_file(file: &mut ExfatFile) -> BridgeResult<Vec<u8>> {
    let mut buf = Vec::new();
    io::Read::read_to_end(file, &mut buf)
        .map_err(|e| BridgeError::ForeignFsFailed(format!("'{}'の読み取りに失敗: {e}", file.name())))?;
    Ok(buf)
}

/// ルート直下から、`comps`の先頭要素だけを解決する(1階層分)。
/// それより深い階層は[`resolve_exfat_owned`]へ委譲する
/// (`Root::items()`は`&mut`借用のため、所有権を持つ`Vec`へ移った後は
/// 再帰でシンプルに扱える)。
fn resolve_exfat_root(root: &mut ExfatRoot, comps: &[&str], want: ExfatWant) -> BridgeResult<ExfatResolved> {
    if comps.is_empty() {
        return match want {
            ExfatWant::List => Ok(ExfatResolved::Dir(summarize_exfat_items(root.items()))),
            ExfatWant::Read => Err(BridgeError::ForeignFsFailed("ルートはディレクトリです".to_string())),
        };
    }

    let items = root.items();
    let idx = items
        .iter()
        .position(|e| exfat_element_name(e).eq_ignore_ascii_case(comps[0]))
        .ok_or_else(|| BridgeError::ForeignFsFailed(format!("'{}'が見つかりません", comps[0])))?;

    match &mut items[idx] {
        exfat_fs::dir::entry::fs::FsElement::D(dir) => {
            let opened = open_exfat_dir(dir)?;
            resolve_exfat_owned(opened, &comps[1..], want)
        }
        exfat_fs::dir::entry::fs::FsElement::F(file) => {
            if comps.len() != 1 {
                return Err(BridgeError::ForeignFsFailed(format!("'{}'はファイルです(ディレクトリではありません)", comps[0])));
            }
            match want {
                ExfatWant::Read => Ok(ExfatResolved::FileBytes(read_exfat_file(file)?)),
                ExfatWant::List => Err(BridgeError::ForeignFsFailed(format!("'{}'はファイルです", comps[0]))),
            }
        }
    }
}

/// [`resolve_exfat_root`]の2階層目以降(所有権を持つ`Vec<ExfatElement>`を
/// 消費しながら再帰的に降りていく)。
fn resolve_exfat_owned(items: Vec<ExfatElement>, comps: &[&str], want: ExfatWant) -> BridgeResult<ExfatResolved> {
    if comps.is_empty() {
        return match want {
            ExfatWant::List => Ok(ExfatResolved::Dir(summarize_exfat_items(&items))),
            ExfatWant::Read => Err(BridgeError::ForeignFsFailed("指定パスはディレクトリです".to_string())),
        };
    }

    for mut item in items {
        if !exfat_element_name(&item).eq_ignore_ascii_case(comps[0]) {
            continue;
        }
        return match &mut item {
            exfat_fs::dir::entry::fs::FsElement::D(dir) => {
                let opened = open_exfat_dir(dir)?;
                resolve_exfat_owned(opened, &comps[1..], want)
            }
            exfat_fs::dir::entry::fs::FsElement::F(file) => {
                if comps.len() != 1 {
                    Err(BridgeError::ForeignFsFailed(format!("'{}'はファイルです(ディレクトリではありません)", comps[0])))
                } else {
                    match want {
                        ExfatWant::Read => Ok(ExfatResolved::FileBytes(read_exfat_file(file)?)),
                        ExfatWant::List => Err(BridgeError::ForeignFsFailed(format!("'{}'はファイルです", comps[0]))),
                    }
                }
            }
        };
    }
    Err(BridgeError::ForeignFsFailed(format!("'{}'が見つかりません", comps[0])))
}

/// 既存のexFATボリューム(実デバイスまたはイメージファイル)への読み取り
/// 専用ハンドル。上流クレート(`exfat-fs` 0.1系)が現時点で書き込みに
/// 対応していないため、[`ForeignFatVolume`]とは異なり読み取りのみ提供する。
pub struct ForeignExfatVolume {
    path: std::path::PathBuf,
}

impl ForeignExfatVolume {
    /// 既存のexFATボリュームを開く。`path`は実デバイスまたはループバック
    /// イメージファイル。開けること自体をここで確認する(fail fast)。
    pub fn open(path: impl AsRef<Path>) -> BridgeResult<Self> {
        let path = path.as_ref().to_path_buf();
        let _ = Self::open_root(&path)?;
        Ok(Self { path })
    }

    fn open_root(path: &Path) -> BridgeResult<ExfatRoot> {
        let file = OpenOptions::new().read(true).open(path).map_err(BridgeError::Io)?;
        exfat_fs::dir::Root::open(file)
            .map_err(|e| BridgeError::ForeignFsFailed(format!("exFATボリュームとして開けませんでした: {e:?}")))
    }

    /// ボリューム内のディレクトリ(`"/"`がルート)の内容を一覧する。
    pub fn list_dir(&self, dir_path: &str) -> BridgeResult<Vec<ForeignDirEntry>> {
        let mut root = Self::open_root(&self.path)?;
        let comps: Vec<&str> = normalize(dir_path).split('/').filter(|s| !s.is_empty()).collect();
        match resolve_exfat_root(&mut root, &comps, ExfatWant::List)? {
            ExfatResolved::Dir(entries) => Ok(entries),
            ExfatResolved::FileBytes(_) => unreachable!("ExfatWant::Listはディレクトリ一覧のみ返す"),
        }
    }

    /// ボリューム内のファイルを丸ごと読み取る。
    pub fn read_file(&self, file_path: &str) -> BridgeResult<Vec<u8>> {
        let mut root = Self::open_root(&self.path)?;
        let comps: Vec<&str> = normalize(file_path).split('/').filter(|s| !s.is_empty()).collect();
        if comps.is_empty() {
            return Err(BridgeError::ForeignFsFailed("ファイルパスを指定してください".to_string()));
        }
        match resolve_exfat_root(&mut root, &comps, ExfatWant::Read)? {
            ExfatResolved::FileBytes(bytes) => Ok(bytes),
            ExfatResolved::Dir(_) => unreachable!("ExfatWant::Readはファイル内容のみ返す"),
        }
    }
}
