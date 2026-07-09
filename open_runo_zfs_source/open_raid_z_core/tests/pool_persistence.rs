//! `Pool::save`/`Pool::open`(メタデータ永続化)の統合テスト。
//!
//! これまでの`Pool`はメタデータ(データセット一覧・ストライプ割当・
//! スナップショット等)が完全にメモリ上にしかなく、プロセスを終了する
//! (アンマウント→再マウント、まして再起動)と実データのバイト列は
//! ディスク上に無事残っていても、それがどのデータセットのものかという
//! 管理情報ごと消えてしまっていた。本テストは、`save`で書き出した
//! メタデータを`open`で正しく復元できることを検証する
//! (「一度プロセスを閉じて、また開く」を`Pool`インスタンスを作り直す
//! ことでシミュレートする)。

use open_raid_z_core::block_device::FileBackedDevice;
use open_raid_z_core::pool::Pool;
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};
use open_raid_z_core::BridgeError;
use std::path::PathBuf;

const CHUNK_SIZE: usize = 64;
const NUM_STRIPES: u64 = 16;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("open_runo_pool_persistence_it_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 同じディスクイメージへ何度でも接続し直せる(=既存の中身を保った状態で
/// `Pool`を再構築できる)、`build_pool`のヘルパー。実際の「プロセスを
/// 再起動してディスクを再度開く」に相当する。
fn open_devices(dir: &std::path::Path) -> Vec<FileBackedDevice> {
    (0..6).map(|i| FileBackedDevice::open(dir.join(format!("disk{i}.img"))).unwrap()).collect()
}

fn create_devices(dir: &std::path::Path) -> Vec<FileBackedDevice> {
    (0..6)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * NUM_STRIPES).unwrap()
        })
        .collect()
}

fn stripe_bytes() -> u64 {
    4 * CHUNK_SIZE as u64 // num_data(4) * chunk_size (Z2: 6台-2パリティ)
}

#[test]
fn dataset_created_written_and_saved_survives_reopening_the_pool_from_scratch() {
    let dir = scratch_dir("basic");

    {
        let vdev = RaidZVdev::new(create_devices(&dir), RaidLevel::Z2, CHUNK_SIZE);
        let mut pool = Pool::new(vdev, NUM_STRIPES);
        pool.create_dataset("documents.txt").unwrap();
        pool.write_unaligned_growing("documents.txt", 0, b"hello, persisted world").unwrap();
        pool.save().unwrap();
        // `pool`はここでスコープを抜けてdropされる(=プロセス終了に相当)。
    }

    // 全く新しい`Pool`インスタンスとして、同じディスクを開き直す。
    let vdev = RaidZVdev::new(open_devices(&dir), RaidLevel::Z2, CHUNK_SIZE);
    let mut reopened = Pool::open(vdev, NUM_STRIPES).unwrap();

    let payload = b"hello, persisted world";
    assert_eq!(reopened.dataset_names(), vec!["documents.txt".to_string()]);
    assert_eq!(reopened.dataset_size("documents.txt").unwrap(), payload.len() as u64);
    assert_eq!(reopened.read_unaligned("documents.txt", 0, payload.len() as u64).unwrap(), payload);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn multiple_datasets_and_capacity_accounting_survive_reopening() {
    let dir = scratch_dir("multi");
    let sb = stripe_bytes();

    let usage_before = {
        let vdev = RaidZVdev::new(create_devices(&dir), RaidLevel::Z2, CHUNK_SIZE);
        let mut pool = Pool::new(vdev, NUM_STRIPES);
        pool.create_dataset("a").unwrap();
        pool.create_dataset("b").unwrap();
        pool.grow_dataset("a", 3 * sb).unwrap();
        pool.grow_dataset("b", 2 * sb).unwrap();
        pool.save().unwrap();
        pool.usage()
    };

    let vdev = RaidZVdev::new(open_devices(&dir), RaidLevel::Z2, CHUNK_SIZE);
    let reopened = Pool::open(vdev, NUM_STRIPES).unwrap();

    assert_eq!(reopened.usage(), usage_before, "空き容量・使用容量が保存前と完全に一致しているはず");
    let mut names = reopened.dataset_names();
    names.sort();
    assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(reopened.dataset_size("a").unwrap(), 3 * sb);
    assert_eq!(reopened.dataset_size("b").unwrap(), 2 * sb);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn destroying_a_dataset_before_saving_means_it_does_not_come_back_on_reopen() {
    let dir = scratch_dir("destroy_then_save");
    let sb = stripe_bytes();

    {
        let vdev = RaidZVdev::new(create_devices(&dir), RaidLevel::Z2, CHUNK_SIZE);
        let mut pool = Pool::new(vdev, NUM_STRIPES);
        pool.create_dataset("temp").unwrap();
        pool.grow_dataset("temp", sb).unwrap();
        pool.create_dataset("keep").unwrap();
        pool.grow_dataset("keep", sb).unwrap();
        // "temp"は保存前に破棄するので、復元後には存在しないはず。
        pool.destroy_dataset("temp").unwrap();
        pool.save().unwrap();
    }

    let vdev = RaidZVdev::new(open_devices(&dir), RaidLevel::Z2, CHUNK_SIZE);
    let reopened = Pool::open(vdev, NUM_STRIPES).unwrap();
    assert_eq!(reopened.dataset_names(), vec!["keep".to_string()]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn snapshots_and_their_shared_stripe_refcounts_survive_reopening() {
    let dir = scratch_dir("snapshots");
    let sb = stripe_bytes();

    {
        let vdev = RaidZVdev::new(create_devices(&dir), RaidLevel::Z2, CHUNK_SIZE);
        let mut pool = Pool::new(vdev, NUM_STRIPES);
        pool.create_dataset("ds").unwrap();
        pool.grow_dataset("ds", sb).unwrap();
        pool.write("ds", 0, &vec![0xABu8; sb as usize]).unwrap();
        pool.create_snapshot("ds", "snap1").unwrap();
        // スナップショット後にCoWで上書き。"ds"は新しいストライプへ移るが、
        // "snap1"は元のストライプを参照し続けるはず。
        pool.write("ds", 0, &vec![0xCDu8; sb as usize]).unwrap();
        pool.save().unwrap();
    }

    let vdev = RaidZVdev::new(open_devices(&dir), RaidLevel::Z2, CHUNK_SIZE);
    let mut reopened = Pool::open(vdev, NUM_STRIPES).unwrap();

    assert_eq!(reopened.read("ds", 0, sb).unwrap(), vec![0xCDu8; sb as usize]);
    assert_eq!(reopened.read_snapshot("ds", "snap1", 0, sb).unwrap(), vec![0xABu8; sb as usize]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn opening_with_a_mismatched_total_stripes_is_rejected() {
    let dir = scratch_dir("mismatch");
    {
        let vdev = RaidZVdev::new(create_devices(&dir), RaidLevel::Z2, CHUNK_SIZE);
        let mut pool = Pool::new(vdev, NUM_STRIPES);
        pool.create_dataset("ds").unwrap();
        pool.save().unwrap();
    }

    // 保存時と異なるtotal_stripesで開こうとするとエラーになるはず
    // (ディスク構成の取り違え等を検知するため)。
    let vdev = RaidZVdev::new(open_devices(&dir), RaidLevel::Z2, CHUNK_SIZE);
    let result = Pool::open(vdev, NUM_STRIPES + 1);
    assert!(matches!(result, Err(BridgeError::InvalidConfig(_))));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn opening_a_pool_that_was_never_saved_is_rejected_cleanly() {
    let dir = scratch_dir("never_saved");
    // save()を一度も呼ばずに、まっさらなディスクをいきなりopenしようとする。
    let devices = create_devices(&dir);
    let vdev = RaidZVdev::new(devices, RaidLevel::Z2, CHUNK_SIZE);
    let result = Pool::open(vdev, NUM_STRIPES);
    assert!(result.is_err(), "スーパーブロックが存在しないディスクのopenは失敗するはず(パニックしない)");

    std::fs::remove_dir_all(&dir).ok();
}
