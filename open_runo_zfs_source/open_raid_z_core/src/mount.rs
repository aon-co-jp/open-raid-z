//! WinFsp実マウント(プロトタイプ / ひな型)。
//!
//! `fs_ops.rs`のトレイト骨組みを、実際にWindows上へドライブレターとして
//! マウントできる`winfsp`クレート連携へ置き換えたもの。
//!
//! 【現状のスコープ】
//! - ルート直下に、[`Pool`]の全データセットが、それぞれ1つのファイル
//!   (`\<データセット名>`)として並ぶフラットな名前空間(サブディレクトリは
//!   引き続き未対応)。ファイルの作成(`create`)・削除(`set_delete`+
//!   `cleanup`)・名前変更(`rename`)はマウント経由でサポートする
//!   (詳細は各メソッドのドキュメント参照)。
//! - 読み書きは[`Pool::read_unaligned`]/[`Pool::write_unaligned_growing`]
//!   (read-modify-write層)経由で行うため、バイト単位の任意オフセット・
//!   任意長のリクエストを受け付ける。書き込みが現在の論理サイズを超える
//!   場合は、通常のファイルシステムと同様に自動的にファイルが伸びる
//!   (プール自体の空き容量が尽きた場合のみエラーになる。詳細は`pool.rs`
//!   の[`Pool::write_unaligned_growing`]参照)。
//! - データセット名はそのままファイル名として使うため、Windowsのファイル名
//!   として不正な文字(`\ / : * ? " < > |`)を含む名前は使えない
//!   (ZFSの`pool/child`のような階層名はこの制約に抵触するため、この段階では
//!   フラットな名前のデータセットのみを想定する)。
//!
//! これはあくまで「実際にマウントできる」ことを証明する最小のひな型であり、
//! 本格的なファイルシステムとしての完成度(ディレクトリ階層・ACL等)は
//! 今後の拡張で高めていく。

use crate::pool::Pool;
use crate::vdev::Vdev;
use std::sync::Mutex;
use widestring::{u16cstr, U16CStr};
use windows::Win32::Foundation::{
    STATUS_ACCESS_DENIED, STATUS_CANNOT_DELETE, STATUS_DATA_ERROR, STATUS_DISK_FULL,
    STATUS_END_OF_FILE, STATUS_INVALID_PARAMETER, STATUS_NOT_A_DIRECTORY, STATUS_NOT_IMPLEMENTED,
    STATUS_OBJECT_NAME_COLLISION, STATUS_OBJECT_NAME_NOT_FOUND,
};
use winfsp::filesystem::{
    DirBuffer, DirInfo, DirMarker, FileInfo, FileSecurity, FileSystemContext, OpenFileInfo,
    VolumeInfo, WideNameInfo,
};
use winfsp::host::{FileSystemHost, FileSystemParams, VolumeParams};
use winfsp::{winfsp_init, FspError, FspInit, Result as FspResult};

const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
/// `NtCreateFile`の`CreateOptions`のうち、このファイルシステムが実際に
/// 見るのは「ディレクトリとして作成せよ」を意味するこのフラグのみ
/// (ルート以外のディレクトリ作成はサポートしないため拒否に使う)。
const FILE_DIRECTORY_FILE: u32 = 0x0000_0001;
/// `cleanup`の`flags`引数のうち、「(事前の`set_delete`により)このハンドルの
/// クローズをもって実際に削除する」ことを意味するビット。
const FSP_CLEANUP_DELETE: u32 = 1;
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

/// ブリッジ層のエラーを、対応するWinFsp向けNTSTATUSへ変換する。
///
/// 以前は`BridgeError`の全variantが実装側で`Io`(汎用I/Oエラー)に潰されて
/// いたため、ここでも一律「予期しないI/Oエラー」しか返せなかった。
/// エラー側の整理([`crate::error::BridgeError`]参照)により、
/// 「見つからない」「既に存在する」「容量不足」「復旧不能」等を
/// それぞれ対応するNTSTATUSへ個別に変換できるようになった。
fn status_from_bridge_error(e: &crate::error::BridgeError) -> i32 {
    use crate::error::BridgeError;
    match e {
        BridgeError::PoolNotFound(_) | BridgeError::DatasetNotFound(_) | BridgeError::SnapshotNotFound(_) => {
            STATUS_OBJECT_NAME_NOT_FOUND.0
        }
        BridgeError::AlreadyExists(_) => STATUS_OBJECT_NAME_COLLISION.0,
        BridgeError::CapacityExceeded(_) => STATUS_DISK_FULL.0,
        BridgeError::InvalidConfig(_) => STATUS_INVALID_PARAMETER.0,
        // 冗長性を超えた同時故障によるデータ消失。「デバイスエラー」ではなく
        // 「データそのものの破損・消失」を意味するSTATUS_DATA_ERRORが実態に近い。
        BridgeError::Unrecoverable(_) => STATUS_DATA_ERROR.0,
        BridgeError::NotImplemented(_) => STATUS_NOT_IMPLEMENTED.0,
        // ACL/exFAT変換やその他のI/Oエラーは、現時点ではまだ個別のNTSTATUSへ
        // 分類していない(これらはWinFsp層の主経路であるread/write/open/
        // read_directory等からは実質的に発生しないため優先度が低い)。
        BridgeError::MountFailed(_) | BridgeError::AclTranslationFailed(_) | BridgeError::ExFatConversionFailed(_) | BridgeError::Io(_) => {
            0xC00000E9u32 as i32 // STATUS_UNEXPECTED_IO_ERROR
        }
    }
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
        write_to_eof: bool,
        constrained_io: bool,
        file_info: &mut FileInfo,
    ) -> FspResult<u32> {
        let FileHandle::DataFile(name) = context else {
            return Err(FspError::NTSTATUS(STATUS_NOT_A_DIRECTORY.0));
        };
        let mut pool = self.pool.lock().expect("プールのロックに失敗しました");
        let current_size = pool
            .dataset_size(name)
            .map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
        // write_to_eof: FILE_APPEND_DATAで開かれたハンドルからの書き込み。
        // 渡された`offset`は無視し、現在の末尾へ書き込む。
        let effective_offset = if write_to_eof { current_size } else { offset };

        let written = if constrained_io {
            // メモリマップされたファイル等からの書き込みでは、ファイルを
            // 伸ばしてはいけない。現在のサイズをはみ出す分は切り詰める。
            if effective_offset >= current_size {
                0
            } else {
                let clamped_len = buffer.len().min((current_size - effective_offset) as usize);
                pool.write_unaligned(name, effective_offset, &buffer[..clamped_len])
                    .map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
                clamped_len
            }
        } else {
            pool.write_unaligned_growing(name, effective_offset, buffer)
                .map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
            buffer.len()
        };

        pool.save().map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
        self.fill_file_info(&pool, context, file_info)?;
        Ok(written as u32)
    }

    fn create(
        &self,
        file_name: &U16CStr,
        create_options: u32,
        _granted_access: u32,
        _file_attributes: u32,
        _security_descriptor: Option<&[std::ffi::c_void]>,
        _allocation_size: u64,
        _extra_buffer: Option<&[u8]>,
        _extra_buffer_is_reparse_point: bool,
        file_info: &mut OpenFileInfo,
    ) -> FspResult<Self::FileContext> {
        if create_options & FILE_DIRECTORY_FILE != 0 {
            // サブディレクトリの作成は未対応(モジュールドキュメント参照)。
            return Err(FspError::NTSTATUS(STATUS_NOT_IMPLEMENTED.0));
        }
        let name = Self::parse_new_top_level_name(file_name)
            .ok_or(FspError::NTSTATUS(STATUS_OBJECT_NAME_NOT_FOUND.0))?;

        let mut pool = self.pool.lock().expect("プールのロックに失敗しました");
        pool.create_dataset(&name)
            .map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
        pool.save().map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
        let handle = FileHandle::DataFile(name);
        self.fill_file_info(&pool, &handle, file_info.as_mut())?;
        Ok(handle)
    }

    fn cleanup(&self, context: &Self::FileContext, _file_name: Option<&U16CStr>, flags: u32) {
        if flags & FSP_CLEANUP_DELETE == 0 {
            return;
        }
        if let FileHandle::DataFile(name) = context {
            let mut pool = self.pool.lock().expect("プールのロックに失敗しました");
            // cleanupはエラーを返せない仕様(WinFspの制約)。set_deleteで
            // 既に削除可能と判定済みのはずなので、失敗しても無視する
            // (保存の失敗も同様。ベストエフォート)。
            if pool.destroy_dataset(name).is_ok() {
                let _ = pool.save();
            }
        }
    }

    fn set_delete(&self, context: &Self::FileContext, _file_name: &U16CStr, delete_file: bool) -> FspResult<()> {
        match context {
            // ルートは削除不可。データセットファイルは常に削除を許可し、
            // 実際の削除は(WinFspの規約どおり)cleanupで行う。
            FileHandle::Root if delete_file => Err(FspError::NTSTATUS(STATUS_CANNOT_DELETE.0)),
            _ => Ok(()),
        }
    }

    fn set_file_size(
        &self,
        context: &Self::FileContext,
        new_size: u64,
        _set_allocation_size: bool,
        file_info: &mut FileInfo,
    ) -> FspResult<()> {
        let FileHandle::DataFile(name) = context else {
            return Err(FspError::NTSTATUS(STATUS_NOT_A_DIRECTORY.0));
        };
        let mut pool = self.pool.lock().expect("プールのロックに失敗しました");
        pool.set_dataset_size(name, new_size)
            .map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
        pool.save().map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
        self.fill_file_info(&pool, context, file_info)
    }

    /// 【既知の制約】リネーム対象を指す他の(この呼び出しとは別の)オープン
    /// ハンドルが残っている場合、そのハンドルは古い名前のまま(`FileHandle`が
    /// 名前を直接保持する設計のため)以後の操作に失敗しうる。詳細は
    /// [`Pool::rename_dataset`]のドキュメント参照。
    fn rename(
        &self,
        context: &Self::FileContext,
        _file_name: &U16CStr,
        new_file_name: &U16CStr,
        replace_if_exists: bool,
    ) -> FspResult<()> {
        let FileHandle::DataFile(old_name) = context else {
            return Err(FspError::NTSTATUS(STATUS_ACCESS_DENIED.0));
        };
        let new_name = Self::parse_new_top_level_name(new_file_name)
            .ok_or(FspError::NTSTATUS(STATUS_OBJECT_NAME_NOT_FOUND.0))?;

        let mut pool = self.pool.lock().expect("プールのロックに失敗しました");
        let target_exists = pool.dataset_names().iter().any(|d| d == &new_name);
        if target_exists {
            if !replace_if_exists {
                return Err(FspError::NTSTATUS(STATUS_OBJECT_NAME_COLLISION.0));
            }
            if new_name != *old_name {
                pool.destroy_dataset(&new_name)
                    .map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
            }
        }
        pool.rename_dataset(old_name, &new_name)
            .map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))?;
        pool.save().map_err(|e| FspError::NTSTATUS(status_from_bridge_error(&e)))
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
            .acquire(marker.is_none(), Some(names.len() as u32))
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
        let stripe_bytes = pool.stripe_bytes();
        // total_size/free_sizeはバイト単位で報告する必要がある。以前は
        // ストライプ数をそのまま渡していたため、Windowsからは「容量数バイト
        // しかない極小ボリューム」に見え、実際のデータサイズを書き込もうと
        // すると即座にSTATUS_DISK_FULLになっていた(実マウントでの書き込みを
        // 一度も検証できていなかったため、これまで発覚していなかった)。
        out_volume_info.total_size = usage.total_stripes * stripe_bytes;
        out_volume_info.free_size = usage.free_stripes * stripe_bytes;
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

    /// `create`/`rename`の対象となる「ルート直下の新しい名前」を検証・抽出する。
    /// [`Self::classify`]と異なり、既存かどうかは問わない(存在確認は呼び出し側の
    /// 責務)が、`\`直下の単一階層であること・不正文字を含まないことは同様に要求する。
    fn parse_new_top_level_name(file_name: &U16CStr) -> Option<String> {
        let name = file_name.to_string().ok()?;
        let name = name.strip_prefix('\\')?;
        if name.is_empty() || name.contains(INVALID_NAME_CHARS) {
            return None;
        }
        Some(name.to_string())
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
