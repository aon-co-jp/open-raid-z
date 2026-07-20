//! `Pool::read_unaligned` / `Pool::write_unaligned` の統合テスト。
//!
//! `Pool::read`/`Pool::write`はストライプ境界に一致するオフセット・長さ
//! しか受け付けないが(WinFspマウント層(`mount.rs`)がこの制約を抱えたまま
//! なっている主因)、`read_unaligned`/`write_unaligned`はread-modify-write
//! により任意のバイトオフセット・任意長の読み書きを提供する。

use open_raid_z_core::block_device::FileBackedDevice;
use open_raid_z_core::pool::Pool;
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};
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
    assert_eq!(&whole[overwrite_offset as usize..(overwrite_offset + 10) as usize], overwrite.as_slice());
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

/// 追記21/24で報告された「chunk_size=65536・RAID-Z2・4ディスクでの
/// FUSEストリーミング書き込み時にストライプ境界付近でバイトが破損する」
/// 疑いのある実バグを再現するための回帰テスト。
///
/// FUSEの典型的な書き込みバッファサイズ(128KiB=131072バイト)が、
/// このchunk_size(65536)・data_disks=2構成での1ストライプぶんの
/// 論理バイト数(131072バイト)とたまたま一致することが引き金になっている
/// 疑いが報告されていたため、実際に131072バイト単位の`write_unaligned_growing`
/// 呼び出しを何度も繰り返す(`cp`のストリーミング書き込みを模す)ことで
/// 再現を試みる。総サイズはストライプ境界に揃っていない
/// (最後の書き込みだけ半端なサイズ)、実際の`cp`と同じ状況にしている。
#[test]
fn streaming_writes_with_fuse_sized_buffer_are_byte_exact_across_stripe_boundaries() {
    const REALISTIC_CHUNK_SIZE: usize = 65536;
    const REALISTIC_NUM_STRIPES: u64 = 20;

    let dir = scratch_dir("fuse_streaming_65536");
    let devices: Vec<FileBackedDevice> = (0..4)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            FileBackedDevice::create_fixed_size(&path, REALISTIC_CHUNK_SIZE as u64 * REALISTIC_NUM_STRIPES).unwrap()
        })
        .collect();
    let vdev = RaidZVdev::new(devices, RaidLevel::Z2, REALISTIC_CHUNK_SIZE);
    let mut pool = Pool::new(vdev, REALISTIC_NUM_STRIPES);

    pool.create_dataset("ds").unwrap();

    // data_disks = 4 - 2(Z2) = 2 -> 1ストライプ = 65536 * 2 = 131072バイト。
    let stripe_bytes = 2 * REALISTIC_CHUNK_SIZE as u64;
    assert_eq!(stripe_bytes, 131_072);

    // FUSEの典型的な書き込みバッファサイズと同じ131072バイト単位で
    // ストリーミング書き込みする。最後だけ半端なサイズ(ストライプ境界に
    // 揃っていない)にして、cpによる実際のファイルコピーの終端を模す。
    let write_buffer_size = 131_072usize;
    let total_len = write_buffer_size * 8 + 40_000; // 複数ストライプ+半端な残り

    let payload: Vec<u8> = (0..total_len).map(|i| (i % 251) as u8).collect();

    let mut offset = 0u64;
    for chunk in payload.chunks(write_buffer_size) {
        pool.write_unaligned_growing("ds", offset, chunk).unwrap();
        offset += chunk.len() as u64;
    }

    let read_back = pool.read_unaligned("ds", 0, total_len as u64).unwrap();
    for (i, (expected, actual)) in payload.iter().zip(read_back.iter()).enumerate() {
        assert_eq!(
            expected,
            actual,
            "バイト位置{i}が不一致(ストライプ境界={}, ストライプ内オフセット={})",
            i as u64 / stripe_bytes,
            i as u64 % stripe_bytes
        );
    }
    assert_eq!(payload.len(), read_back.len());

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
