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
        let file = OpenOptions::new().read(true).write(true).open(path).map_err(BridgeError::Io)?;
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
                let entry =
                    entry.map_err(|e| BridgeError::ForeignFsFailed(format!("ディレクトリ読み取りに失敗: {e}")))?;
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
            let name = entry.file_name();
            // ルート直下と異なり、非ルートディレクトリは`fatfs`が`.`/`..`を
            // 実エントリとして返す(FATのディレクトリ領域そのものに含まれる
            // ため)。呼び出し側(CLIの`ls`・FUSEの`readdir`)は`.`/`..`を
            // 自前で合成する前提のため、ここでは除外する。
            if name == "." || name == ".." {
                continue;
            }
            entries.push(ForeignDirEntry { name, is_dir: entry.is_dir(), size_bytes: entry.len() });
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

    /// ボリューム内へ新規ディレクトリを作成する(`orzctl foreign`のマウント
    /// 拡張用。既存の`ls`/`cat`/`put`では使わない)。
    pub fn create_dir(&self, dir_path: &str) -> BridgeResult<()> {
        self.fs
            .root_dir()
            .create_dir(normalize(dir_path))
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{dir_path}'を作成できませんでした: {e}")))?;
        Ok(())
    }

    /// ボリューム内のファイルまたは(空の)ディレクトリを削除する。
    pub fn remove(&self, path: &str) -> BridgeResult<()> {
        self.fs
            .root_dir()
            .remove(normalize(path))
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{path}'を削除できませんでした: {e}")))?;
        Ok(())
    }

    /// ボリューム内のファイル・ディレクトリを名前変更/移動する。
    pub fn rename(&self, src_path: &str, dst_path: &str) -> BridgeResult<()> {
        let root = self.fs.root_dir();
        root.rename(normalize(src_path), &root, normalize(dst_path))
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{src_path}'から'{dst_path}'への変更に失敗: {e}")))?;
        Ok(())
    }
}

/// `fatfs`はパス区切りに`/`を使い、先頭の`/`は不要(ルート相対)。
/// Windows形式のパス(`\`区切りや先頭`/`付き)を渡されても動くように正規化する。
fn normalize(path: &str) -> &str {
    path.trim_start_matches('/').trim_start_matches('\\')
}

/// 既存のexFATボリューム(実デバイスまたはイメージファイル)への読み書き
/// ハンドル。上流クレートを`exfat-fs`(0.1系、読み取り専用)から
/// `hadris-fat`(`write`+`exfat` feature、読み書き両対応)へ移行した。
pub struct ForeignExfatVolume {
    fs: hadris_fat::exfat::ExFatFs<std::fs::File>,
}

impl ForeignExfatVolume {
    /// 既存のexFATボリュームを開く。`path`は実デバイスまたはループバック
    /// イメージファイル。
    pub fn open(path: impl AsRef<Path>) -> BridgeResult<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path).map_err(BridgeError::Io)?;
        let fs = hadris_fat::exfat::ExFatFs::open(file)
            .map_err(|e| BridgeError::ForeignFsFailed(format!("exFATボリュームとして開けませんでした: {e:?}")))?;
        Ok(Self { fs })
    }

    /// ボリューム内のディレクトリ(`"/"`がルート)の内容を一覧する。
    pub fn list_dir(&self, dir_path: &str) -> BridgeResult<Vec<ForeignDirEntry>> {
        let normalized = normalize(dir_path);
        let dir = if normalized.is_empty() {
            self.fs.root_dir()
        } else {
            self.fs
                .open_dir(normalized)
                .map_err(|e| BridgeError::ForeignFsFailed(format!("'{dir_path}'を開けませんでした: {e:?}")))?
        };
        let mut entries = Vec::new();
        for entry in dir.entries() {
            let entry =
                entry.map_err(|e| BridgeError::ForeignFsFailed(format!("ディレクトリ読み取りに失敗: {e:?}")))?;
            entries.push(ForeignDirEntry {
                name: entry.name.clone(),
                is_dir: entry.is_directory(),
                size_bytes: entry.size(),
            });
        }
        Ok(entries)
    }

    /// ボリューム内のファイルを丸ごと読み取る。
    pub fn read_file(&self, file_path: &str) -> BridgeResult<Vec<u8>> {
        let mut reader = self
            .fs
            .open_file(normalize(file_path))
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{file_path}'を開けませんでした: {e:?}")))?;
        let mut out = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = hadris_fat::io::Read::read(&mut reader, &mut buf)
                .map_err(|e| BridgeError::ForeignFsFailed(format!("'{file_path}'の読み取りに失敗: {e:?}")))?;
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }
        Ok(out)
    }

    /// ボリューム内へファイルを新規作成(または既存を上書き)して書き込む。
    /// 現状はルート直下のみ対応(サブディレクトリへの書き込みは未対応、
    /// `foreign_fs`のFAT32/FAT16実装と同様の制約)。
    pub fn write_file(&self, file_path: &str, data: &[u8]) -> BridgeResult<()> {
        let name = normalize(file_path);
        if name.contains('/') {
            return Err(BridgeError::ForeignFsFailed(
                "exFATへの書き込みは現状ルート直下のみ対応しています(サブディレクトリは未対応)".to_string(),
            ));
        }
        let root = self.fs.root_dir();
        let entry = self
            .fs
            .create_file(&root, name)
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{file_path}'を作成できませんでした: {e:?}")))?;
        let mut writer = self
            .fs
            .write_file(&entry)
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{file_path}'への書き込み準備に失敗: {e:?}")))?;
        hadris_fat::io::Write::write_all(&mut writer, data)
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{file_path}'への書き込みに失敗: {e:?}")))?;
        writer
            .finish()
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{file_path}'の書き込み確定に失敗: {e:?}")))?;
        Ok(())
    }

    /// ルート直下に新規ディレクトリを作成する(FAT32版と同様、現状ルート
    /// 直下のみ対応)。
    pub fn create_dir(&self, dir_path: &str) -> BridgeResult<()> {
        let name = normalize(dir_path);
        if name.contains('/') {
            return Err(BridgeError::ForeignFsFailed(
                "exFATでのディレクトリ作成は現状ルート直下のみ対応しています".to_string(),
            ));
        }
        let root = self.fs.root_dir();
        self.fs
            .create_dir(&root, name)
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{dir_path}'を作成できませんでした: {e:?}")))?;
        Ok(())
    }

    /// ルート直下のファイルまたはディレクトリを削除する。
    pub fn remove(&self, path: &str) -> BridgeResult<()> {
        let name = normalize(path);
        let root = self.fs.root_dir();
        let entry = root
            .find(name)
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{path}'を開けませんでした: {e:?}")))?
            .ok_or_else(|| BridgeError::ForeignFsFailed(format!("'{path}'が見つかりません")))?;
        self.fs
            .delete(&entry)
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{path}'を削除できませんでした: {e:?}")))?;
        Ok(())
    }
}

/// 既存のext2/ext4ボリューム(実デバイスまたはイメージファイル)への
/// **読み取り専用**ハンドル。
///
/// 上流の`ext4-view`クレート(純Rust・読み取り専用)をラップする。
/// FAT32/exFATと異なり書き込みには対応しない(2026-07時点で書き込み対応の
/// 成熟した純Rust ext4実装が存在しないため。書き込みが必要な場合は
/// Linux上でカーネルのext4ドライバを使うこと)。読み取り専用である旨は
/// 各書き込み系APIが常にエラーを返すことで明示する。
pub struct ForeignExt4Volume {
    fs: ext4_view::Ext4,
}

impl ForeignExt4Volume {
    /// 既存のext2/ext4ボリュームを開く。`path`は実デバイス(パーティションを
    /// 指すパス)またはループバックイメージファイルのいずれでもよい。
    pub fn open(path: impl AsRef<Path>) -> BridgeResult<Self> {
        let fs = ext4_view::Ext4::load_from_path(path.as_ref())
            .map_err(|e| BridgeError::ForeignFsFailed(format!("ext4ボリュームとして開けませんでした: {e}")))?;
        Ok(Self { fs })
    }

    /// `ext4-view`はパスを`/`始まりの絶対パスとして解釈する。FAT32/exFAT側の
    /// `normalize`(先頭`/`を剥がす)とは逆に、こちらは先頭`/`を保証する。
    fn absolutize(path: &str) -> String {
        let trimmed = path.trim_start_matches(['/', '\\']);
        format!("/{trimmed}")
    }

    /// ボリューム内のディレクトリ(`"/"`がルート)の内容を一覧する。
    pub fn list_dir(&self, dir_path: &str) -> BridgeResult<Vec<ForeignDirEntry>> {
        let abs = Self::absolutize(dir_path);
        let entries = self
            .fs
            .read_dir(abs.as_str())
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{dir_path}'を開けませんでした: {e}")))?;
        let mut out = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| BridgeError::ForeignFsFailed(format!("ディレクトリ読み取りに失敗: {e}")))?;
            let name = String::from_utf8_lossy(entry.file_name().as_ref()).into_owned();
            if name == "." || name == ".." {
                continue;
            }
            let file_type = entry
                .file_type()
                .map_err(|e| BridgeError::ForeignFsFailed(format!("'{name}'の種別取得に失敗: {e}")))?;
            let is_dir = file_type.is_dir();
            let size_bytes = if is_dir { 0 } else { entry.metadata().map(|m| m.len()).unwrap_or(0) };
            out.push(ForeignDirEntry { name, is_dir, size_bytes });
        }
        Ok(out)
    }

    /// ボリューム内のファイルを丸ごと読み取る。
    pub fn read_file(&self, file_path: &str) -> BridgeResult<Vec<u8>> {
        let abs = Self::absolutize(file_path);
        self.fs
            .read(abs.as_str())
            .map_err(|e| BridgeError::ForeignFsFailed(format!("'{file_path}'の読み取りに失敗: {e}")))
    }

    /// ext4は読み取り専用のため、書き込みは常にエラーを返す。
    pub fn write_file(&self, _file_path: &str, _data: &[u8]) -> BridgeResult<()> {
        Err(BridgeError::ForeignFsFailed(
            "ext4への書き込みは未対応です(読み取り専用。書き込み対応の成熟した純Rust実装が無いため)".to_string(),
        ))
    }

    /// ext4は読み取り専用のため、ディレクトリ作成は常にエラーを返す。
    pub fn create_dir(&self, _dir_path: &str) -> BridgeResult<()> {
        Err(BridgeError::ForeignFsFailed("ext4へのディレクトリ作成は未対応です(読み取り専用)".to_string()))
    }

    /// ext4は読み取り専用のため、削除は常にエラーを返す。
    pub fn remove(&self, _path: &str) -> BridgeResult<()> {
        Err(BridgeError::ForeignFsFailed("ext4での削除は未対応です(読み取り専用)".to_string()))
    }
}
