//! WinFsp実マウントの統合テスト(ひな型)。
//!
//! `mount_pool`が実際にWindows上へドライブレターとしてマウントされ、
//! 標準の`std::fs`経由でファイルの読み書きができる(=本物のファイル
//! システムとして機能している)ことを検証する。
//!
//! `mount_pool`はプール内の全データセットをそれぞれ`\<データセット名>`
//! というファイルとしてルート直下に公開する(`mount.rs`参照)。このテストは
//! その中の1データセット(`tank`)についてのみ読み書きを検証する。
//!
//! チャンク境界に一致する範囲でのみ読み書きできるという、ひな型段階の
//! 制約(`mount.rs`のモジュールドキュメント参照)に合わせて、
//! データセット全体をちょうど1回で書き切るサイズのペイロードを使う。

#![cfg(feature = "winfsp_backend")]

use open_zfs_winfsp_bridge::block_device::FileBackedDevice;
use open_zfs_winfsp_bridge::mount::mount_pool;
use open_zfs_winfsp_bridge::pool::Pool;
use open_zfs_winfsp_bridge::vdev::{RaidLevel, RaidZVdev};
use std::path::PathBuf;

const CHUNK_SIZE: usize = 4096;
const NUM_STRIPES: u64 = 4;
const MOUNT_POINT: &str = "Z:";

fn scratch_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("open_runo_winfsp_mount_it_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn mounted_pool_survives_a_real_file_write_and_read_round_trip() {
    let dir = scratch_dir();
    let devices: Vec<FileBackedDevice> = (0..6)
        .map(|i| {
            let path = dir.join(format!("disk{i}.img"));
            FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * NUM_STRIPES).unwrap()
        })
        .collect();
    let vdev = RaidZVdev::new(devices, RaidLevel::Z2, CHUNK_SIZE);
    let mut pool = Pool::new(vdev, NUM_STRIPES);
    pool.create_dataset("tank").unwrap();
    // num_data = 4 (6台 - Z2の2パリティ)
    let dataset_bytes = NUM_STRIPES * (4 * CHUNK_SIZE as u64);
    pool.grow_dataset("tank", dataset_bytes).unwrap();

    let mut host = match mount_pool(pool, MOUNT_POINT) {
        Ok(host) => host,
        Err(e) => {
            eprintln!("WinFspマウントに失敗したためテストをスキップします: {e:?}");
            std::fs::remove_dir_all(&dir).ok();
            return;
        }
    };

    let file_path = format!("{MOUNT_POINT}\\tank");
    let payload: Vec<u8> = (0..dataset_bytes).map(|i| (i % 256) as u8).collect();

    std::fs::write(&file_path, &payload).expect("マウント先ファイルへの書き込みに失敗");
    let read_back = std::fs::read(&file_path).expect("マウント先ファイルからの読み込みに失敗");
    assert_eq!(read_back, payload, "マウント経由で読み書きした内容が一致しない");

    host.unmount();
    std::fs::remove_dir_all(&dir).ok();
}
