//! コピーオンライト(CoW)のTESTモード統合テスト。
//!
//! `Pool::write`が実際に「新しい空き物理ストライプへ書いてから参照を
//! 切り替える」方式で動作しており、書き込み中にクラッシュしても旧データが
//! 破壊されないことを、実ファイルI/Oを経由して検証する。

use open_raid_z_core::block_device::FileBackedDevice;
use open_raid_z_core::pool::Pool;
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};
use std::path::PathBuf;

const CHUNK_SIZE: usize = 64;
const NUM_STRIPES: u64 = 8;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("open_runo_cow_it_{name}_{}", std::process::id()));
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
fn overwrite_relocates_to_a_new_physical_stripe_instead_of_overwriting_in_place() {
    let dir = scratch_dir("relocate");
    let mut pool = build_pool(&dir);
    let sb = stripe_bytes();

    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", sb).unwrap();

    let v1 = vec![0xAAu8; sb as usize];
    pool.write("ds", 0, &v1).unwrap();
    let phys_after_v1 = pool.physical_stripe_for("ds", 0).unwrap();

    let v2 = vec![0xBBu8; sb as usize];
    pool.write("ds", 0, &v2).unwrap();
    let phys_after_v2 = pool.physical_stripe_for("ds", 0).unwrap();

    assert_ne!(
        phys_after_v1, phys_after_v2,
        "CoWなら上書きのたびに物理ストライプが変わるはず(in-place上書きになっていない)"
    );

    // 論理読み出しは常に最新のデータを返す
    assert_eq!(pool.read("ds", 0, sb).unwrap(), v2);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn old_physical_stripe_is_untouched_until_the_reference_switch_completes() {
    // CoWの核心: 新データを書いている間、旧データが指しているストライプの
    // 中身は一切変更されない。これにより書き込み中のクラッシュでも
    // データが破壊されない(参照切り替え=単一のポインタ更新のみがクリティカル)。
    let dir = scratch_dir("crash_safety");
    let mut pool = build_pool(&dir);
    let sb = stripe_bytes();

    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", sb).unwrap();

    let v1 = vec![0x11u8; sb as usize];
    pool.write("ds", 0, &v1).unwrap();
    let old_phys = pool.physical_stripe_for("ds", 0).unwrap();

    // 新データを書き込む(この時点でold_physの中身はまだv1のまま)
    let v2 = vec![0x22u8; sb as usize];
    pool.write("ds", 0, &v2).unwrap();
    let new_phys = pool.physical_stripe_for("ds", 0).unwrap();
    assert_ne!(old_phys, new_phys);

    // 新しい物理ストライプにはv2が書かれている
    assert_eq!(pool.read_physical_stripe(new_phys).unwrap(), v2);

    // "クラッシュ直前"を模して、参照切り替えが起きる直前の状態を再現する:
    // old_physの生データは、write呼び出しの間ずっとv1のままだったはず
    // (write完了後は空き領域に戻っているが、中身自体はまだv1が残っている
    // ことをもって「上書きされていない」ことを確認する)
    let old_phys_raw = pool.read_physical_stripe(old_phys).unwrap();
    assert_eq!(
        old_phys_raw, v1,
        "旧物理ストライプの中身は新データ書き込みによって変更されてはいけない"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn repeated_overwrites_reclaim_old_stripes_without_leaking_pool_capacity() {
    let dir = scratch_dir("no_leak");
    let mut pool = build_pool(&dir);
    let sb = stripe_bytes();

    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", sb).unwrap();
    let used_after_grow = pool.usage().used_stripes;

    // 何度上書きしても、古いストライプはプールへ返却されるため
    // 使用容量は変わらない(リークしない)
    for round in 0..10u8 {
        let v = vec![round; sb as usize];
        pool.write("ds", 0, &v).unwrap();
        assert_eq!(pool.usage().used_stripes, used_after_grow, "round {round}");
    }

    assert_eq!(pool.read("ds", 0, sb).unwrap(), vec![9u8; sb as usize]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn write_fails_cleanly_when_pool_has_no_free_stripe_for_cow() {
    // プールの空き容量を使い切っている状態でも、grow済みの範囲への
    // "更新"書き込みには最低1つの空きストライプ(CoWの一時退避先)が必要。
    // これは実際のZFSでも「プールが100%埋まっていると更新すら失敗しうる」
    // という実際の制約に対応する、忠実な挙動。
    let dir = scratch_dir("full_pool");
    let mut pool = build_pool(&dir);
    let sb = stripe_bytes();

    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", NUM_STRIPES * sb).unwrap(); // プール容量を使い切る
    assert_eq!(pool.usage().free_stripes, 0);

    let v = vec![0x42u8; sb as usize];
    let result = pool.write("ds", 0, &v);
    assert!(result.is_err(), "空きストライプが無い状態でのCoW書き込みは失敗するはず");

    std::fs::remove_dir_all(&dir).ok();
}
