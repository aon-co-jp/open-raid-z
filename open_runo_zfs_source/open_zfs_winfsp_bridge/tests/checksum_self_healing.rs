//! チェックサム+自己修復のTESTモード統合テスト。
//!
//! ディスク自体は生きている(読み込みエラーにはならない)が、内容だけが
//! 静かに壊れている「ビットロット」を、実際のファイルI/Oを経由してシミュレート
//! し、`RaidZVdev`がチェックサム不一致を検知してパリティから自動修復
//! (自己修復)することを検証する。

use open_zfs_winfsp_bridge::block_device::{BlockDevice, FaultInjectableDevice, FileBackedDevice};
use open_zfs_winfsp_bridge::vdev::{RaidLevel, RaidZVdev};
use std::path::PathBuf;

const CHUNK_SIZE: usize = 64;
const NUM_STRIPES: u64 = 8;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "open_runo_checksum_it_{name}_{}",
        std::process::id()
    ));
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

fn build_devices(
    dir: &std::path::Path,
    num_devices: usize,
) -> Vec<FaultInjectableDevice<FileBackedDevice>> {
    (0..num_devices)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            let dev = FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * NUM_STRIPES)
                .expect("仮想ディスクファイルの作成に失敗");
            FaultInjectableDevice::new(dev)
        })
        .collect()
}

/// `failed`フラグを立てずに、ディスクの中身だけを直接壊す
/// (物理故障ではなく「ビットロット」のシミュレーション)。
fn corrupt_disk_directly<D: BlockDevice>(
    vdev: &mut RaidZVdev<D>,
    disk_index: usize,
    stripe: u64,
) {
    let offset = stripe * CHUNK_SIZE as u64;
    let mut garbage = vdev.devices_mut()[disk_index]
        .read_at(offset, CHUNK_SIZE)
        .unwrap();
    for b in garbage.iter_mut() {
        *b ^= 0xFF; // 全ビット反転で明確に壊す
    }
    vdev.devices_mut()[disk_index]
        .write_at(offset, &garbage)
        .unwrap();
}

#[test]
fn read_stripe_detects_and_heals_silent_corruption() {
    let dir = scratch_dir("single");
    let devices = build_devices(&dir, 6); // 4データ + 2パリティ(Z2)
    let mut vdev = RaidZVdev::new(devices, RaidLevel::Z2, CHUNK_SIZE);

    for stripe in 0..NUM_STRIPES {
        vdev.write_stripe(stripe, &expected_stripe_data(4, stripe)).unwrap();
    }

    // ディスク2番を、故障フラグを立てずに直接破壊(ビットロット)
    corrupt_disk_directly(&mut vdev, 2, 3);

    // read_stripeは(failedにしていないので)ディスク2番からの読み込み自体は
    // 成功するが、チェックサム不一致を検知して自動的に復旧するはず。
    let data = vdev.read_stripe(3).expect("チェックサム不一致でも1台以内なら復旧できるはず");
    assert_eq!(data, expected_stripe_data(4, 3), "復旧後のデータが正しくない");

    // 自己修復により、ディスク2番自体の中身も正しい値に書き戻されているはず
    let offset = 3 * CHUNK_SIZE as u64;
    let healed_raw = vdev.devices_mut()[2].read_at(offset, CHUNK_SIZE).unwrap();
    let expected_chunk = &expected_stripe_data(4, 3)[2 * CHUNK_SIZE..3 * CHUNK_SIZE];
    assert_eq!(healed_raw, expected_chunk, "自己修復後もディスク上の中身が破損したまま");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn scrub_finds_and_heals_multiple_corruptions_across_the_pool() {
    let dir = scratch_dir("scrub");
    let devices = build_devices(&dir, 7); // 4データ + 3パリティ(Z3)
    let mut vdev = RaidZVdev::new(devices, RaidLevel::Z3, CHUNK_SIZE);

    for stripe in 0..NUM_STRIPES {
        vdev.write_stripe(stripe, &expected_stripe_data(4, stripe)).unwrap();
    }

    // 複数のストライプ・複数のディスクにまたがってビットロットを注入
    corrupt_disk_directly(&mut vdev, 0, 1);
    corrupt_disk_directly(&mut vdev, 5, 4); // パリティ(Q)も対象に含める
    corrupt_disk_directly(&mut vdev, 3, 6);

    let report = vdev.scrub(NUM_STRIPES).expect("scrubに失敗");
    assert_eq!(report.stripes_scanned, NUM_STRIPES);
    assert_eq!(report.corruptions_healed, 3, "3件の破損すべてを検知・修復できているはず");

    // scrub後は全ストライプが健全に読み出せる
    for stripe in 0..NUM_STRIPES {
        let data = vdev.read_stripe(stripe).unwrap();
        assert_eq!(data, expected_stripe_data(4, stripe), "stripe {stripe}");
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn corruption_combined_with_one_real_failure_still_recovers_within_z2_limit() {
    // Z2は「合計2台まで」の異常(物理故障+サイレント破損の合計)を許容できるはず。
    let dir = scratch_dir("mixed");
    let devices = build_devices(&dir, 6);
    let mut vdev = RaidZVdev::new(devices, RaidLevel::Z2, CHUNK_SIZE);

    for stripe in 0..NUM_STRIPES {
        vdev.write_stripe(stripe, &expected_stripe_data(4, stripe)).unwrap();
    }

    corrupt_disk_directly(&mut vdev, 1, 0); // ビットロット
    vdev.devices_mut()[3].failed = true; // 物理故障

    let data = vdev.read_stripe(0).expect("破損1台+故障1台の合計2台はZ2の許容範囲内");
    assert_eq!(data, expected_stripe_data(4, 0));

    std::fs::remove_dir_all(&dir).ok();
}
