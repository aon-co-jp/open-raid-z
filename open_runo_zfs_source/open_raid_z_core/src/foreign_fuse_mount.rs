//! `foreign_fs`(既存のFAT32/FAT16/exFATボリューム)をLinux上へ実際に
//! マウント可能にする、`fuse_mount.rs`のforeign版。
//!
//! `fuse_mount.rs`(open-raid-z独自のRAID-Zプール用)がフラットな名前空間
//! (1データセット=1ファイル)しか扱えないのに対し、こちらは`ForeignFatVolume`/
//! `ForeignExfatVolume`が元々パス文字列ベースの階層アクセスに対応している
//! ため、**本物のディレクトリ階層をそのままFUSE越しに公開できる**。
//!
//! 【設計】FUSEはinode番号でファイルを識別するため、「フルパス文字列 <->
//! inode番号」の対応表を持つ(`fuse_mount.rs`の「データセット名 <-> inode」と
//! 同じ考え方を、階層パスへ拡張したもの)。`ForeignFatVolume`/
//! `ForeignExfatVolume`はハンドルを保持しない(呼び出しのたびにボリューム内を
//! パス解決する)設計のため、書き込みは「open〜releaseの間はメモリ上へ
//! バッファし、releaseで`write_file`を1回呼ぶ」方式にした(`fatfs`/
//! `hadris-fat`がどちらも「全内容を渡して書き込む」APIのため、この方式が
//! 最も単純かつ正しい)。
//!
//! 【現状のスコープ】
//! - FAT32/FAT16: 読み書き・ディレクトリ作成/削除・ファイル削除・
//!   名前変更(リネーム)、全て対応。
//! - exFAT: 読み取り・ルート直下への書き込み/ディレクトリ作成・削除は対応。
//!   リネームは上流クレート(`hadris-fat`)が未対応のため`ENOSYS`を返す。
//!   サブディレクトリへの書き込みも上流の制約により未対応。

use crate::error::BridgeError;
use crate::foreign_fs::{ForeignDirEntry, ForeignExfatVolume, ForeignFatVolume};
use fuser::{
    Errno, FileAttr, FileHandle, FileType, Filesystem, Generation, INodeNo, MountOption, ReplyAttr,
    ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyWrite, Request,
};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

const ROOT_INO: u64 = 1;
const ATTR_TTL: Duration = Duration::from_secs(1);

fn errno_from_bridge_error(e: &BridgeError) -> Errno {
    match e {
        BridgeError::ForeignFsFailed(_) | BridgeError::Io(_) => Errno::EIO,
        _ => Errno::EIO,
    }
}

/// `ForeignFatVolume`(FAT32/FAT16)と`ForeignExfatVolume`(exFAT)を、
/// 共通のパスベースAPIとして扱うためのラッパー。
pub enum ForeignVolume {
    Fat(ForeignFatVolume),
    Exfat(ForeignExfatVolume),
}

impl ForeignVolume {
    fn list_dir(&self, path: &str) -> Result<Vec<ForeignDirEntry>, BridgeError> {
        match self {
            ForeignVolume::Fat(v) => v.list_dir(path),
            ForeignVolume::Exfat(v) => v.list_dir(path),
        }
    }
    fn read_file(&self, path: &str) -> Result<Vec<u8>, BridgeError> {
        match self {
            ForeignVolume::Fat(v) => v.read_file(path),
            ForeignVolume::Exfat(v) => v.read_file(path),
        }
    }
    fn write_file(&self, path: &str, data: &[u8]) -> Result<(), BridgeError> {
        match self {
            ForeignVolume::Fat(v) => v.write_file(path, data),
            ForeignVolume::Exfat(v) => v.write_file(path, data),
        }
    }
    fn create_dir(&self, path: &str) -> Result<(), BridgeError> {
        match self {
            ForeignVolume::Fat(v) => v.create_dir(path),
            ForeignVolume::Exfat(v) => v.create_dir(path),
        }
    }
    fn remove(&self, path: &str) -> Result<(), BridgeError> {
        match self {
            ForeignVolume::Fat(v) => v.remove(path),
            ForeignVolume::Exfat(v) => v.remove(path),
        }
    }
    /// exFATは上流クレートがrenameに対応していないため`NotImplemented`を返す。
    fn rename(&self, src: &str, dst: &str) -> Result<(), BridgeError> {
        match self {
            ForeignVolume::Fat(v) => v.rename(src, dst),
            ForeignVolume::Exfat(_) => {
                Err(BridgeError::ForeignFsFailed("exFATのリネームは未対応です(上流クレートの制約)".to_string()))
            }
        }
    }
}

/// 書き込み中のファイルのバッファ(open〜releaseの間だけ保持)。
struct WriteBuffer {
    data: Vec<u8>,
}

// SAFETY: `fatfs`クレートの`FsOptions`は`&'static dyn OemCpConverter`/
// `&'static dyn TimeProvider`をトレイトオブジェクト参照として保持するが、
// これらのトレイト自体が`Sync`をスーパートレイトとして要求していないため、
// 具体的な実装(既定の`LossyOemCpConverter`/`DefaultTimeProvider`、いずれも
// 内部状態を持たないゼロサイズ型)が実際にはスレッド間共有安全であるにも
// 関わらず、型システム上は`ForeignFatVolume`(延いては`ForeignVolume`)が
// `Sync`と判定されない。`fuser::Filesystem`トレイトは`Send + Sync +
// 'static`を無条件に要求するため、このままではビルドできない。
//
// `ForeignFuseState`への全アクセスは常に`Mutex`経由(本ファイルの
// `ForeignFuseFilesystem::state`)であり、複数スレッドから同時に内部の
// `FileSystem`/`ExFatFs`が並行アクセスされることは無い(`Mutex`が排他制御
// する)。したがって、たとえ理論上任意の`OemCpConverter`/`TimeProvider`
// 実装がスレッド非安全であっても、実際に複数スレッドから同時アクセスされる
// ことはなく、`unsafe impl Sync`は安全に成立する。同じ理由で`Send`(所有権
// ごと別スレッドへ渡すこと自体)も安全なため、`unsafe impl Send`も付与する
// (`spawn_mount2`が`Filesystem + Send`を要求するため必要)。
unsafe impl Sync for ForeignVolume {}
unsafe impl Send for ForeignVolume {}

struct ForeignFuseState {
    volume: ForeignVolume,
    path_to_ino: HashMap<String, u64>,
    ino_to_path: HashMap<u64, String>,
    next_ino: u64,
    /// ファイルハンドル(=inode番号を流用)ごとの書き込みバッファ。
    write_buffers: HashMap<u64, WriteBuffer>,
}

impl ForeignFuseState {
    fn new(volume: ForeignVolume) -> Self {
        let mut path_to_ino = HashMap::new();
        let mut ino_to_path = HashMap::new();
        path_to_ino.insert(String::new(), ROOT_INO);
        ino_to_path.insert(ROOT_INO, String::new());
        Self { volume, path_to_ino, ino_to_path, next_ino: ROOT_INO + 1, write_buffers: HashMap::new() }
    }

    fn ino_for_path(&mut self, path: &str) -> u64 {
        if let Some(&ino) = self.path_to_ino.get(path) {
            return ino;
        }
        let ino = self.next_ino;
        self.next_ino += 1;
        self.path_to_ino.insert(path.to_string(), ino);
        self.ino_to_path.insert(ino, path.to_string());
        ino
    }

    fn path_for_ino(&self, ino: u64) -> Option<&str> {
        self.ino_to_path.get(&ino).map(|s| s.as_str())
    }

    fn forget_path(&mut self, path: &str) {
        if let Some(ino) = self.path_to_ino.remove(path) {
            self.ino_to_path.remove(&ino);
        }
    }

    fn rename_path(&mut self, old_path: &str, new_path: &str) {
        if let Some(ino) = self.path_to_ino.remove(old_path) {
            self.ino_to_path.insert(ino, new_path.to_string());
            self.path_to_ino.insert(new_path.to_string(), ino);
        }
    }

    /// 親パス+子の名前から子のフルパスを組み立てる(常に`/`区切り、先頭`/`無し)。
    fn join(parent: &str, name: &str) -> String {
        if parent.is_empty() {
            name.to_string()
        } else {
            format!("{parent}/{name}")
        }
    }

    /// 親パスを取り除いたベース名を返す(readdir表示・lookup応答用)。
    fn basename(path: &str) -> &str {
        Path::new(path).file_name().and_then(|s| s.to_str()).unwrap_or(path)
    }
}

/// `ForeignVolume`をLinux上へマウントするファイルシステム実装。
pub struct ForeignFuseFilesystem {
    state: Mutex<ForeignFuseState>,
}

impl ForeignFuseFilesystem {
    pub fn new(volume: ForeignVolume) -> Self {
        Self { state: Mutex::new(ForeignFuseState::new(volume)) }
    }

    fn dir_attr(ino: u64) -> FileAttr {
        let now = SystemTime::now();
        FileAttr {
            ino: INodeNo(ino),
            size: 0,
            blocks: 0,
            atime: now,
            mtime: now,
            ctime: now,
            crtime: now,
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 2,
            uid: 0,
            gid: 0,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        }
    }

    fn file_attr(ino: u64, size: u64) -> FileAttr {
        let now = SystemTime::now();
        FileAttr {
            ino: INodeNo(ino),
            size,
            blocks: size.div_ceil(512),
            atime: now,
            mtime: now,
            ctime: now,
            crtime: now,
            kind: FileType::RegularFile,
            perm: 0o644,
            nlink: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        }
    }

}

/// `volume`を`mount_point`(既存の空ディレクトリ)へ実際にマウントする。
/// `crate::fuse_mount::mount_pool`と同じ運用形: 戻り値の`BackgroundSession`を
/// `.join()`(または drop)することでマウントを解除できる。
pub fn mount_foreign_volume(volume: ForeignVolume, mount_point: &str) -> std::io::Result<fuser::BackgroundSession> {
    let fs = ForeignFuseFilesystem::new(volume);
    let mut config = fuser::Config::default();
    config.mount_options = vec![MountOption::FSName("open_raid_z_foreign".to_string()), MountOption::RW];
    fuser::spawn_mount2(fs, mount_point, &config)
}

impl Filesystem for ForeignFuseFilesystem {
    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let Some(name_str) = name.to_str() else {
            reply.error(Errno::EINVAL);
            return;
        };
        let mut state = self.state.lock().expect("ボリュームのロックに失敗しました");
        let Some(parent_path) = state.path_for_ino(parent.0).map(|s| s.to_string()) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let child_path = ForeignFuseState::join(&parent_path, name_str);

        let entries = match state.volume.list_dir(&parent_path) {
            Ok(e) => e,
            Err(e) => {
                reply.error(errno_from_bridge_error(&e));
                return;
            }
        };
        let Some(entry) = entries.iter().find(|e| e.name == name_str) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let ino = state.ino_for_path(&child_path);
        let attr =
            if entry.is_dir { Self::dir_attr(ino) } else { Self::file_attr(ino, entry.size_bytes) };
        reply.entry(&ATTR_TTL, &attr, Generation(0));
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        if ino.0 == ROOT_INO {
            reply.attr(&ATTR_TTL, &Self::dir_attr(ROOT_INO));
            return;
        }
        let state = self.state.lock().expect("ボリュームのロックに失敗しました");
        let Some(path) = state.path_for_ino(ino.0) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let parent = Path::new(path).parent().and_then(|p| p.to_str()).unwrap_or("").to_string();
        let base = ForeignFuseState::basename(path);
        match state.volume.list_dir(&parent) {
            Ok(entries) => match entries.iter().find(|e| e.name == base) {
                Some(entry) => {
                    let attr = if entry.is_dir {
                        Self::dir_attr(ino.0)
                    } else {
                        Self::file_attr(ino.0, entry.size_bytes)
                    };
                    reply.attr(&ATTR_TTL, &attr);
                }
                None => reply.error(Errno::ENOENT),
            },
            Err(e) => reply.error(errno_from_bridge_error(&e)),
        }
    }

    fn readdir(&self, _req: &Request, ino: INodeNo, _fh: FileHandle, offset: u64, mut reply: ReplyDirectory) {
        let mut state = self.state.lock().expect("ボリュームのロックに失敗しました");
        let Some(path) = state.path_for_ino(ino.0).map(|s| s.to_string()) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let entries = match state.volume.list_dir(&path) {
            Ok(e) => e,
            Err(e) => {
                reply.error(errno_from_bridge_error(&e));
                return;
            }
        };

        let mut all: Vec<(u64, FileType, String)> = vec![
            (ROOT_INO, FileType::Directory, ".".to_string()),
            (ROOT_INO, FileType::Directory, "..".to_string()),
        ];
        for entry in &entries {
            let child_path = ForeignFuseState::join(&path, &entry.name);
            let child_ino = state.ino_for_path(&child_path);
            let kind = if entry.is_dir { FileType::Directory } else { FileType::RegularFile };
            all.push((child_ino, kind, entry.name.clone()));
        }

        for (i, (ino, kind, name)) in all.into_iter().enumerate().skip(offset as usize) {
            if reply.add(INodeNo(ino), (i + 1) as u64, kind, name) {
                break;
            }
        }
        reply.ok();
    }

    fn open(&self, _req: &Request, ino: INodeNo, _flags: fuser::OpenFlags, reply: ReplyOpen) {
        reply.opened(FileHandle(ino.0), fuser::FopenFlags::empty());
    }

    #[allow(clippy::too_many_arguments)]
    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: fuser::OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        reply: ReplyData,
    ) {
        let state = self.state.lock().expect("ボリュームのロックに失敗しました");
        let Some(path) = state.path_for_ino(ino.0) else {
            reply.error(Errno::ENOENT);
            return;
        };
        match state.volume.read_file(path) {
            Ok(data) => {
                let offset = offset as usize;
                if offset >= data.len() {
                    reply.data(&[]);
                    return;
                }
                let end = (offset + size as usize).min(data.len());
                reply.data(&data[offset..end]);
            }
            Err(e) => reply.error(errno_from_bridge_error(&e)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn write(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: fuser::WriteFlags,
        _flags: fuser::OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        reply: ReplyWrite,
    ) {
        let mut state = self.state.lock().expect("ボリュームのロックに失敗しました");
        // 初回の書き込みでは、既存内容(あれば)をバッファへ読み込んでおく
        // (追記・部分書き込みでも既存内容を壊さないようにするため)。
        if !state.write_buffers.contains_key(&fh.0) {
            let path = state.path_for_ino(ino.0).unwrap_or("").to_string();
            let existing = state.volume.read_file(&path).unwrap_or_default();
            state.write_buffers.insert(fh.0, WriteBuffer { data: existing });
        }
        let buf = state.write_buffers.get_mut(&fh.0).expect("直前に確保済み");
        let offset = offset as usize;
        let end = offset + data.len();
        if buf.data.len() < end {
            buf.data.resize(end, 0);
        }
        buf.data[offset..end].copy_from_slice(data);
        reply.written(data.len() as u32);
    }

    fn release(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        _flags: fuser::OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        let mut state = self.state.lock().expect("ボリュームのロックに失敗しました");
        if let Some(buf) = state.write_buffers.remove(&fh.0) {
            let path = state.path_for_ino(ino.0).unwrap_or("").to_string();
            if let Err(e) = state.volume.write_file(&path, &buf.data) {
                reply.error(errno_from_bridge_error(&e));
                return;
            }
        }
        reply.ok();
    }

    fn create(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        let Some(name_str) = name.to_str() else {
            reply.error(Errno::EINVAL);
            return;
        };
        let mut state = self.state.lock().expect("ボリュームのロックに失敗しました");
        let Some(parent_path) = state.path_for_ino(parent.0).map(|s| s.to_string()) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let child_path = ForeignFuseState::join(&parent_path, name_str);
        if let Err(e) = state.volume.write_file(&child_path, &[]) {
            reply.error(errno_from_bridge_error(&e));
            return;
        }
        let ino = state.ino_for_path(&child_path);
        state.write_buffers.insert(ino, WriteBuffer { data: Vec::new() });
        reply.created(&ATTR_TTL, &Self::file_attr(ino, 0), Generation(0), FileHandle(ino), fuser::FopenFlags::empty());
    }

    fn mkdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, _mode: u32, _umask: u32, reply: ReplyEntry) {
        let Some(name_str) = name.to_str() else {
            reply.error(Errno::EINVAL);
            return;
        };
        let mut state = self.state.lock().expect("ボリュームのロックに失敗しました");
        let Some(parent_path) = state.path_for_ino(parent.0).map(|s| s.to_string()) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let child_path = ForeignFuseState::join(&parent_path, name_str);
        if let Err(e) = state.volume.create_dir(&child_path) {
            reply.error(errno_from_bridge_error(&e));
            return;
        }
        let ino = state.ino_for_path(&child_path);
        reply.entry(&ATTR_TTL, &Self::dir_attr(ino), Generation(0));
    }

    fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        self.remove_common(parent, name, reply);
    }

    fn rmdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        self.remove_common(parent, name, reply);
    }

    fn rename(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        new_parent: INodeNo,
        new_name: &OsStr,
        _flags: fuser::RenameFlags,
        reply: ReplyEmpty,
    ) {
        let (Some(name_str), Some(new_name_str)) = (name.to_str(), new_name.to_str()) else {
            reply.error(Errno::EINVAL);
            return;
        };
        let mut state = self.state.lock().expect("ボリュームのロックに失敗しました");
        let (Some(parent_path), Some(new_parent_path)) = (
            state.path_for_ino(parent.0).map(|s| s.to_string()),
            state.path_for_ino(new_parent.0).map(|s| s.to_string()),
        ) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let old_path = ForeignFuseState::join(&parent_path, name_str);
        let new_path = ForeignFuseState::join(&new_parent_path, new_name_str);
        match state.volume.rename(&old_path, &new_path) {
            Ok(()) => {
                state.rename_path(&old_path, &new_path);
                reply.ok();
            }
            Err(e) => reply.error(errno_from_bridge_error(&e)),
        }
    }
}

impl ForeignFuseFilesystem {
    fn remove_common(&self, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let Some(name_str) = name.to_str() else {
            reply.error(Errno::EINVAL);
            return;
        };
        let mut state = self.state.lock().expect("ボリュームのロックに失敗しました");
        let Some(parent_path) = state.path_for_ino(parent.0).map(|s| s.to_string()) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let child_path = ForeignFuseState::join(&parent_path, name_str);
        match state.volume.remove(&child_path) {
            Ok(()) => {
                state.forget_path(&child_path);
                reply.ok();
            }
            Err(e) => reply.error(errno_from_bridge_error(&e)),
        }
    }
}
