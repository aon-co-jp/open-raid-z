//! RAID0/RAID1/RAID5/RAID6の統合テスト。
//!
//! `raidz_failure_recovery.rs`と同じ考え方(ファイルバックエンドの仮想
//! ディスクで実際のI/O経由の障害シミュレーション)を、Z2/Z3以外の
//! 業界標準RAIDレベル(`vdev.rs`のRaidLevel拡張)に対して検証する。

use open_raid_z_core::block_device::{BlockDevice, FaultInjectableDevice, FileBackedDevice};
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};
use std::path::PathBuf;

const CHUNK_SIZE: usize = 64;
const NUM_STRIPES: u64 = 8;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("open_runo_raid_series_it_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn expected_stripe_data(num_data: usize, stripe: u64) -> Vec<u8> {
    let mut data = vec![0u8; num_data * CHUNK_SIZE];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = ((stripe as usize * 131 + i * 17 + 7) % 256) as u8;
    }
    data
}

fn build_devices(dir: &std::path::Path, num_devices: usize) -> Vec<FaultInjectableDevice<FileBackedDevice>> {
    (0..num_devices)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            let dev = FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * NUM_STRIPES)
                .expect("仮想ディスクファイルの作成に失敗");
            FaultInjectableDevice::new(dev)
        })
        .collect()
}

fn write_all_stripes<D: BlockDevice>(vdev: &mut RaidZVdev<D>) {
    let num_data = vdev.num_data_disks();
    for stripe in 0..NUM_STRIPES {
        let data = expected_stripe_data(num_data, stripe);
        vdev.write_stripe(stripe, &data).expect("write_stripe失敗");
    }
}

fn assert_all_stripes_readable_and_correct<D: BlockDevice>(vdev: &mut RaidZVdev<D>) {
    let num_data = vdev.num_data_disks();
    for stripe in 0..NUM_STRIPES {
        let data = vdev.read_stripe(stripe).expect("read_stripe失敗(復旧できないはずがない)");
        assert_eq!(data, expected_stripe_data(num_data, stripe), "stripe {stripe} の内容が不一致");
    }
}

#[test]
fn raid0_has_no_redundancy_and_any_single_failure_is_unrecoverable() {
    let dir = scratch_dir("raid0");
    let devices = build_devices(&dir, 4); // 4データ、パリティ無し
    let mut vdev = RaidZVdev::new(devices, RaidLevel::Raid0, CHUNK_SIZE);

    write_all_stripes(&mut vdev);
    assert_all_stripes_readable_and_correct(&mut vdev);

    vdev.devices_mut()[1].failed = true;
    assert!(vdev.read_stripe(0).is_err(), "RAID0は冗長性が無いため1台の故障でも復旧不能なはず");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn raid1_mirrors_across_all_disks_and_survives_losing_all_but_one_copy() {
    let dir = scratch_dir("raid1");
    let devices = build_devices(&dir, 4); // データ1台相当+3面ミラー
    let mut vdev = RaidZVdev::new(devices, RaidLevel::Raid1, CHUNK_SIZE);
    assert_eq!(vdev.num_data_disks(), 1, "Raid1はデータディスク1台(残り全台がミラーコピー)のはず");

    write_all_stripes(&mut vdev);
    assert_all_stripes_readable_and_correct(&mut vdev);

    // 4台中3台を同時に失っても、残り1台から読めるはず(N面ミラーの定義通り)。
    for idx in [0usize, 1usize, 2usize] {
        vdev.devices_mut()[idx].failed = true;
    }
    assert_all_stripes_readable_and_correct(&mut vdev);

    // 全滅すれば当然読めない。
    vdev.devices_mut()[3].failed = true;
    assert!(vdev.read_stripe(0).is_err(), "ミラー全滅は復旧不能なはず");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn raid1_resilver_restores_replaced_mirror_member() {
    let dir = scratch_dir("raid1_resilver");
    let devices = build_devices(&dir, 3);
    let mut vdev = RaidZVdev::new(devices, RaidLevel::Raid1, CHUNK_SIZE);

    write_all_stripes(&mut vdev);

    vdev.devices_mut()[1].failed = true;
    vdev.devices_mut()[1].failed = false; // 交換直後、中身は古い/空のまま
    vdev.resilver(1, NUM_STRIPES).expect("resilverに失敗");

    assert_all_stripes_readable_and_correct(&mut vdev);
    for stripe in 0..NUM_STRIPES {
        let offset = stripe * CHUNK_SIZE as u64;
        let direct = vdev.devices_mut()[1].read_at(offset, CHUNK_SIZE).unwrap();
        assert_eq!(direct, expected_stripe_data(1, stripe), "resilver後のミラーコピーが不一致");
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn raid5_survives_single_disk_failure_but_not_two() {
    let dir = scratch_dir("raid5");
    let devices = build_devices(&dir, 4); // 3データ + 1パリティ(P)
    let mut vdev = RaidZVdev::new(devices, RaidLevel::Raid5, CHUNK_SIZE);

    write_all_stripes(&mut vdev);
    assert_all_stripes_readable_and_correct(&mut vdev);

    vdev.devices_mut()[0].failed = true;
    assert_all_stripes_readable_and_correct(&mut vdev);

    vdev.devices_mut()[1].failed = true;
    assert!(vdev.read_stripe(0).is_err(), "RAID5は単一パリティのため2台同時故障は復旧不能なはず");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn raid6_survives_two_disk_failure_but_not_three() {
    let dir = scratch_dir("raid6");
    let devices = build_devices(&dir, 5); // 3データ + 2パリティ(P,Q)
    let mut vdev = RaidZVdev::new(devices, RaidLevel::Raid6, CHUNK_SIZE);

    write_all_stripes(&mut vdev);
    assert_all_stripes_readable_and_correct(&mut vdev);

    vdev.devices_mut()[0].failed = true;
    vdev.devices_mut()[2].failed = true;
    assert_all_stripes_readable_and_correct(&mut vdev);

    vdev.devices_mut()[3].failed = true;
    assert!(vdev.read_stripe(0).is_err(), "RAID6は二重パリティのため3台同時故障は復旧不能なはず");

    std::fs::remove_dir_all(&dir).ok();
}
