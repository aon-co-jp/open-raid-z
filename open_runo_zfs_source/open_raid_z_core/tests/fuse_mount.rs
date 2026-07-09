//! Linux実マウント(FUSE)の統合テスト。
//!
//! `mount_pool`が実際にLinux上へマウントされ、標準の`std::fs`経由で
//! ファイルの読み書き・作成・削除・リネーム・切り詰めができる
//! (=本物のファイルシステムとして機能している)ことを検証する。
//! `tests/winfsp_mount.rs`/`tests/winfsp_mount_file_ops.rs`のLinux版。

#![cfg(all(target_os = "linux", feature = "fuse_backend"))]

use open_raid_z_core::block_device::FileBackedDevice;
use open_raid_z_core::fuse_mount::mount_pool;
use open_raid_z_core::pool::Pool;
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};
use std::path::PathBuf;

const CHUNK_SIZE: usize = 4096;
const NUM_STRIPES: u64 = 8;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("open_runo_fuse_mount_it_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build_pool(disk_dir: &std::path::Path) -> Pool<RaidZVdev<FileBackedDevice>> {
    let devices: Vec<FileBackedDevice> = (0..6)
        .map(|i| {
            let path = disk_dir.join(format!("disk{i}.img"));
            FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * NUM_STRIPES).unwrap()
        })
        .collect();
    let vdev = RaidZVdev::new(devices, RaidLevel::Z2, CHUNK_SIZE);
    Pool::new(vdev, NUM_STRIPES)
}

#[test]
fn mounted_pool_supports_a_full_create_write_read_rename_delete_cycle() {
    let disk_dir = scratch_dir("disks");
    let mount_dir = scratch_dir("mnt");
    let pool = build_pool(&disk_dir);

    let session = mount_pool(pool, mount_dir.to_str().unwrap()).expect("FUSEマウントに失敗しました");

    // 新規作成 + 書き込み + 読み込み(マウント経由、grow_dataset不要の自動拡張)。
    let file_path = mount_dir.join("hello.txt");
    std::fs::write(&file_path, b"hello from fuse").expect("マウント上での新規作成に失敗");
    let read_back = std::fs::read(&file_path).expect("作成直後のファイルが読めない");
    assert_eq!(read_back, b"hello from fuse");

    // 一覧に出てくることの確認。
    let names: Vec<String> = std::fs::read_dir(&mount_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.contains(&"hello.txt".to_string()), "readdirにファイルが出てこない: {names:?}");

    // リネーム。
    let renamed_path = mount_dir.join("renamed.txt");
    std::fs::rename(&file_path, &renamed_path).expect("マウント上でのリネームに失敗");
    assert_eq!(std::fs::read(&renamed_path).unwrap(), b"hello from fuse");
    assert!(std::fs::metadata(&file_path).is_err(), "リネーム前の名前がまだ存在する");

    // 切り詰め(truncate)。
    {
        let f = std::fs::OpenOptions::new().write(true).open(&renamed_path).unwrap();
        f.set_len(5).expect("set_len(truncate)に失敗");
    }
    assert_eq!(std::fs::metadata(&renamed_path).unwrap().len(), 5);
    assert_eq!(std::fs::read(&renamed_path).unwrap(), b"hello");

    // 削除。
    std::fs::remove_file(&renamed_path).expect("マウント上での削除に失敗");
    assert!(std::fs::metadata(&renamed_path).is_err(), "削除したはずのファイルがまだ存在する");

    session.umount_and_join().ok();
    std::fs::remove_dir_all(&disk_dir).ok();
    std::fs::remove_dir_all(&mount_dir).ok();
}

#[test]
fn mounted_pool_streams_a_multi_stripe_file_and_reassembles_it_exactly() {
    let disk_dir = scratch_dir("disks_large");
    let mount_dir = scratch_dir("mnt_large");
    let pool = build_pool(&disk_dir);

    let session = mount_pool(pool, mount_dir.to_str().unwrap()).expect("FUSEマウントに失敗しました");

    // num_data(4) * chunk_size(4096) = 16384バイト/ストライプ。複数ストライプに
    // またがる、境界に揃っていないサイズのペイロードを書き込む。
    let payload: Vec<u8> = (0..40000u32).map(|i| (i % 251) as u8).collect();
    let file_path = mount_dir.join("video.bin");
    std::fs::write(&file_path, &payload).expect("大きめファイルの書き込みに失敗");
    let read_back = std::fs::read(&file_path).expect("大きめファイルの読み込みに失敗");
    assert_eq!(read_back, payload);
    assert_eq!(std::fs::metadata(&file_path).unwrap().len(), payload.len() as u64);

    session.umount_and_join().ok();
    std::fs::remove_dir_all(&disk_dir).ok();
    std::fs::remove_dir_all(&mount_dir).ok();
}
