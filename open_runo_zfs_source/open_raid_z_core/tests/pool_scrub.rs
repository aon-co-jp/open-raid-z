//! `Pool::scrub` / `Pool::vdev_mut` の統合テスト。
//!
//! `scrub`(チェックサム不一致=サイレント破損の検知・自己修復)自体は
//! `RaidZVdev`/`Raid10Vdev`のどちらにも実装済みだったが、`Pool`が`vdev`
//! フィールドを非公開で保持するため、`Pool`しか持たない呼び出し側からは
//! 一切呼び出せないという抜けがあった。本テストは、`Pool::scrub`が
//! RAID-Z系・RAID10いずれのバックエンドでも正しく機能することと、
//! `Pool::vdev_mut`経由でRAID10固有の`resilver`(シグネチャがRAID-Z系と
//! 異なるため`Vdev`トレイトには未統一)を呼び出せることを検証する。

use open_raid_z_core::block_device::{BlockDevice, FaultInjectableDevice, FileBackedDevice};
use open_raid_z_core::pool::Pool;
use open_raid_z_core::raid10::Raid10Vdev;
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};
use std::path::PathBuf;

// 512(以前は64): メタデータ予約ストライプ数はチャンクサイズに反比例して
// 増える(`pool.rs`の`superblock_stripe_count`参照)。64バイトのように
// 極端に小さいチャンクサイズは実運用ではまず使われない非現実的な値であり、
// このテストの意図(scrub/resilverの検証)には無関係なので、予約比率が
// 過大にならない程度の現実的な値へ引き上げた。
const CHUNK_SIZE: usize = 512;
const NUM_STRIPES: u64 = 8;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("open_runo_pool_scrub_it_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn stripe_bytes() -> u64 {
    4 * CHUNK_SIZE as u64 // num_data(4) * chunk_size (Z2: 6台-2パリティ)
}

/// 直接ディスクの中身だけを壊す(`failed`フラグは立てない、ビットロットの
/// シミュレーション)。ディスクの生バイトへは`Pool::vdev_mut`経由でしか
/// 到達できない(scrubがPool経由で呼べることの検証も兼ねて、意図的に
/// `Pool`のAPIだけを使って完結させる)。
fn corrupt_disk_directly<D: BlockDevice>(vdev: &mut RaidZVdev<D>, disk_index: usize, stripe: u64) {
    let offset = stripe * CHUNK_SIZE as u64;
    let mut garbage = vdev.devices_mut()[disk_index].read_at(offset, CHUNK_SIZE).unwrap();
    for b in garbage.iter_mut() {
        *b ^= 0xFF;
    }
    vdev.devices_mut()[disk_index].write_at(offset, &garbage).unwrap();
}

#[test]
fn pool_scrub_detects_and_heals_corruption_on_a_raidz_backed_pool() {
    let dir = scratch_dir("raidz");
    let devices: Vec<FileBackedDevice> = (0..6)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * NUM_STRIPES).unwrap()
        })
        .collect();
    let vdev = RaidZVdev::new(devices, RaidLevel::Z2, CHUNK_SIZE);
    let mut pool = Pool::new(vdev, NUM_STRIPES);

    pool.create_dataset("tank").unwrap();
    // メタデータ予約ストライプ数は`total_stripes`に応じて動的に決まるため、
    // ハードコードせず実際の空き容量から逆算する。CoW書き込み用に1ストライプ
    // ぶんの余白を残す。
    let usable_stripes = pool.usage().free_stripes - 1;
    pool.grow_dataset("tank", usable_stripes * stripe_bytes()).unwrap();

    let payload: Vec<u8> = (0..usable_stripes * stripe_bytes()).map(|i| (i % 251) as u8).collect();
    pool.write("tank", 0, &payload).unwrap();

    // Poolを経由したままではディスクへ直接アクセスできないため、
    // `Pool::vdev_mut`(今回追加したエスケープハッチ)経由でビットロットを注入する。
    corrupt_disk_directly(pool.vdev_mut(), 2, 3);

    // `Pool::scrub`(今回追加)がプール全体をスキャンし、破損を検知・修復する。
    let report = pool.scrub().expect("Pool::scrubに失敗");
    assert_eq!(report.stripes_scanned, NUM_STRIPES);
    assert_eq!(report.corruptions_healed, 1);

    assert_eq!(pool.read("tank", 0, usable_stripes * stripe_bytes()).unwrap(), payload);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn pool_scrub_works_on_a_raid10_backed_pool_via_the_shared_vdev_trait() {
    let dir = scratch_dir("raid10");
    let devices: Vec<FileBackedDevice> = (0..4)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * NUM_STRIPES).unwrap()
        })
        .collect();
    let vdev = Raid10Vdev::new(devices, 2, CHUNK_SIZE).unwrap(); // 2ミラーグループ
    let total_stripes = 2 * NUM_STRIPES;
    let mut pool = Pool::new(vdev, total_stripes);

    pool.create_dataset("tank").unwrap();
    // メタデータ予約ストライプ数は`total_stripes`に応じて動的に決まるため、
    // ハードコードせず実際の空き容量から逆算する。CoW書き込み用に1ストライプ
    // ぶんの余白を残す。
    let usable_stripes = pool.usage().free_stripes - 1;
    pool.grow_dataset("tank", usable_stripes * CHUNK_SIZE as u64).unwrap();

    let payload: Vec<u8> = (0..usable_stripes * CHUNK_SIZE as u64)
        .map(|i| (i % 251) as u8)
        .collect();
    pool.write("tank", 0, &payload).unwrap();

    // グループ0の2台目のミラーメンバー(グローバルストライプ2、内部ストライプ1)を
    // 直接破壊する。メタデータ予約ストライプ数が変わっても、2グループの
    // ラウンドロビンでグループ0が担当するのは偶数番のグローバルストライプで
    // あり、実際に書き込み済みの範囲(先頭から`usable_stripes`ぶん)に
    // グローバルストライプ2が含まれることに変わりは無い。
    let inner_stripe_offset = CHUNK_SIZE as u64; // グローバルストライプ2 = グループ0の内部ストライプ1
    let mut garbage =
        pool.vdev_mut().group_devices_mut(0)[1].read_at(inner_stripe_offset, CHUNK_SIZE).unwrap();
    for b in garbage.iter_mut() {
        *b ^= 0xFF;
    }
    pool.vdev_mut().group_devices_mut(0)[1].write_at(inner_stripe_offset, &garbage).unwrap();

    // `Vdev`トレイトに統一された`scrub`のおかげで、RaidZVdevと全く同じ
    // `Pool::scrub()`呼び出しがRaid10Vdev上でも機能する。
    let report = pool.scrub().expect("Pool::scrubに失敗");
    assert_eq!(report.stripes_scanned, total_stripes);
    assert_eq!(report.corruptions_healed, 1);

    assert_eq!(pool.read("tank", 0, usable_stripes * CHUNK_SIZE as u64).unwrap(), payload);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn pool_vdev_mut_allows_raid10_specific_resilver_not_yet_unified_in_the_vdev_trait() {
    let dir = scratch_dir("resilver");
    let devices: Vec<FaultInjectableDevice<FileBackedDevice>> = (0..4)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            FaultInjectableDevice::new(
                FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * NUM_STRIPES).unwrap(),
            )
        })
        .collect();
    let vdev = Raid10Vdev::new(devices, 2, CHUNK_SIZE).unwrap();
    let total_stripes = 2 * NUM_STRIPES;
    let mut pool = Pool::new(vdev, total_stripes);

    pool.create_dataset("tank").unwrap();
    let usable_stripes = pool.usage().free_stripes - 1;
    pool.grow_dataset("tank", usable_stripes * CHUNK_SIZE as u64).unwrap();

    let payload: Vec<u8> = (0..usable_stripes * CHUNK_SIZE as u64)
        .map(|i| (i * 7 % 251) as u8)
        .collect();
    pool.write("tank", 0, &payload).unwrap();

    // グループ1の1台目のディスクを物理故障扱いにし、resilverで再構築する。
    pool.vdev_mut().group_devices_mut(1)[0].failed = true;
    pool.vdev_mut().group_devices_mut(1)[0].failed = false; // 交換直後、中身は信用しない
    pool.vdev_mut().resilver(1, 0, NUM_STRIPES).unwrap();

    assert_eq!(pool.read("tank", 0, usable_stripes * CHUNK_SIZE as u64).unwrap(), payload);

    std::fs::remove_dir_all(&dir).ok();
}
