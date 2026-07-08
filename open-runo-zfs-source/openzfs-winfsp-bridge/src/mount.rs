//! WinFsp実マウント(プロトタイプ / ひな型)。
//!
//! `fs_ops.rs`のトレイト骨組みを、実際にWindows上へドライブレターとして
//! マウントできる`winfsp`クレート連携へ置き換えたもの。
//!
//! 【現状のスコープ(ひな型段階)】
//! - ルート直下に固定ファイル`\pool.dat`が1つだけあるフラットな名前空間
//!   (ディレクトリ階層・複数ファイル・削除/リネームは未対応)。
//! - `\pool.dat`の実体は[`Pool`]の1データセットに対応する。
//! - 読み書きは、`Pool::read`/`Pool::write`がストライプ境界単位でしか
//!   受け付けないため、オフセット・長さともデータセットのチャンク境界
//!   (`chunk_size × num_data_disks`)に一致するリクエストのみ成功する。
//!   境界に合わないリクエストはエラーになる(将来、任意オフセットの
//!   read-modify-write用バッファリング層を追加することで解消する予定の、
//!   意図的に残した制約)。
//!
//! これはあくまで「実際にマウントできる」ことを証明する最小のひな型であり、
//! 本格的なファイルシステムとしての完成度(複数ファイル・ディレクトリ・
//! ACL・任意オフセット書き込み等)は今後の拡張で高めていく。

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

const DATA_FILE_NAME: &str = "\\pool.dat";
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;

/// マウントしたファイルシステム内で開かれているハンドルが指す対象。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileHandle {
    Root,
    DataFile,
}

/// `Pool<V>`を1つの固定ファイルとしてWinFsp経由でマウント可能にする
/// ファイルシステムコンテキスト。
pub struct PoolFileSystem<V: Vdev> {
    pool: Mutex<Pool<V>>,
    dataset_name: String,
    dataset_size: u64,
    dir_buffer: DirBuffer,
}

impl<V: Vdev> PoolFileSystem<V> {
    pub fn new(pool: Pool<V>, dataset_name: impl Into<String>) -> FspResult<Self> {
        let dataset_name = dataset_name.into();
        let dataset_size = pool
            .dataset_size(&dataset_name)
            .map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
        Ok(Self {
            pool: Mutex::new(pool),
            dataset_name,
            dataset_size,
            dir_buffer: DirBuffer::new(),
        })
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
        match classify(file_name) {
            Some(FileHandle::Root) => Ok(FileSecurity {
                reparse: false,
                sz_security_descriptor: 0,
                attributes: FILE_ATTRIBUTE_DIRECTORY,
            }),
            Some(FileHandle::DataFile) => Ok(FileSecurity {
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
        let handle = classify(file_name).ok_or(FspError::NTSTATUS(STATUS_OBJECT_NAME_NOT_FOUND.0))?;
        self.fill_file_info(handle, file_info.as_mut());
        Ok(handle)
    }

    fn close(&self, _context: Self::FileContext) {}

    fn get_file_info(&self, context: &Self::FileContext, file_info: &mut FileInfo) -> FspResult<()> {
        self.fill_file_info(*context, file_info);
        Ok(())
    }

    fn read(&self, context: &Self::FileContext, buffer: &mut [u8], offset: u64) -> FspResult<u32> {
        if *context != FileHandle::DataFile {
            return Err(FspError::NTSTATUS(STATUS_NOT_A_DIRECTORY.0));
        }
        if offset >= self.dataset_size {
            return Err(FspError::NTSTATUS(STATUS_END_OF_FILE.0));
        }
        let len = buffer.len().min((self.dataset_size - offset) as usize) as u64;
        let mut pool = self.pool.lock().expect("プールのロックに失敗しました");
        let data = pool
            .read(&self.dataset_name, offset, len)
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
        if *context != FileHandle::DataFile {
            return Err(FspError::NTSTATUS(STATUS_NOT_A_DIRECTORY.0));
        }
        let mut pool = self.pool.lock().expect("プールのロックに失敗しました");
        pool.write(&self.dataset_name, offset, buffer)
            .map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
        drop(pool);
        self.fill_file_info(FileHandle::DataFile, file_info);
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
        let lock = self
            .dir_buffer
            .acquire(marker.is_none(), Some(1))
            .map_err(|_| FspError::NTSTATUS(0xC00000E9u32 as i32))?;

        if marker.is_none() {
            let mut info: DirInfo = DirInfo::new();
            info.set_name(&DATA_FILE_NAME[1..]).ok();
            self.fill_file_info(FileHandle::DataFile, info.file_info_mut());
            lock.write(&mut info).ok();
        }
        drop(lock);
        Ok(self.dir_buffer.read(marker, buffer))
    }

    fn get_volume_info(&self, out_volume_info: &mut VolumeInfo) -> FspResult<()> {
        let pool = self.pool.lock().expect("プールのロックに失敗しました");
        let usage = pool.usage();
        // チャンクサイズが分からないため、ストライプ数をそのままバイト換算の
        // 概算として使う(vdev側にchunk_size取得が無いため近似値)。
        out_volume_info.total_size = self.dataset_size.max(usage.total_stripes);
        out_volume_info.free_size = usage.free_stripes;
        out_volume_info.set_volume_label("OpenRuno");
        Ok(())
    }
}

impl<V: Vdev> PoolFileSystem<V> {
    fn fill_file_info(&self, handle: FileHandle, file_info: &mut FileInfo) {
        *file_info = FileInfo::default();
        match handle {
            FileHandle::Root => {
                file_info.file_attributes = FILE_ATTRIBUTE_DIRECTORY;
            }
            FileHandle::DataFile => {
                file_info.file_attributes = FILE_ATTRIBUTE_NORMAL;
                file_info.file_size = self.dataset_size;
                file_info.allocation_size = self.dataset_size;
            }
        }
    }
}

fn classify(file_name: &U16CStr) -> Option<FileHandle> {
    if file_name == u16cstr!("\\") {
        Some(FileHandle::Root)
    } else if file_name.to_string().ok()?.eq_ignore_ascii_case(DATA_FILE_NAME) {
        Some(FileHandle::DataFile)
    } else {
        None
    }
}

/// WinFspを初期化し、`pool`を`mount_point`(例: `"Z:"`)へ実際にマウントする。
/// `FileSystemHost`を返すので、呼び出し側は不要になったら`drop`(または
/// `unmount`)することでマウントを解除できる。
pub fn mount_pool<V>(
    pool: Pool<V>,
    dataset_name: impl Into<String>,
    mount_point: &str,
) -> FspResult<FileSystemHost<PoolFileSystem<V>>>
where
    V: Vdev + Send + Sync,
{
    let _init: FspInit = winfsp_init()?;
    let context = PoolFileSystem::new(pool, dataset_name)?;

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
