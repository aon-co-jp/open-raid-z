//! スナップショット/クローンのTESTモード統合テスト。
//!
//! スナップショット作成が「一瞬・低容量」であること(実データをコピーしない)、
//! スナップショット後に元データセットを書き換えてもスナップショットの内容が
//! 変わらないこと、クローンが独立して書き込み可能で元データを壊さないこと、
//! 参照カウントによりストライプが正しく解放・保護されることを、
//! 実ファイルI/Oを経由して検証する。

use open_raid_z_core::block_device::FileBackedDevice;
use open_raid_z_core::pool::Pool;
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};
use std::path::PathBuf;

const CHUNK_SIZE: usize = 64;
const NUM_STRIPES: u64 = 8;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("open_runo_snap_it_{name}_{}", std::process::id()));
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

fn stripe_bytes() -> u64 {
    4 * CHUNK_SIZE as u64
}

#[test]
fn snapshot_creation_does_not_copy_data_and_is_cheap() {
    let dir = scratch_dir("cheap");
    let mut pool = build_pool(&dir);
    let sb = stripe_bytes();

    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", 2 * sb).unwrap();
    let v = vec![0x77u8; 2 * sb as usize];
    pool.write("ds", 0, &v).unwrap();

    let used_before_snapshot = pool.usage().used_stripes;
    pool.create_snapshot("ds", "snap1").unwrap();
    let used_after_snapshot = pool.usage().used_stripes;

    // スナップショット作成は既存ストライプへの参照を増やすだけで、
    // 新たにデータ用のストライプを消費しない(実データをコピーしない)。
    assert_eq!(
        used_before_snapshot, used_after_snapshot,
        "スナップショット作成でプールの使用容量が増えてはいけない(実データをコピーしていない証拠)"
    );

    assert_eq!(pool.snapshot_size("ds", "snap1").unwrap(), 2 * sb);
    assert_eq!(pool.read_snapshot("ds", "snap1", 0, 2 * sb).unwrap(), v);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn writing_to_dataset_after_snapshot_does_not_change_the_snapshot() {
    let dir = scratch_dir("isolated");
    let mut pool = build_pool(&dir);
    let sb = stripe_bytes();

    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", sb).unwrap();
    let original = vec![0x11u8; sb as usize];
    pool.write("ds", 0, &original).unwrap();

    pool.create_snapshot("ds", "before").unwrap();

    let updated = vec![0x22u8; sb as usize];
    pool.write("ds", 0, &updated).unwrap();

    // 現在のデータセットは新しい値
    assert_eq!(pool.read("ds", 0, sb).unwrap(), updated);
    // スナップショットは撮った時点のまま変わらない
    assert_eq!(pool.read_snapshot("ds", "before", 0, sb).unwrap(), original);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn destroying_dataset_keeps_snapshot_data_alive_via_refcount() {
    let dir = scratch_dir("keepalive");
    let mut pool = build_pool(&dir);
    let sb = stripe_bytes();

    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", sb).unwrap();
    let v = vec![0x33u8; sb as usize];
    pool.write("ds", 0, &v).unwrap();
    pool.create_snapshot("ds", "keep").unwrap();

    pool.destroy_dataset("ds").unwrap();

    // 元データセットを破棄しても、スナップショットが参照している限り
    // 物理ストライプは解放されず、スナップショット経由でデータを読める。
    assert_eq!(pool.read_snapshot("ds", "keep", 0, sb).unwrap(), v);

    // スナップショットも破棄すれば、今度こそ容量がプールへ返却される
    let used_before_destroy = pool.usage().used_stripes;
    assert!(used_before_destroy > 0, "スナップショットがストライプを保持しているはず");
    pool.destroy_snapshot("ds", "keep").unwrap();
    // メタデータ用の予約ストライプぶん、常に1が残る。
    assert_eq!(pool.usage().used_stripes, 1);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn clone_is_independently_writable_without_affecting_the_original_snapshot() {
    let dir = scratch_dir("clone");
    let mut pool = build_pool(&dir);
    let sb = stripe_bytes();

    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", sb).unwrap();
    let original = vec![0xAAu8; sb as usize];
    pool.write("ds", 0, &original).unwrap();
    pool.create_snapshot("ds", "base").unwrap();

    let used_before_clone = pool.usage().used_stripes;
    pool.create_clone("ds", "base", "clone1").unwrap();
    let used_after_clone = pool.usage().used_stripes;

    // クローン作成もデータをコピーしないので使用容量は増えない
    assert_eq!(used_before_clone, used_after_clone);

    // クローン直後は元データと同じ内容が読める
    assert_eq!(pool.read("clone1", 0, sb).unwrap(), original);

    // クローンへ書き込む(CoWで分岐するはず)
    let cloned_write = vec![0xCCu8; sb as usize];
    pool.write("clone1", 0, &cloned_write).unwrap();

    // クローンは新しい値、元データセット・スナップショットは無傷
    assert_eq!(pool.read("clone1", 0, sb).unwrap(), cloned_write);
    assert_eq!(pool.read("ds", 0, sb).unwrap(), original);
    assert_eq!(pool.read_snapshot("ds", "base", 0, sb).unwrap(), original);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn multiple_snapshots_over_time_each_preserve_their_own_point_in_time_view() {
    let dir = scratch_dir("timeline");
    let mut pool = build_pool(&dir);
    let sb = stripe_bytes();

    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", sb).unwrap();

    pool.write("ds", 0, &vec![1u8; sb as usize]).unwrap();
    pool.create_snapshot("ds", "t1").unwrap();

    pool.write("ds", 0, &vec![2u8; sb as usize]).unwrap();
    pool.create_snapshot("ds", "t2").unwrap();

    pool.write("ds", 0, &vec![3u8; sb as usize]).unwrap();

    assert_eq!(pool.read_snapshot("ds", "t1", 0, sb).unwrap(), vec![1u8; sb as usize]);
    assert_eq!(pool.read_snapshot("ds", "t2", 0, sb).unwrap(), vec![2u8; sb as usize]);
    assert_eq!(pool.read("ds", 0, sb).unwrap(), vec![3u8; sb as usize]);
    assert_eq!(pool.snapshot_names("ds"), vec!["t1".to_string(), "t2".to_string()]);

    std::fs::remove_dir_all(&dir).ok();
}
