//! `Pool::read_unaligned` / `Pool::write_unaligned` の統合テスト。
//!
//! `Pool::read`/`Pool::write`はストライプ境界に一致するオフセット・長さ
//! しか受け付けないが(WinFspマウント層(`mount.rs`)がこの制約を抱えたまま
//! なっている主因)、`read_unaligned`/`write_unaligned`はread-modify-write
//! により任意のバイトオフセット・任意長の読み書きを提供する。

use open_zfs_winfsp_bridge::block_device::FileBackedDevice;
use open_zfs_winfsp_bridge::pool::Pool;
use open_zfs_winfsp_bridge::vdev::{RaidLevel, RaidZVdev};
use std::path::PathBuf;

const CHUNK_SIZE: usize = 64;
const NUM_STRIPES: u64 = 8;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("open_runo_unaligned_io_it_{name}_{}", std::process::id()));
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

/// num_data(4) * chunk_size(64) = 256バイト/ストライプ
fn stripe_bytes() -> u64 {
    4 * CHUNK_SIZE as u64
}

#[test]
fn write_unaligned_then_read_unaligned_round_trips_within_a_single_stripe() {
    let dir = scratch_dir("single_stripe");
    let mut pool = build_pool(&dir);
    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", stripe_bytes()).unwrap();

    // ストライプ境界(256バイト)に一切揃っていない、オフセット10・長さ37の書き込み。
    let offset = 10u64;
    let payload: Vec<u8> = (0..37u8).collect();
    pool.write_unaligned("ds", offset, &payload).unwrap();

    let read_back = pool.read_unaligned("ds", offset, payload.len() as u64).unwrap();
    assert_eq!(read_back, payload);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn write_unaligned_spanning_multiple_stripes_round_trips() {
    let dir = scratch_dir("multi_stripe");
    let mut pool = build_pool(&dir);
    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", 3 * stripe_bytes()).unwrap();

    // 1ストライプ目の途中から始まり、3ストライプ目の途中で終わる書き込み。
    let offset = stripe_bytes() - 50;
    let len = stripe_bytes() + 100; // 3ストライプにまたがる
    let payload: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
    pool.write_unaligned("ds", offset, &payload).unwrap();

    let read_back = pool.read_unaligned("ds", offset, len).unwrap();
    assert_eq!(read_back, payload);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn write_unaligned_preserves_untouched_bytes_around_the_written_region() {
    let dir = scratch_dir("preserve");
    let mut pool = build_pool(&dir);
    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", stripe_bytes()).unwrap();

    // まずストライプ全体を既知の値で埋める。
    let baseline: Vec<u8> = vec![0xAAu8; stripe_bytes() as usize];
    pool.write("ds", 0, &baseline).unwrap();

    // ストライプの真ん中あたりだけを、境界に揃っていない範囲で上書きする。
    let overwrite_offset = 100u64;
    let overwrite: Vec<u8> = vec![0x55u8; 10];
    pool.write_unaligned("ds", overwrite_offset, &overwrite).unwrap();

    let whole = pool.read("ds", 0, stripe_bytes()).unwrap();
    // 上書きした範囲だけが変化し、それ以外はbaselineのまま。
    assert!(whole[..overwrite_offset as usize].iter().all(|&b| b == 0xAA));
    assert_eq!(
        &whole[overwrite_offset as usize..(overwrite_offset + 10) as usize],
        overwrite.as_slice()
    );
    assert!(whole[(overwrite_offset + 10) as usize..].iter().all(|&b| b == 0xAA));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn read_unaligned_with_zero_length_returns_empty_without_touching_the_pool() {
    let dir = scratch_dir("zero_len");
    let mut pool = build_pool(&dir);
    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", stripe_bytes()).unwrap();

    assert_eq!(pool.read_unaligned("ds", 12, 0).unwrap(), Vec::<u8>::new());
    // write_unalignedも同様に空データは即座に成功として扱う。
    pool.write_unaligned("ds", 12, &[]).unwrap();

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn write_unaligned_beyond_allocated_capacity_fails_cleanly_and_leaves_existing_data_untouched() {
    let dir = scratch_dir("overflow");
    let mut pool = build_pool(&dir);
    pool.create_dataset("ds").unwrap();
    pool.grow_dataset("ds", stripe_bytes()).unwrap(); // 割当は1ストライプ分のみ

    let baseline: Vec<u8> = vec![0x11u8; stripe_bytes() as usize];
    pool.write("ds", 0, &baseline).unwrap();

    // 割当容量(1ストライプ=256バイト)を超える範囲への書き込みは拒否される。
    let overflowing_payload = vec![0xFFu8; 20];
    let result = pool.write_unaligned("ds", stripe_bytes() - 10, &overflowing_payload);
    assert!(result.is_err());

    // 失敗時、既存データは一切変化していない(read-modify-writeが未完了のまま中断されている)。
    let whole = pool.read("ds", 0, stripe_bytes()).unwrap();
    assert_eq!(whole, baseline);

    std::fs::remove_dir_all(&dir).ok();
}
