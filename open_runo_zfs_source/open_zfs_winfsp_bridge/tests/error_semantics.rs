//! `BridgeError`のセマンティックなvariant([`DatasetNotFound`]/[`SnapshotNotFound`]/
//! [`AlreadyExists`]/[`CapacityExceeded`]/[`InvalidConfig`]/[`Unrecoverable`])が
//! 実際に使われ、呼び出し側がエラーの種類をパターンマッチで判別できることを
//! 検証する。
//!
//! 以前はこれらのvariant自体は定義されていたものの、`pool.rs`/`vdev.rs`/
//! `raid10.rs`の実装側は全てのエラーを`BridgeError::Io(std::io::Error::other(..))`
//! で潰していたため、呼び出し側は「データセットが無いのか」「容量不足なのか」
//! 「単なるI/Oエラーなのか」を一切区別できなかった(例えば`mount.rs`が
//! WinFspへ返すNTSTATUSを細かく分類しようにも、元のエラーに情報が残って
//! いなかった)。
//!
//! [`DatasetNotFound`]: open_zfs_winfsp_bridge::BridgeError::DatasetNotFound
//! [`SnapshotNotFound`]: open_zfs_winfsp_bridge::BridgeError::SnapshotNotFound
//! [`AlreadyExists`]: open_zfs_winfsp_bridge::BridgeError::AlreadyExists
//! [`CapacityExceeded`]: open_zfs_winfsp_bridge::BridgeError::CapacityExceeded
//! [`InvalidConfig`]: open_zfs_winfsp_bridge::BridgeError::InvalidConfig
//! [`Unrecoverable`]: open_zfs_winfsp_bridge::BridgeError::Unrecoverable

use open_zfs_winfsp_bridge::block_device::{FaultInjectableDevice, FileBackedDevice};
use open_zfs_winfsp_bridge::pool::Pool;
use open_zfs_winfsp_bridge::raid10::Raid10Vdev;
use open_zfs_winfsp_bridge::vdev::{RaidLevel, RaidZVdev};
use open_zfs_winfsp_bridge::BridgeError;
use std::path::PathBuf;

const CHUNK_SIZE: usize = 64;
const NUM_STRIPES: u64 = 8;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("open_runo_error_semantics_it_{name}_{}", std::process::id()));
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
fn operations_on_a_missing_dataset_return_dataset_not_found() {
    let dir = scratch_dir("dataset_not_found");
    let mut pool = build_pool(&dir);

    assert!(matches!(
        pool.dataset_size("ghost"),
        Err(BridgeError::DatasetNotFound(name)) if name == "ghost"
    ));
    assert!(matches!(pool.destroy_dataset("ghost"), Err(BridgeError::DatasetNotFound(_))));
    assert!(matches!(
        pool.grow_dataset("ghost", stripe_bytes()),
        Err(BridgeError::DatasetNotFound(_))
    ));
    assert!(matches!(
        pool.write("ghost", 0, &vec![0u8; stripe_bytes() as usize]),
        Err(BridgeError::DatasetNotFound(_))
    ));
    assert!(matches!(pool.read("ghost", 0, stripe_bytes()), Err(BridgeError::DatasetNotFound(_))));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn creating_a_duplicate_dataset_or_snapshot_returns_already_exists() {
    let dir = scratch_dir("already_exists");
    let mut pool = build_pool(&dir);

    pool.create_dataset("tank").unwrap();
    assert!(matches!(pool.create_dataset("tank"), Err(BridgeError::AlreadyExists(_))));

    pool.grow_dataset("tank", stripe_bytes()).unwrap();
    pool.create_snapshot("tank", "snap1").unwrap();
    assert!(matches!(
        pool.create_snapshot("tank", "snap1"),
        Err(BridgeError::AlreadyExists(_))
    ));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn operations_on_a_missing_snapshot_return_snapshot_not_found() {
    let dir = scratch_dir("snapshot_not_found");
    let mut pool = build_pool(&dir);
    pool.create_dataset("tank").unwrap();
    pool.grow_dataset("tank", stripe_bytes()).unwrap();

    assert!(matches!(
        pool.destroy_snapshot("tank", "ghost"),
        Err(BridgeError::SnapshotNotFound(_))
    ));
    assert!(matches!(
        pool.snapshot_size("tank", "ghost"),
        Err(BridgeError::SnapshotNotFound(_))
    ));
    assert!(matches!(
        pool.read_snapshot("tank", "ghost", 0, stripe_bytes()),
        Err(BridgeError::SnapshotNotFound(_))
    ));
    assert!(matches!(
        pool.create_clone("tank", "ghost", "clone1"),
        Err(BridgeError::SnapshotNotFound(_))
    ));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn exceeding_pool_or_dataset_capacity_returns_capacity_exceeded() {
    let dir = scratch_dir("capacity_exceeded");
    let mut pool = build_pool(&dir);
    pool.create_dataset("tank").unwrap();

    // プール容量(8ストライプ)を超える割当。
    assert!(matches!(
        pool.grow_dataset("tank", (NUM_STRIPES + 1) * stripe_bytes()),
        Err(BridgeError::CapacityExceeded(_))
    ));

    // データセットには1ストライプぶんしか割り当てず、その範囲を超える書き込み/読み込み。
    pool.grow_dataset("tank", stripe_bytes()).unwrap();
    assert!(matches!(
        pool.write("tank", 0, &vec![0u8; 2 * stripe_bytes() as usize]),
        Err(BridgeError::CapacityExceeded(_))
    ));
    assert!(matches!(
        pool.read("tank", 0, 2 * stripe_bytes()),
        Err(BridgeError::CapacityExceeded(_))
    ));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn raid10_with_a_bad_mirror_width_returns_invalid_config() {
    let dir = scratch_dir("invalid_config");
    let devices: Vec<FileBackedDevice> = (0..4)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * NUM_STRIPES).unwrap()
        })
        .collect();

    // ミラー幅1台は不正(最低2台必要)。
    assert!(matches!(
        Raid10Vdev::new(devices, 1, CHUNK_SIZE),
        Err(BridgeError::InvalidConfig(_))
    ));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn losing_more_disks_than_parity_allows_returns_unrecoverable() {
    let dir = scratch_dir("unrecoverable");
    let devices: Vec<FaultInjectableDevice<FileBackedDevice>> = (0..6)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            FaultInjectableDevice::new(FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * NUM_STRIPES).unwrap())
        })
        .collect();
    let mut vdev = RaidZVdev::new(devices, RaidLevel::Z2, CHUNK_SIZE); // パリティ2台まで許容

    vdev.write_stripe(0, &vec![0xAAu8; 4 * CHUNK_SIZE]).unwrap();

    // Z2の許容範囲(2台)を超える3台同時故障。
    vdev.devices_mut()[0].failed = true;
    vdev.devices_mut()[1].failed = true;
    vdev.devices_mut()[2].failed = true;

    assert!(matches!(vdev.read_stripe(0), Err(BridgeError::Unrecoverable(_))));

    std::fs::remove_dir_all(&dir).ok();
}
