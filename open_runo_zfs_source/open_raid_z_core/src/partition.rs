//! 1台の物理ディスク(`BlockDevice`)を論理的に分割し、それぞれを独立した
//! `BlockDevice`として複数のvdevへ同時に所属させるための層。
//!
//! 【想定シナリオ】「1台のディスクをパーティション分割し、片方をミラー
//! (RAID1)のメンバーに、もう片方を(他3台以上と組んだ)RAID6/RAID-Z2の
//! メンバーにする」という要望に対応する。分割さえしていれば、同じ生ディスクを
//! 2つの独立したRAID配列が同時に読み書きしても、互いのバイト範囲を侵さない
//! ため安全である(パーティション無しに同じ生ディスクを2つの配列へ
//! 同時所属させることは、両配列が同じバイト列を独立に上書きし合い
//! データ破損するため、技術的に成立しない)。
//!
//! `PartitionedDevice`は内部で`Arc<Mutex<D>>`により実デバイスを共有し、
//! 各パーティションは自分の担当範囲(`start_offset`..`start_offset+size`)への
//! アクセスだけに変換して委譲する。
//!
//! 【`Rc<RefCell<D>>`ではなく`Arc<Mutex<D>>`を使う理由】
//! `mount.rs::mount_pool`は`V: Vdev + Send + Sync`を要求する(WinFspが
//! ファイルシステム要求をワーカースレッドから呼び出すため)。以前は
//! `Rc<RefCell<D>>`で実装しており、`Rc`は`Send`/`Sync`のどちらも満たさない
//! ため、パーティション分割したディスクを含む`Pool`は**原理的にWinFsp経由で
//! マウントできない**という、README記載の2つの目玉機能(ディスクの
//! パーティション分割・使い回し / 実際のWinFspマウント)が組み合わせ不可能
//! になっているバグがあった。`Arc<Mutex<D>>`(`D: Send`であれば
//! `Arc<Mutex<D>>: Send + Sync`)へ変更することでこれを解消している。

use crate::block_device::BlockDevice;
use crate::error::{BridgeError, BridgeResult};
use std::sync::{Arc, Mutex};

/// 1台の実デバイスを共有する、範囲制限付きの論理パーティション。
pub struct PartitionedDevice<D: BlockDevice> {
    inner: Arc<Mutex<D>>,
    start_offset: u64,
    size: u64,
}

impl<D: BlockDevice> PartitionedDevice<D> {
    pub fn size(&self) -> u64 {
        self.size
    }

    fn check_bounds(&self, offset: u64, len: u64) -> BridgeResult<()> {
        let end = offset.checked_add(len).ok_or_else(|| out_of_bounds("オフセット計算がオーバーフローしました"))?;
        if end > self.size {
            return Err(out_of_bounds(&format!(
                "パーティション範囲外へのアクセスです(要求: {offset}..{end}, パーティションサイズ: {})",
                self.size
            )));
        }
        Ok(())
    }

    fn lock_inner(&self) -> BridgeResult<std::sync::MutexGuard<'_, D>> {
        self.inner.lock().map_err(|_| {
            out_of_bounds("パーティション共有元デバイスのロックが破損しています(他スレッドがパニックしました)")
        })
    }
}

fn out_of_bounds(msg: &str) -> BridgeError {
    BridgeError::Io(std::io::Error::other(msg.to_string()))
}

impl<D: BlockDevice> BlockDevice for PartitionedDevice<D> {
    fn read_at(&mut self, offset: u64, len: usize) -> BridgeResult<Vec<u8>> {
        self.check_bounds(offset, len as u64)?;
        self.lock_inner()?.read_at(self.start_offset + offset, len)
    }

    fn write_at(&mut self, offset: u64, data: &[u8]) -> BridgeResult<()> {
        self.check_bounds(offset, data.len() as u64)?;
        self.lock_inner()?.write_at(self.start_offset + offset, data)
    }
}

/// 1台の実デバイス`device`を、`sizes`で指定した連続する区間へ分割する
/// (`sizes[0]`が先頭パーティション、以降オフセットなしで連結)。
/// 各パーティションは独立した`BlockDevice`として、別々のvdevへそれぞれ
/// 渡すことができる。
pub fn partition_device<D: BlockDevice>(device: D, sizes: &[u64]) -> Vec<PartitionedDevice<D>> {
    let shared = Arc::new(Mutex::new(device));
    let mut partitions = Vec::with_capacity(sizes.len());
    let mut offset = 0u64;
    for &size in sizes {
        partitions.push(PartitionedDevice { inner: Arc::clone(&shared), start_offset: offset, size });
        offset += size;
    }
    partitions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_device::FileBackedDevice;

    fn scratch_disk(name: &str, size: u64) -> FileBackedDevice {
        let path = std::env::temp_dir().join(format!("open_runo_partition_test_{name}_{}", std::process::id()));
        FileBackedDevice::create_fixed_size(&path, size).unwrap()
    }

    #[test]
    fn partitions_are_independently_readable_and_writable() {
        let disk = scratch_disk("basic", 200);
        let mut parts = partition_device(disk, &[100, 100]);
        let (mut a, mut b) = (parts.remove(0), parts.remove(0));

        a.write_at(0, b"hello from partition A").unwrap();
        b.write_at(0, b"hello from partition B").unwrap();

        assert_eq!(a.read_at(0, 22).unwrap(), b"hello from partition A");
        assert_eq!(b.read_at(0, 22).unwrap(), b"hello from partition B");
    }

    #[test]
    fn writes_to_one_partition_do_not_leak_into_another() {
        let disk = scratch_disk("isolation", 128);
        let mut parts = partition_device(disk, &[64, 64]);
        let (mut a, mut b) = (parts.remove(0), parts.remove(0));

        a.write_at(0, &[0xFFu8; 64]).unwrap();
        // パーティションBは別区間(オフセット64〜)を指しているため、
        // ゼロ初期化されたままのはず。
        assert_eq!(b.read_at(0, 64).unwrap(), vec![0u8; 64]);
    }

    #[test]
    fn out_of_bounds_access_is_rejected() {
        let disk = scratch_disk("bounds", 100);
        let mut parts = partition_device(disk, &[50, 50]);
        let mut a = parts.remove(0);

        assert!(a.read_at(40, 20).is_err(), "パーティションサイズ50を超える読み込みは拒否されるべき");
        assert!(a.write_at(45, &[0u8; 10]).is_err());
    }

    #[test]
    fn partition_size_is_reported_correctly() {
        let disk = scratch_disk("size", 300);
        let parts = partition_device(disk, &[100, 200]);
        assert_eq!(parts[0].size(), 100);
        assert_eq!(parts[1].size(), 200);
    }

    /// `mount.rs::mount_pool`は`V: Vdev + Send + Sync`を要求する(WinFspが
    /// ファイルシステム要求を任意のワーカースレッドから呼び出すため)。
    /// 以前`PartitionedDevice`が`Rc<RefCell<D>>`を使っており`Send`/`Sync`の
    /// どちらも満たせなかった(=パーティション分割したディスクを含む`Pool`は
    /// 原理的にWinFsp経由でマウントできなかった)ことの再発防止テスト。
    /// `mount.rs`自体は`winfsp_backend`feature配下(かつ実際のWinFsp SDKが
    /// 必要)でこの環境ではコンパイルを確認できないため、同じ境界条件
    /// (`V: Vdev + Send + Sync`)だけをここで独立に検証しておく。
    #[test]
    fn partitioned_device_backed_vdev_satisfies_mount_pools_send_sync_bound() {
        fn assert_mount_pool_compatible<V: crate::vdev::Vdev + Send + Sync>() {}

        assert_mount_pool_compatible::<crate::vdev::RaidZVdev<PartitionedDevice<FileBackedDevice>>>();
    }
}
