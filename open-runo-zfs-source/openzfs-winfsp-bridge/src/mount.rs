//! WinFsp実マウント(プロトタイプ / ひな型)。
//!
//! `fs_ops.rs`のトレイト骨組みを、実際にWindows上へドライブレターとして
//! マウントできる`winfsp`クレート連携へ置き換えたもの。
//!
//! 【現状のスコープ】
//! - ルート直下に、[`Pool`]へ`create_dataset`済みの全データセットが、それぞれ
//!   1つのファイル(`\<データセット名>`)として並ぶフラットな名前空間
//!   (ディレクトリ階層・ファイル単位でのcreate/delete/rename は未対応。
//!   データセットの追加/削除自体は[`Pool::create_dataset`]/
//!   [`Pool::destroy_dataset`]をマウント外から呼ぶ運用を想定)。
//! - 読み書きは[`Pool::read_unaligned`]/[`Pool::write_unaligned`]
//!   (read-modify-write層)経由で行うため、バイト単位の任意オフセット・
//!   任意長のリクエストを受け付ける(以前の版はストライプ境界に一致する
//!   リクエストしか受け付けなかった。詳細は`pool.rs`参照)。
//!   データセットの割当容量([`Pool::grow_dataset`]で確保済みの範囲)を
//!   超えるリクエストは引き続きエラーになる(暗黙の自動拡張は行わない)。
//! - データセット名はそのままファイル名として使うため、Windowsのファイル名
//!   として不正な文字(`\ / : * ? " < > |`)を含む名前は使えない
//!   (ZFSの`pool/child`のような階層名はこの制約に抵触するため、この段階では
//!   フラットな名前のデータセットのみを想定する)。
//!
//! これはあくまで「実際にマウントできる」ことを証明する最小のひな型であり、
//! 本格的なファイルシステムとしての完成度(ディレクトリ階層・ACL・
//! 任意オフセット書き込み等)は今後の拡張で高めていく。

use crate::pool::Pool;
use crate::vdev::Vdev;
use std::sync::Mutex;
use widestring::{u16cstr, U16CStr};
use windows::Win32::Foundation::{
    STATUS_END_OF_FILE, STATUS_NOT_A_DIRECTORY, STATUS_OBJECT_NAME_NOT_FOUND,
};
use winfsp::filesystem::{
    DirBuffer, DirInfo, DirMarker, FileInfo, FileSecurity, FileSystemContext, OpenFileInfo,
    VolumeInfo, WideNameInfo,
};
use winfsp::host::{FileSystemHost, FileSystemParams, VolumeParams};
use winfsp::{winfsp_init, FspError, FspInit, Result as FspResult};

const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
/// Windowsのファイル名として使えない文字。データセット名がこれらを
/// 含む場合はマウント上に公開しない(`list_exposable_datasets`参照)。
const INVALID_NAME_CHARS: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

/// マウントしたファイルシステム内で開かれているハンドルが指す対象。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileHandle {
    Root,
    /// 対応するデータセット名。
    DataFile(String),
}

/// `Pool<V>`が保持する全データセットを、ルート直下のファイル群として
/// WinFsp経由でマウント可能にするファイルシステムコンテキスト。
pub struct PoolFileSystem<V: Vdev> {
    pool: Mutex<Pool<V>>,
    dir_buffer: DirBuffer,
}

impl<V: Vdev> PoolFileSystem<V> {
    pub fn new(pool: Pool<V>) -> FspResult<Self> {
        Ok(Self {
            pool: Mutex::new(pool),
            dir_buffer: DirBuffer::new(),
        })
    }

    /// ファイル名として公開可能な(Windowsで不正な文字を含まない)
    /// データセット名だけを、ソート済みで返す。
    fn list_exposable_datasets(pool: &Pool<V>) -> Vec<String> {
        pool.dataset_names()
            .into_iter()
            .filter(|name| !name.is_empty() && !name.contains(INVALID_NAME_CHARS))
            .collect()
    }
}

fn status_from_bridge_error(_e: &crate::error::BridgeError) -> i32 {
    // ブリッジ層のエラーはWinFsp向けの詳細なNTSTATUSへ細かく分類していないため、
    // 現時点では一律「予期しないI/Oエラー」として扱う。
    0xC00000E9u32 as i32 // STATUS_UNEXPECTED_IO_ERROR
}

impl<V: Vdev> FileSystemContext for PoolFileSystem<V> {
    type FileContext = FileHandle;

    fn get_security_by_name(
        &self,
        file_name: &U16CStr,
        _security_descriptor: Option<&mut [std::ffi::c_void]>,
        _reparse_point_resolver: impl FnOnce(&U16CStr) -> Option<FileSecurity>,
    ) -> FspResult<FileSecurity> {
        let pool = self.pool.lock().expect("プールのロックに失敗しました");
        match self.classify(&pool, file_name) {
            Some(FileHandle::Root) => Ok(FileSecurity {
                reparse: false,
                sz_security_descriptor: 0,
                attributes: FILE_ATTRIBUTE_DIRECTORY,
            }),
            Some(FileHandle::DataFile(_)) => Ok(FileSecurity {
                reparse: false,
                sz_security_descriptor: 0,
                attributes: FILE_ATTRIBUTE_NORMAL,
            }),
            None => Err(FspError::NTSTATUS(STATUS_OBJECT_NAME_NOT_FOUND.0)),
        }
    }

    fn open(
        &self,
        file_name: &U16CStr,
        _create_options: u32,
        _granted_access: u32,
        file_info: &mut OpenFileInfo,
    ) -> FspResult<Self::FileContext> {
        let pool = self.pool.lock().expect("プールのロックに失敗しました");
        let handle = self
            .classify(&pool, file_name)
            .ok_or(FspError::NTSTATUS(STATUS_OBJECT_NAME_NOT_FOUND.0))?;
        self.fill_file_info(&pool, &handle, file_info.as_mut())?;
        Ok(handle)
    }

    fn close(&self, _context: Self::FileContext) {}

    fn get_file_info(&self, context: &Self::FileContext, file_info: &mut FileInfo) -> FspResult<()> {
        let pool = self.pool.lock().expect("プールのロックに失敗しました");
        self.fill_file_info(&pool, context, file_info)
    }

    fn read(&self, context: &Self::FileContext, buffer: &mut [u8], offset: u64) -> FspResult<u32> {
        let FileHandle::DataFile(name) = context else {
            return Err(FspError::NTSTATUS(STATUS_NOT_A_DIRECTORY.0));
        };
        let mut pool = self.pool.lock().expect("プールのロックに失敗しました");
        let dataset_size = pool
            .dataset_size(name)
            .map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
        if offset >= dataset_size {
            return Err(FspError::NTSTATUS(STATUS_END_OF_FILE.0));
        }
        let len = buffer.len().min((dataset_size - offset) as usize) as u64;
        let data = pool
            .read_unaligned(name, offset, len)
            .map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
        buffer[..data.len()].copy_from_slice(&data);
        Ok(data.len() as u32)
    }

    fn write(
        &self,
        context: &Self::FileContext,
        buffer: &[u8],
        offset: u64,
        _write_to_eof: bool,
        _constrained_io: bool,
        file_info: &mut FileInfo,
    ) -> FspResult<u32> {
        let FileHandle::DataFile(name) = context else {
            return Err(FspError::NTSTATUS(STATUS_NOT_A_DIRECTORY.0));
        };
        let mut pool = self.pool.lock().expect("プールのロックに失敗しました");
        pool.write_unaligned(name, offset, buffer)
            .map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
        self.fill_file_info(&pool, context, file_info)?;
        Ok(buffer.len() as u32)
    }

    fn read_directory(
        &self,
        context: &Self::FileContext,
        _pattern: Option<&U16CStr>,
        marker: DirMarker,
        buffer: &mut [u8],
    ) -> FspResult<u32> {
        if *context != FileHandle::Root {
            return Err(FspError::NTSTATUS(STATUS_NOT_A_DIRECTORY.0));
        }
        let pool = self.pool.lock().expect("プールのロックに失敗しました");
        let names = Self::list_exposable_datasets(&pool);

        let lock = self
            .dir_buffer
            .acquire(marker.is_none(), Some(names.len()))
            .map_err(|_| FspError::NTSTATUS(0xC00000E9u32 as i32))?;

        if marker.is_none() {
            for name in &names {
                let mut info: DirInfo = DirInfo::new();
                info.set_name(name).ok();
                let handle = FileHandle::DataFile(name.clone());
                self.fill_file_info(&pool, &handle, info.file_info_mut())?;
                lock.write(&mut info).ok();
            }
        }
        drop(lock);
        Ok(self.dir_buffer.read(marker, buffer))
    }

    fn get_volume_info(&self, out_volume_info: &mut VolumeInfo) -> FspResult<()> {
        let pool = self.pool.lock().expect("プールのロックに失敗しました");
        let usage = pool.usage();
        out_volume_info.total_size = usage.total_stripes;
        out_volume_info.free_size = usage.free_stripes;
        out_volume_info.set_volume_label("OpenRuno");
        Ok(())
    }
}

impl<V: Vdev> PoolFileSystem<V> {
    fn fill_file_info(
        &self,
        pool: &Pool<V>,
        handle: &FileHandle,
        file_info: &mut FileInfo,
    ) -> FspResult<()> {
        *file_info = FileInfo::default();
        match handle {
            FileHandle::Root => {
                file_info.file_attributes = FILE_ATTRIBUTE_DIRECTORY;
            }
            FileHandle::DataFile(name) => {
                let size = pool
                    .dataset_size(name)
                    .map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
                file_info.file_attributes = FILE_ATTRIBUTE_NORMAL;
                file_info.file_size = size;
                file_info.allocation_size = size;
            }
        }
        Ok(())
    }

    /// `\`(ルート)または`\<データセット名>`をファイルハンドルへ分類する。
    /// 存在しないデータセット名、または公開不可(不正文字を含む)な
    /// データセット名は`None`を返す。
    fn classify(&self, pool: &Pool<V>, file_name: &U16CStr) -> Option<FileHandle> {
        if file_name == u16cstr!("\\") {
            return Some(FileHandle::Root);
        }
        let name = file_name.to_string().ok()?;
        let name = name.strip_prefix('\\')?;
        if name.is_empty() || name.contains(INVALID_NAME_CHARS) {
            return None;
        }
        if pool.dataset_names().iter().any(|d| d == name) {
            Some(FileHandle::DataFile(name.to_string()))
        } else {
            None
        }
    }
}

/// WinFspを初期化し、`pool`(が保持する全データセット)を`mount_point`
/// (例: `"Z:"`)へ実際にマウントする。`FileSystemHost`を返すので、
/// 呼び出し側は不要になったら`drop`(または`unmount`)することでマウントを
/// 解除できる。
pub fn mount_pool<V>(pool: Pool<V>, mount_point: &str) -> FspResult<FileSystemHost<PoolFileSystem<V>>>
where
    V: Vdev + Send + Sync,
{
    let _init: FspInit = winfsp_init()?;
    let context = PoolFileSystem::new(pool)?;

    let mut volume_params = VolumeParams::new();
    volume_params
        .sector_size(4096)
        .sectors_per_allocation_unit(1)
        .filesystem_name("OpenRunoFS")
        .case_sensitive_search(true)
        .case_preserved_names(true)
        .unicode_on_disk(true)
        .persistent_acls(false)
        .read_only_volume(false);

    let mut host = FileSystemHost::new_with_options(
        FileSystemParams {
            use_dir_info_by_name: false,
            volume_params: volume_params.clone(),
            debug_mode: Default::default(),
        },
        context,
    )?;
    host.mount(mount_point)?;
    winfsp::host::FileSystemHost::<PoolFileSystem<V>, winfsp::host::FineGuard>::start(&mut host)?;
    Ok(host)
}
