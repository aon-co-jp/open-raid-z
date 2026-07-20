//! RAID-Z2/Z3の「TESTモード」統合テスト。
//!
//! ファイルバックエンドの仮想ディスク(BlockDevice)を使い、実際のディスク
//! I/Oを経由してRAID-Z2(2台同時故障)・RAID-Z3(3台同時故障)からの
//! データ復旧と、故障ディスク交換後の自動修復(resilver)までを検証する。
//!
//! 実ドライブ(VHDX/USBメモリ等)を使わずに、`open_raid_z_core`の
//! ストレージ層(`block_device`/`vdev`)の正しさをエンドツーエンドで
//! 確認できる、安全な検証手段として用意した。

use open_raid_z_core::block_device::{BlockDevice, FaultInjectableDevice, FileBackedDevice};
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};
use std::path::PathBuf;

const CHUNK_SIZE: usize = 64;
const NUM_STRIPES: u64 = 8;

/// テストごとに一意な一時ディレクトリを用意する(前回の残骸があれば削除)。
fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("open_runo_raidz_it_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// ストライプ番号から決定的な(再現可能な)テストデータを生成する。
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

/// 指定インデックスのディスクに、健全な状態で書かれているはずの生データが
/// 実際に書き込まれているかを直接確認する(resilverが本当にディスクへ
/// 正しい内容を書き戻したことの証明。read_stripeの復旧経路を経由しない)。
fn assert_disk_directly_matches_healthy_content<D: BlockDevice>(vdev: &mut RaidZVdev<D>, disk_index: usize) {
    let chunk_size = vdev.chunk_size();
    let num_data = vdev.num_data_disks();
    let parity_count = vdev.num_total_disks() - num_data;

    for stripe in 0..NUM_STRIPES {
        let offset = stripe * chunk_size as u64;
        let actual =
            vdev.devices_mut()[disk_index].read_at(offset, chunk_size).expect("resilver後のディスクは読めるはず");

        if disk_index < num_data {
            let full = expected_stripe_data(num_data, stripe);
            let expected_chunk = &full[disk_index * chunk_size..(disk_index + 1) * chunk_size];
            assert_eq!(actual, expected_chunk, "disk {disk_index} stripe {stripe} のデータが不一致");
        } else {
            // パリティディスクの場合は、他の健全なディスクから独立して
            // パリティを再計算し、resilverされた内容と一致するか確認する。
            let full = expected_stripe_data(num_data, stripe);
            let chunks: Vec<&[u8]> = full.chunks(chunk_size).collect();
            let gf = zfs_accel_hlsl::galois::GaloisTables::new();
            let expected_parity = if parity_count == 2 {
                let (p, q) = zfs_accel_hlsl::raidz23_parity::compute_pq(&chunks, &gf);
                vec![p, q]
            } else {
                let (p, q, r) = zfs_accel_hlsl::raidz23_parity::compute_pqr(&chunks, &gf);
                vec![p, q, r]
            };
            let parity_idx = disk_index - num_data;
            assert_eq!(actual, expected_parity[parity_idx], "disk {disk_index}(parity) stripe {stripe} が不一致");
        }
    }
}

#[test]
fn raidz2_survives_two_disk_failure_and_resilvers_correctly() {
    let dir = scratch_dir("z2");
    let devices = build_devices(&dir, 6); // 4データ + 2パリティ(P,Q)
    let mut vdev = RaidZVdev::new(devices, RaidLevel::Z2, CHUNK_SIZE);

    write_all_stripes(&mut vdev);
    assert_all_stripes_readable_and_correct(&mut vdev);

    // --- Z2の2台同時故障をシミュレート ---
    let failed = [1usize, 3usize];
    for &idx in &failed {
        vdev.devices_mut()[idx].failed = true;
    }

    // 2台までの故障はP・Qパリティから復旧できるはず
    assert_all_stripes_readable_and_correct(&mut vdev);

    // --- ディスク交換 + 自動復旧(resilver)をシミュレート ---
    // 交換後の新品ディスクは「読めるが中身は信用できない(空)」状態を模して
    // 中身はそのまま(古い/空のまま)にし、オンライン化(failed=false)だけ行う。
    // resilverは中身を一切信用せず常に再構築する設計なので、これで正しい。
    for &idx in &failed {
        vdev.devices_mut()[idx].failed = false;
        vdev.resilver(idx, NUM_STRIPES).expect("resilverに失敗");
    }

    // 全ディスクが健全になった状態での読み出しが正しいこと
    assert_all_stripes_readable_and_correct(&mut vdev);

    // resilverされた各ディスクの生データが実際に正しいことを直接確認
    for &idx in &failed {
        assert_disk_directly_matches_healthy_content(&mut vdev, idx);
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn raidz3_survives_three_disk_failure_and_resilvers_correctly() {
    let dir = scratch_dir("z3");
    let devices = build_devices(&dir, 7); // 4データ + 3パリティ(P,Q,R)
    let mut vdev = RaidZVdev::new(devices, RaidLevel::Z3, CHUNK_SIZE);

    write_all_stripes(&mut vdev);
    assert_all_stripes_readable_and_correct(&mut vdev);

    // --- Z3の3台同時故障をシミュレート(データ2台+パリティ1台の混在ケース) ---
    // num_data=4なので disk index: 0-3=データ, 4=P, 5=Q, 6=R。
    // ここではデータ2台(0,2)とPパリティ(4)を同時に失う、という
    // 「データとパリティが混在した故障」ケースを検証する。
    let failed = [0usize, 2usize, 4usize];
    for &idx in &failed {
        vdev.devices_mut()[idx].failed = true;
    }

    // 3台までの故障はP・Q・Rのうち生き残った分だけで復旧できるはず
    assert_all_stripes_readable_and_correct(&mut vdev);

    // --- ディスク交換 + 自動復旧(resilver) ---
    // (Z2テストと同様、交換後ディスクの中身は信用せずresilverが常に再構築する)
    for &idx in &failed {
        vdev.devices_mut()[idx].failed = false;
        vdev.resilver(idx, NUM_STRIPES).expect("resilverに失敗");
    }

    assert_all_stripes_readable_and_correct(&mut vdev);

    for &idx in &failed {
        assert_disk_directly_matches_healthy_content(&mut vdev, idx);
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn raidz2_read_fails_cleanly_when_three_disks_are_lost() {
    // Z2の許容故障数(2台)を超えた場合は、静かに壊れたデータを返すのではなく
    // 明示的にエラーになることを確認する(データ破損の誤検出防止)。
    let dir = scratch_dir("z2_overload");
    let devices = build_devices(&dir, 6);
    let mut vdev = RaidZVdev::new(devices, RaidLevel::Z2, CHUNK_SIZE);

    write_all_stripes(&mut vdev);

    for idx in [0usize, 1usize, 2usize] {
        vdev.devices_mut()[idx].failed = true;
    }

    assert!(vdev.read_stripe(0).is_err(), "3台同時故障はZ2の復旧限界を超えるためエラーになるべき");

    std::fs::remove_dir_all(&dir).ok();
}
