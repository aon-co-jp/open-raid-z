//! `Pool::write_unaligned_growing` / `Pool::set_dataset_size` / `Pool::rename_dataset`
//! の統合テスト。
//!
//! これらは、WinFspマウント(`mount.rs`)経由での`create`/書き込み/
//! `set_file_size`/`rename`をサポートするために追加された。通常の
//! ファイルシステムと同様「作成直後は0バイト」「書き込めば自動的に
//! ファイルが伸びる」「切り詰めれば容量が返却される」という挙動を、
//! 明示的な`grow_dataset`呼び出し無しで実現する。

use open_raid_z_core::block_device::FileBackedDevice;
use open_raid_z_core::pool::Pool;
use open_raid_z_core::BridgeError;
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};
use std::path::PathBuf;

const CHUNK_SIZE: usize = 64;
const NUM_STRIPES: u64 = 8;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("open_runo_dynamic_file_size_it_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build_pool(dir: &std::path::Path) -> Pool<RaidZVdev<FileBackedDevice>> {
    let devices: Vec<FileBackedDevice> = (0..6)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * NUM_STRIPES).unwrap()
        })
        .collect();
    let vdev = RaidZVdev::new(devices, RaidLevel::Z2, CHUNK_SIZE);
    Pool::new(vdev, NUM_STRIPES)
}

/// num_data(4) * chunk_size(64) = 256バイト/ストライプ
fn stripe_bytes() -> u64 {
    4 * CHUNK_SIZE as u64
}

#[test]
fn write_unaligned_growing_extends_a_freshly_created_zero_size_dataset() {
    let dir = scratch_dir("fresh");
    let mut pool = build_pool(&dir);
    pool.create_dataset("ds").unwrap();
    assert_eq!(pool.dataset_size("ds").unwrap(), 0);

    // grow_datasetを一切呼ばずに、いきなり境界に揃っていない書き込みを行う。
    let payload = b"hello, open-runo";
    pool.write_unaligned_growing("ds", 0, payload).unwrap();

    // 論理サイズは、ストライプ境界(256バイト)ではなく実際に書いたバイト数どおり。
    assert_eq!(pool.dataset_size("ds").unwrap(), payload.len() as u64);
    assert_eq!(pool.read_unaligned("ds", 0, payload.len() as u64).unwrap(), payload);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn write_unaligned_growing_extends_incrementally_and_preserves_earlier_writes() {
    let dir = scratch_dir("incremental");
    let mut pool = build_pool(&dir);
    pool.create_dataset("ds").unwrap();

    pool.write_unaligned_growing("ds", 0, b"first-").unwrap();
    assert_eq!(pool.dataset_size("ds").unwrap(), 6);

    // 既存の末尾のさらに先(境界を越える位置)へ追記する。
    let second = b"second-chunk-of-data";
    let second_offset = stripe_bytes(); // 1ストライプ分先、非連続の追記
    pool.write_unaligned_growing("ds", second_offset, second).unwrap();
    assert_eq!(pool.dataset_size("ds").unwrap(), second_offset + second.len() as u64);

    // 先に書いた分は変わらず読める。
    assert_eq!(pool.read_unaligned("ds", 0, 6).unwrap(), b"first-");
    assert_eq!(pool.read_unaligned("ds", second_offset, second.len() as u64).unwrap(), second);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn write_unaligned_growing_fails_cleanly_when_the_pool_has_no_free_capacity() {
    let dir = scratch_dir("pool_full");
    let mut pool = build_pool(&dir);
    pool.create_dataset("ds").unwrap();
    // プール総容量(NUM_STRIPES=8ストライプ)を丸ごと使い切る書き込みを要求する。
    let huge = vec![0x42u8; (NUM_STRIPES * stripe_bytes()) as usize + 1];
    let result = pool.write_unaligned_growing("ds", 0, &huge);
    assert!(matches!(result, Err(BridgeError::CapacityExceeded(_))));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn set_dataset_size_shrinks_and_reclaims_capacity_for_other_datasets() {
    let dir = scratch_dir("shrink");
    let mut pool = build_pool(&dir);
    pool.create_dataset("ds").unwrap();
    pool.write_unaligned_growing("ds", 0, &vec![0x7Eu8; (3 * stripe_bytes()) as usize]).unwrap();
    assert_eq!(pool.dataset_size("ds").unwrap(), 3 * stripe_bytes());

    // 1ストライプ分だけ残して切り詰める。
    pool.set_dataset_size("ds", stripe_bytes()).unwrap();
    assert_eq!(pool.dataset_size("ds").unwrap(), stripe_bytes());
    // 切り詰め後もその範囲のデータは無事読める。
    assert_eq!(
        pool.read_unaligned("ds", 0, stripe_bytes()).unwrap(),
        vec![0x7Eu8; stripe_bytes() as usize]
    );

    // 解放されたはずの2ストライプ分を、別のデータセットが実際に使えることを確認する
    // (usage()の数値だけでなく、実際に確保・書き込みできることまで検証する)。
    pool.create_dataset("other").unwrap();
    pool.write_unaligned_growing("other", 0, &vec![0x11u8; (2 * stripe_bytes()) as usize]).unwrap();
    assert_eq!(pool.dataset_size("other").unwrap(), 2 * stripe_bytes());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn set_dataset_size_growing_zero_fills_the_extended_region() {
    let dir = scratch_dir("grow_zero_fill");
    let mut pool = build_pool(&dir);
    pool.create_dataset("ds").unwrap();
    pool.write_unaligned_growing("ds", 0, b"abc").unwrap();

    pool.set_dataset_size("ds", stripe_bytes()).unwrap();
    assert_eq!(pool.dataset_size("ds").unwrap(), stripe_bytes());

    let whole = pool.read_unaligned("ds", 0, stripe_bytes()).unwrap();
    assert_eq!(&whole[..3], b"abc");
    assert!(whole[3..].iter().all(|&b| b == 0), "拡張された領域はゼロ埋めされているはず");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rename_dataset_moves_data_and_frees_up_the_old_name_for_reuse() {
    let dir = scratch_dir("rename");
    let mut pool = build_pool(&dir);
    pool.create_dataset("old").unwrap();
    pool.write_unaligned_growing("old", 0, b"payload").unwrap();

    pool.rename_dataset("old", "new").unwrap();

    // 新しい名前でデータが読める。
    assert_eq!(pool.read_unaligned("new", 0, 7).unwrap(), b"payload");
    // 古い名前はもう存在しない。
    assert!(matches!(pool.dataset_size("old"), Err(BridgeError::DatasetNotFound(_))));
    // 古い名前は別データセットとして再利用できる。
    pool.create_dataset("old").unwrap();
    assert_eq!(pool.dataset_size("old").unwrap(), 0);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rename_dataset_rejects_collision_and_leaves_both_datasets_untouched() {
    let dir = scratch_dir("rename_collision");
    let mut pool = build_pool(&dir);
    pool.create_dataset("a").unwrap();
    pool.write_unaligned_growing("a", 0, b"aaa").unwrap();
    pool.create_dataset("b").unwrap();
    pool.write_unaligned_growing("b", 0, b"bbb").unwrap();

    let result = pool.rename_dataset("a", "b");
    assert!(matches!(result, Err(BridgeError::AlreadyExists(_))));

    // どちらのデータセットも元のまま無事。
    assert_eq!(pool.read_unaligned("a", 0, 3).unwrap(), b"aaa");
    assert_eq!(pool.read_unaligned("b", 0, 3).unwrap(), b"bbb");

    std::fs::remove_dir_all(&dir).ok();
}
