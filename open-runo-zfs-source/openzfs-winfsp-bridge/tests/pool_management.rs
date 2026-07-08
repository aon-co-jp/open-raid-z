//! ストレージプール管理のTESTモード統合テスト。
//!
//! 複数ディスクを1つのプールとしてまとめ、その中から複数のデータセット
//! (ファイルシステム)へ動的に容量を切り出す、というZFSのプール管理の
//! 特徴を、実ファイルI/Oを経由して検証する。

use openzfs_winfsp_bridge::block_device::FileBackedDevice;
use openzfs_winfsp_bridge::pool::Pool;
use openzfs_winfsp_bridge::vdev::{RaidLevel, RaidZVdev};
use std::path::PathBuf;

const CHUNK_SIZE: usize = 64;
const NUM_STRIPES: u64 = 8;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openruno_pool_it_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build_pool(dir: &std::path::Path) -> Pool<FileBackedDevice> {
    let devices: Vec<FileBackedDevice> = (0..6)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * NUM_STRIPES).unwrap()
        })
        .collect();
    let vdev = RaidZVdev::new(devices, RaidLevel::Z2, CHUNK_SIZE);
    // num_data=4なので1ストライプ=256バイト、プール容量=8ストライプ=2048バイト
    Pool::new(vdev, NUM_STRIPES)
}

fn data_disk_bytes_per_stripe() -> u64 {
    4 * CHUNK_SIZE as u64 // num_data(4) * chunk_size
}

#[test]
fn pool_carves_out_multiple_datasets_without_being_bound_to_a_single_disk() {
    let dir = scratch_dir("basic");
    let mut pool = build_pool(&dir);
    let stripe_bytes = data_disk_bytes_per_stripe();

    assert_eq!(pool.usage().total_stripes, NUM_STRIPES);
    assert_eq!(pool.usage().free_stripes, NUM_STRIPES);

    pool.create_dataset("alpha").unwrap();
    pool.create_dataset("beta").unwrap();

    // どちらのデータセットも「特定の1台のディスク容量」に縛られず、
    // プール全体の空き容量から自由に切り出せる。
    pool.grow_dataset("alpha", 3 * stripe_bytes).unwrap();
    pool.grow_dataset("beta", 3 * stripe_bytes).unwrap();

    assert_eq!(pool.dataset_size("alpha").unwrap(), 3 * stripe_bytes);
    assert_eq!(pool.dataset_size("beta").unwrap(), 3 * stripe_bytes);
    assert_eq!(pool.usage().used_stripes, 6);
    assert_eq!(pool.usage().free_stripes, 2);

    // 各データセットへの書き込みは互いに独立している(混ざらない)
    let alpha_data: Vec<u8> = (0..3 * stripe_bytes).map(|i| (i % 256) as u8).collect();
    let beta_data: Vec<u8> = (0..3 * stripe_bytes).map(|i| 255 - (i % 256) as u8).collect();
    pool.write("alpha", 0, &alpha_data).unwrap();
    pool.write("beta", 0, &beta_data).unwrap();

    assert_eq!(pool.read("alpha", 0, 3 * stripe_bytes).unwrap(), alpha_data);
    assert_eq!(pool.read("beta", 0, 3 * stripe_bytes).unwrap(), beta_data);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn growing_beyond_pool_capacity_fails_cleanly() {
    let dir = scratch_dir("overflow");
    let mut pool = build_pool(&dir);
    let stripe_bytes = data_disk_bytes_per_stripe();

    pool.create_dataset("only").unwrap();
    // プール容量(8ストライプ)を超える割当は拒否されるはず
    let result = pool.grow_dataset("only", (NUM_STRIPES + 1) * stripe_bytes);
    assert!(result.is_err());

    // プールの状態は変化していない(部分的に確保されたりしない)
    assert_eq!(pool.usage().used_stripes, 0);
    assert_eq!(pool.usage().free_stripes, NUM_STRIPES);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn destroying_a_dataset_reclaims_capacity_for_others() {
    let dir = scratch_dir("reclaim");
    let mut pool = build_pool(&dir);
    let stripe_bytes = data_disk_bytes_per_stripe();

    pool.create_dataset("temp").unwrap();
    pool.grow_dataset("temp", NUM_STRIPES * stripe_bytes).unwrap(); // プール全容量を使い切る
    assert_eq!(pool.usage().free_stripes, 0);

    // 容量を使い切っている状態では新規データセットへの割当はできない
    pool.create_dataset("new").unwrap();
    assert!(pool.grow_dataset("new", stripe_bytes).is_err());

    // "temp"を破棄すると容量がプールへ返却される
    pool.destroy_dataset("temp").unwrap();
    assert_eq!(pool.usage().free_stripes, NUM_STRIPES);

    // 返却された容量を別のデータセットが利用できる
    pool.grow_dataset("new", 2 * stripe_bytes).unwrap();
    assert_eq!(pool.dataset_size("new").unwrap(), 2 * stripe_bytes);

    let payload: Vec<u8> = (0..2 * stripe_bytes).map(|i| (i * 7 % 251) as u8).collect();
    pool.write("new", 0, &payload).unwrap();
    assert_eq!(pool.read("new", 0, 2 * stripe_bytes).unwrap(), payload);

    std::fs::remove_dir_all(&dir).ok();
}
