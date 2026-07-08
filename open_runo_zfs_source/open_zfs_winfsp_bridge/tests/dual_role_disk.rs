//! 「1台のディスクをパーティション分割し、片方をミラー(RAID1)のメンバーに、
//! もう片方を別のRAID6/RAID-Z2配列のメンバーにする」という、1台のディスクを
//! 2つの独立した配列で同時に使い回すシナリオの統合テスト。
//!
//! `partition_device`でディスクを論理分割してさえいれば、2つの配列が
//! 同じ物理ディスクの異なるバイト範囲を独立に読み書きしても、互いの
//! データを一切破壊しないことを実際のI/Oで検証する。

use open_zfs_winfsp_bridge::block_device::FileBackedDevice;
use open_zfs_winfsp_bridge::partition::partition_device;
use open_zfs_winfsp_bridge::vdev::{RaidLevel, RaidZVdev};
use std::path::PathBuf;

const CHUNK_SIZE: usize = 64;
const STRIPES: u64 = 8;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("open_runo_dual_role_it_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn one_disk_serves_a_mirror_and_a_raid6_array_simultaneously_via_partitioning() {
    let dir = scratch_dir("shared");

    // 共有ディスク: 半分をミラー用、半分をRAID6用のパーティションとして使う。
    let shared_disk_path = dir.join("shared.img");
    let partition_size = CHUNK_SIZE as u64 * STRIPES;
    let shared_disk = FileBackedDevice::create_fixed_size(&shared_disk_path, partition_size * 2).unwrap();
    let mut parts = partition_device(shared_disk, &[partition_size, partition_size]);
    let mirror_partition = parts.remove(0);
    let raid6_partition = parts.remove(0);

    // 専用ディスク(パーティション分割していないディスク)も、`RaidZVdev`が
    // 単一の型`D`で全メンバーを揃える必要があるため、`PartitionedDevice`として
    // (全域を1パーティションとして)統一する。
    let whole_disk_as_partition = |path: PathBuf, size: u64| {
        let raw = FileBackedDevice::create_fixed_size(&path, size).unwrap();
        partition_device(raw, &[size]).remove(0)
    };

    // --- ミラー(RAID1): 共有ディスクのパーティションA + 専用ディスク1台 ---
    let mirror_partner = whole_disk_as_partition(dir.join("mirror_partner.img"), partition_size);
    let mut mirror_vdev = RaidZVdev::new(vec![mirror_partition, mirror_partner], RaidLevel::Raid1, CHUNK_SIZE);

    // --- RAID6/Z2: 共有ディスクのパーティションB + 専用ディスク3台 ---
    let raid6_disks: Vec<_> = (0..3)
        .map(|i| whole_disk_as_partition(dir.join(format!("raid6_disk{i}.img")), partition_size))
        .collect();
    let mut raid6_devices = vec![raid6_partition];
    raid6_devices.extend(raid6_disks);
    let mut raid6_vdev = RaidZVdev::new(raid6_devices, RaidLevel::Raid6, CHUNK_SIZE);

    // 両配列へ、それぞれ異なるデータを全ストライプぶん書き込む。
    for stripe in 0..STRIPES {
        let mirror_data = vec![0xAAu8; CHUNK_SIZE]; // Raid1はnum_data=1
        mirror_vdev.write_stripe(stripe, &mirror_data).unwrap();

        let raid6_data = vec![0xBBu8; CHUNK_SIZE * 2]; // Raid6は常に2台がパリティ(4台中2台がデータ)
        raid6_vdev.write_stripe(stripe, &raid6_data).unwrap();
    }

    // 両配列とも、互いに一切影響を受けずに正しく読み出せることを確認する。
    for stripe in 0..STRIPES {
        assert_eq!(mirror_vdev.read_stripe(stripe).unwrap(), vec![0xAAu8; CHUNK_SIZE]);
        assert_eq!(raid6_vdev.read_stripe(stripe).unwrap(), vec![0xBBu8; CHUNK_SIZE * 2]);
    }

    std::fs::remove_dir_all(&dir).ok();
}
