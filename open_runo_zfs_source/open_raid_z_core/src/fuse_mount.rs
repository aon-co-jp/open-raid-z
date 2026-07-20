//! Linux実マウント(`mount.rs`のWinFsp版に対応する、FUSE版)。
//!
//! `winfsp`クレートがWindows上へドライブレターとしてマウントする役割を担うのと
//! 同様に、`fuser`クレート(Linuxの[FUSE](https://www.kernel.org/doc/html/latest/filesystems/fuse.html)
//! への実Rustバインディング)を使って、[`Pool`]をLinux上の通常のディレクトリへ
//! マウント可能にする。
//!
//! 【現状のスコープ】
//! - `mount.rs`と同じく、マウントポイント直下に[`Pool`]の全データセットが
//!   それぞれ1つのファイル(`<データセット名>`)として並ぶフラットな名前空間
//!   (サブディレクトリは未対応)。ファイルの作成・削除・名前変更・追記・
//!   任意オフセット読み書き・切り詰めに対応する。
//! - データセット名の制約(Windowsのファイル名として不正な文字を含む名前を
//!   拒否する)は、Linux単体では本来不要だが、**同じプールを将来Windows側
//!   (`mount.rs`)からもマウントする可能性がある**ため、`mount.rs`と同一の
//!   制約をあえて課している(どちらのOSでマウントしても同じデータセット群が
//!   同じように見える、という一貫性を優先した設計判断)。
//! - FUSEはinode番号でファイルを識別するため、`mount.rs`の`FileHandle`
//!   (データセット名を直接保持)とは異なり、こちらは「データセット名 <-> inode
//!   番号」の対応表を持つ。これにより`mount.rs`の既知の制約(リネーム中に
//!   他のオープンハンドルが古い名前を参照し続けて失敗しうる)が**この実装には
//!   存在しない**(リネーム後もinode番号自体は変わらないため、既存のオープン
//!   ハンドルは引き続き正しいデータセットを指し続ける)。

use crate::error::BridgeError;
use crate::pool::Pool;
use crate::vdev::Vdev;
use fuser::{
    Errno, FileAttr, FileHandle, FileType, Filesystem, FopenFlags, Generation, INodeNo, MountOption, ReplyAttr,
    ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, Request,
};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// ルートディレクトリのinode番号(FUSEの慣例どおり1固定)。
const ROOT_INO: u64 = 1;
/// 属性キャッシュのTTL。この実装はマウント元のプロセス自身がすべての
/// 書き込みを仲介するため長くしても安全だが、控えめな値にしておく。
const ATTR_TTL: Duration = Duration::from_secs(1);
/// Windowsのファイル名として不正な文字。`mount.rs`の同名の制約と揃えている
/// (理由はモジュールドキュメント参照)。
const INVALID_NAME_CHARS: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

fn errno_from_bridge_error(e: &BridgeError) -> Errno {
    match e {
        BridgeError::PoolNotFound(_) | BridgeError::DatasetNotFound(_) | BridgeError::SnapshotNotFound(_) => {
            Errno::ENOENT
        }
        BridgeError::AlreadyExists(_) => Errno::EEXIST,
        BridgeError::CapacityExceeded(_) => Errno::ENOSPC,
        BridgeError::InvalidConfig(_) => Errno::EINVAL,
        BridgeError::Unrecoverable(_) => Errno::EIO,
        BridgeError::NotImplemented(_) => Errno::ENOSYS,
        BridgeError::MountFailed(_)
        | BridgeError::AclTranslationFailed(_)
        | BridgeError::ExFatConversionFailed(_)
        | BridgeError::ForeignFsFailed(_)
        | BridgeError::Io(_) => Errno::EIO,
    }
}

/// データセット名とinode番号の対応、および[`Pool`]本体をまとめて保持する。
/// `mount.rs`と異なり名前ではなくinode番号がファイルの恒久的な識別子なので、
/// リネーム後もinode番号は変わらない(モジュールドキュメント参照)。
struct PoolState<V: Vdev> {
    pool: Pool<V>,
    name_to_ino: HashMap<String, u64>,
    ino_to_name: HashMap<u64, String>,
    next_ino: u64,
}

impl<V: Vdev> PoolState<V> {
    fn new(pool: Pool<V>) -> Self {
        Self { pool, name_to_ino: HashMap::new(), ino_to_name: HashMap::new(), next_ino: ROOT_INO + 1 }
    }

    /// 名前に対応するinode番号を返す(無ければ新規に払い出す)。
    fn ino_for_name(&mut self, name: &str) -> u64 {
        if let Some(&ino) = self.name_to_ino.get(name) {
            return ino;
        }
        let ino = self.next_ino;
        self.next_ino += 1;
        self.name_to_ino.insert(name.to_string(), ino);
        self.ino_to_name.insert(ino, name.to_string());
        ino
    }

    fn name_for_ino(&self, ino: u64) -> Option<&str> {
        self.ino_to_name.get(&ino).map(|s| s.as_str())
    }

    fn forget_name(&mut self, name: &str) {
        if let Some(ino) = self.name_to_ino.remove(name) {
            self.ino_to_name.remove(&ino);
        }
    }

    fn rename_name(&mut self, old_name: &str, new_name: &str) {
        if let Some(ino) = self.name_to_ino.remove(old_name) {
            self.ino_to_name.insert(ino, new_name.to_string());
            self.name_to_ino.insert(new_name.to_string(), ino);
        }
    }

    fn is_exposable(name: &str) -> bool {
        !name.is_empty() && !name.contains(INVALID_NAME_CHARS)
    }
}

/// `Pool<V>`が保持する全データセットを、マウントポイント直下のファイル群として
/// FUSE経由でマウント可能にするファイルシステム実装。
pub struct PoolFilesystem<V: Vdev> {
    state: Mutex<PoolState<V>>,
}

impl<V: Vdev> PoolFilesystem<V> {
    pub fn new(pool: Pool<V>) -> Self {
        Self { state: Mutex::new(PoolState::new(pool)) }
    }

    fn root_attr() -> FileAttr {
        let now = SystemTime::now();
        FileAttr {
            ino: INodeNo(ROOT_INO),
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

impl<V: Vdev + Send + Sync + 'static> Filesystem for PoolFilesystem<V> {
    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        if parent.0 != ROOT_INO {
            reply.error(Errno::ENOTDIR);
            return;
        }
        let Some(name_str) = name.to_str() else {
            reply.error(Errno::EINVAL);
            return;
        };
        let mut state = self.state.lock().expect("プールのロックに失敗しました");
        if !PoolState::<V>::is_exposable(name_str) || !state.pool.dataset_names().iter().any(|d| d == name_str) {
            reply.error(Errno::ENOENT);
            return;
        }
        let size = match state.pool.dataset_size(name_str) {
            Ok(s) => s,
            Err(e) => {
                reply.error(errno_from_bridge_error(&e));
                return;
            }
        };
        let ino = state.ino_for_name(name_str);
        reply.entry(&ATTR_TTL, &Self::file_attr(ino, size), Generation(0));
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        if ino.0 == ROOT_INO {
            reply.attr(&ATTR_TTL, &Self::root_attr());
            return;
        }
        let state = self.state.lock().expect("プールのロックに失敗しました");
        let Some(name) = state.name_for_ino(ino.0) else {
            reply.error(Errno::ENOENT);
            return;
        };
        match state.pool.dataset_size(name) {
            Ok(size) => reply.attr(&ATTR_TTL, &Self::file_attr(ino.0, size)),
            Err(e) => reply.error(errno_from_bridge_error(&e)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn setattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<FileHandle>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<fuser::BsdFileFlags>,
        reply: ReplyAttr,
    ) {
        if ino.0 == ROOT_INO {
            reply.attr(&ATTR_TTL, &Self::root_attr());
            return;
        }
        let mut state = self.state.lock().expect("プールのロックに失敗しました");
        let Some(name) = state.name_for_ino(ino.0).map(str::to_string) else {
            reply.error(Errno::ENOENT);
            return;
        };
        // `size`の変更(truncate/ftruncate相当)のみ扱う。mode/uid/gid/timesは無視する
        // (通常のファイルシステムほどの属性管理は現時点でのスコープ外)。
        if let Some(new_size) = size {
            if let Err(e) = state.pool.set_dataset_size(&name, new_size) {
                reply.error(errno_from_bridge_error(&e));
                return;
            }
            if let Err(e) = state.pool.save() {
                reply.error(errno_from_bridge_error(&e));
                return;
            }
        }
        match state.pool.dataset_size(&name) {
            Ok(current_size) => reply.attr(&ATTR_TTL, &Self::file_attr(ino.0, current_size)),
            Err(e) => reply.error(errno_from_bridge_error(&e)),
        }
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
        let mut state = self.state.lock().expect("プールのロックに失敗しました");
        let Some(name) = state.name_for_ino(ino.0).map(str::to_string) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let dataset_size = match state.pool.dataset_size(&name) {
            Ok(s) => s,
            Err(e) => {
                reply.error(errno_from_bridge_error(&e));
                return;
            }
        };
        if offset >= dataset_size {
            reply.data(&[]);
            return;
        }
        let len = (size as u64).min(dataset_size - offset);
        match state.pool.read_unaligned(&name, offset, len) {
            Ok(data) => reply.data(&data),
            Err(e) => reply.error(errno_from_bridge_error(&e)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn write(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: fuser::WriteFlags,
        _flags: fuser::OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        reply: ReplyWrite,
    ) {
        let mut state = self.state.lock().expect("プールのロックに失敗しました");
        let Some(name) = state.name_for_ino(ino.0).map(str::to_string) else {
            reply.error(Errno::ENOENT);
            return;
        };
        match state.pool.write_unaligned_growing(&name, offset, data).and_then(|()| state.pool.save()) {
            Ok(()) => reply.written(data.len() as u32),
            Err(e) => reply.error(errno_from_bridge_error(&e)),
        }
    }

    fn open(&self, _req: &Request, ino: INodeNo, _flags: fuser::OpenFlags, reply: ReplyOpen) {
        if ino.0 == ROOT_INO {
            reply.error(Errno::EISDIR);
            return;
        }
        let state = self.state.lock().expect("プールのロックに失敗しました");
        if state.name_for_ino(ino.0).is_none() {
            reply.error(Errno::ENOENT);
            return;
        }
        reply.opened(FileHandle(0), FopenFlags::empty());
    }

    fn opendir(&self, _req: &Request, ino: INodeNo, _flags: fuser::OpenFlags, reply: ReplyOpen) {
        if ino.0 != ROOT_INO {
            reply.error(Errno::ENOTDIR);
            return;
        }
        reply.opened(FileHandle(0), FopenFlags::empty());
    }

    fn readdir(&self, _req: &Request, ino: INodeNo, _fh: FileHandle, offset: u64, mut reply: ReplyDirectory) {
        if ino.0 != ROOT_INO {
            reply.error(Errno::ENOTDIR);
            return;
        }
        let mut state = self.state.lock().expect("プールのロックに失敗しました");
        let mut entries: Vec<(u64, FileType, String)> =
            vec![(ROOT_INO, FileType::Directory, ".".to_string()), (ROOT_INO, FileType::Directory, "..".to_string())];
        let mut names = state.pool.dataset_names();
        names.retain(|n| PoolState::<V>::is_exposable(n));
        for name in names {
            let ino = state.ino_for_name(&name);
            entries.push((ino, FileType::RegularFile, name));
        }

        for (i, (ino, kind, name)) in entries.into_iter().enumerate().skip(offset as usize) {
            // add()はバッファが一杯になるとtrueを返す。その場合は残りを次回の
            // readdir呼び出し(このoffsetから再開)に委ねる。
            if reply.add(INodeNo(ino), (i + 1) as u64, kind, &name) {
                break;
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
        if parent.0 != ROOT_INO {
            reply.error(Errno::ENOTDIR);
            return;
        }
        let Some(name_str) = name.to_str() else {
            reply.error(Errno::EINVAL);
            return;
        };
        if !PoolState::<V>::is_exposable(name_str) {
            reply.error(Errno::EINVAL);
            return;
        }
        let mut state = self.state.lock().expect("プールのロックに失敗しました");
        if let Err(e) = state.pool.create_dataset(name_str) {
            reply.error(errno_from_bridge_error(&e));
            return;
        }
        if let Err(e) = state.pool.save() {
            let _ = state.pool.destroy_dataset(name_str); // 保存に失敗したなら作成自体をロールバックする
            reply.error(errno_from_bridge_error(&e));
            return;
        }
        let ino = state.ino_for_name(name_str);
        reply.created(&ATTR_TTL, &Self::file_attr(ino, 0), Generation(0), FileHandle(0), FopenFlags::empty());
    }

    fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        if parent.0 != ROOT_INO {
            reply.error(Errno::ENOTDIR);
            return;
        }
        let Some(name_str) = name.to_str() else {
            reply.error(Errno::EINVAL);
            return;
        };
        let mut state = self.state.lock().expect("プールのロックに失敗しました");
        match state.pool.destroy_dataset(name_str).and_then(|()| state.pool.save()) {
            Ok(()) => {
                state.forget_name(name_str);
                reply.ok();
            }
            Err(e) => reply.error(errno_from_bridge_error(&e)),
        }
    }

    fn rename(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        newparent: INodeNo,
        newname: &OsStr,
        _flags: fuser::RenameFlags,
        reply: ReplyEmpty,
    ) {
        if parent.0 != ROOT_INO || newparent.0 != ROOT_INO {
            reply.error(Errno::ENOTDIR);
            return;
        }
        let (Some(old_name), Some(new_name)) = (name.to_str(), newname.to_str()) else {
            reply.error(Errno::EINVAL);
            return;
        };
        if !PoolState::<V>::is_exposable(new_name) {
            reply.error(Errno::EINVAL);
            return;
        }
        let mut state = self.state.lock().expect("プールのロックに失敗しました");
        match state.pool.rename_dataset(old_name, new_name).and_then(|()| state.pool.save()) {
            Ok(()) => {
                state.rename_name(old_name, new_name);
                reply.ok();
            }
            Err(e) => reply.error(errno_from_bridge_error(&e)),
        }
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
        let state = self.state.lock().expect("プールのロックに失敗しました");
        let usage = state.pool.usage();
        let stripe_bytes = state.pool.stripe_bytes();
        let bsize: u32 = stripe_bytes.min(u32::MAX as u64) as u32;
        reply.statfs(
            usage.total_stripes,
            usage.free_stripes,
            usage.free_stripes,
            state.name_to_ino.len() as u64,
            u64::MAX / 2,
            bsize.max(1),
            255,
            bsize.max(1),
        );
    }

    fn flush(&self, _req: &Request, _ino: INodeNo, _fh: FileHandle, _lock_owner: fuser::LockOwner, reply: ReplyEmpty) {
        reply.ok();
    }

    fn release(
        &self,
        _req: &Request,
        _ino: INodeNo,
        _fh: FileHandle,
        _flags: fuser::OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        reply.ok();
    }

    fn releasedir(&self, _req: &Request, _ino: INodeNo, _fh: FileHandle, _flags: fuser::OpenFlags, reply: ReplyEmpty) {
        reply.ok();
    }
}

/// `pool`(が保持する全データセット)を`mount_point`(既存の空ディレクトリ)へ
/// 実際にマウントする。戻り値の`BackgroundSession`をdrop(または
/// `umount_and_join`)することでマウントを解除できる。
pub fn mount_pool<V>(pool: Pool<V>, mount_point: &str) -> std::io::Result<fuser::BackgroundSession>
where
    V: Vdev + Send + Sync + 'static,
{
    let fs = PoolFilesystem::new(pool);
    let mut config = fuser::Config::default();
    config.mount_options = vec![MountOption::FSName("open_raid_z".to_string()), MountOption::RW];
    fuser::spawn_mount2(fs, mount_point, &config)
}
