//! `Pool`が`Raid10Vdev`(RaidZVdev以外のvdev実装)でも正しく動作することの
//! 統合テスト。
//!
//! `Pool`は`crate::vdev::Vdev`トレイトへ一般化されているため、
//! `RaidZVdev`と全く同じデータセット/スナップショット/CoW APIを
//! `Raid10Vdev`に対しても使えるはずである。本テストはそれを、
//! 実際にデータセットを作成・書き込み・読み出しすることで検証する。

use openzfs_winfsp_bridge::block_device::FileBackedDevice;
use openzfs_winfsp_bridge::pool::Pool;
use openzfs_winfsp_bridge::raid10::Raid10Vdev;
use std::path::PathBuf;

const CHUNK_SIZE: usize = 64;
const STRIPES_PER_GROUP: u64 = 8;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openruno_raid10_pool_it_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn pool_creates_and_round_trips_a_dataset_on_top_of_raid10() {
    let dir = scratch_dir("basic");
    let devices: Vec<FileBackedDevice> = (0..4)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * STRIPES_PER_GROUP).unwrap()
        })
        .collect();
    let vdev = Raid10Vdev::new(devices, 2, CHUNK_SIZE).unwrap(); // 2ミラーグループ
    assert_eq!(vdev.num_groups(), 2);

    // Vdev::num_data_disks()は常に1(1書き込み=1グループぶん=chunk_sizeバイト)なので、
    // グローバルストライプ数はグループ数 * グループ内ストライプ数。
    let total_stripes = 2 * STRIPES_PER_GROUP;
    let mut pool = Pool::new(vdev, total_stripes);

    assert_eq!(pool.usage().total_stripes, total_stripes);

    // CoW書き込みは常に新しい空きストライプへ先に書いてから参照を切り替えるため、
    // データセットにはプール容量を丸ごと割り当てず、CoW用の空き(1ストライプ以上)を
    // 残しておく必要がある(RaidZVdevでもRaid10Vdevでも共通のPoolの制約)。
    let dataset_stripes = total_stripes - 1;
    pool.create_dataset("tank").unwrap();
    pool.grow_dataset("tank", dataset_stripes * CHUNK_SIZE as u64).unwrap();
    assert_eq!(pool.dataset_size("tank").unwrap(), dataset_stripes * CHUNK_SIZE as u64);

    let payload: Vec<u8> = (0..dataset_stripes * CHUNK_SIZE as u64).map(|i| (i % 251) as u8).collect();
    pool.write("tank", 0, &payload).unwrap();
    assert_eq!(pool.read("tank", 0, dataset_stripes * CHUNK_SIZE as u64).unwrap(), payload);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn pool_cow_write_relocates_stripe_on_raid10_just_like_raidz() {
    // Pool::writeのCoW(新しい空きストライプへ書いてから参照を切り替える)は
    // vdevの実装に依存しないPool自身のロジックなので、Raid10Vdevでも
    // 同じ性質(旧ストライプは上書きされない)が成り立つはずである。
    let dir = scratch_dir("cow");
    let devices: Vec<FileBackedDevice> = (0..4)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * STRIPES_PER_GROUP).unwrap()
        })
        .collect();
    let vdev = Raid10Vdev::new(devices, 2, CHUNK_SIZE).unwrap();
    let total_stripes = 2 * STRIPES_PER_GROUP;
    let mut pool = Pool::new(vdev, total_stripes);

    pool.create_dataset("tank").unwrap();
    pool.grow_dataset("tank", CHUNK_SIZE as u64).unwrap(); // 1論理ストライプぶん

    let first = vec![0xAAu8; CHUNK_SIZE];
    pool.write("tank", 0, &first).unwrap();
    let phys_after_first = pool.physical_stripe_for("tank", 0).unwrap();

    let second = vec![0xBBu8; CHUNK_SIZE];
    pool.write("tank", 0, &second).unwrap();
    let phys_after_second = pool.physical_stripe_for("tank", 0).unwrap();

    assert_ne!(phys_after_first, phys_after_second, "CoWにより物理ストライプが変わるはず");
    assert_eq!(pool.read("tank", 0, CHUNK_SIZE as u64).unwrap(), second);

    std::fs::remove_dir_all(&dir).ok();
}
