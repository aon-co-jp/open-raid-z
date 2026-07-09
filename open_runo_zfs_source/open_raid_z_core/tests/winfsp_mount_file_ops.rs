//! WinFsp実マウント経由でのファイル操作(create/delete/rename/append/truncate)
//! の統合テスト。
//!
//! `tests/winfsp_mount.rs`は「既存データセットへの読み書き」のみを検証するが、
//! こちらは`mount.rs`が新たにサポートした、マウント先での`\<新しい名前>`の
//! 新規作成・削除・名前変更・追記・サイズ変更(`std::fs`のCreate/remove_file/
//! rename/OpenOptions::append/File::set_len)が実際のWindowsファイルシステム
//! APIから見て正しく動作することを検証する。
//!
//! 各テストは並列実行時に衝突しないよう、それぞれ別のドライブレターを使う。

#![cfg(feature = "winfsp_backend")]

use open_raid_z_core::block_device::FileBackedDevice;
use open_raid_z_core::mount::mount_pool;
use open_raid_z_core::pool::Pool;
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};
use std::io::{Read, Write};
use std::path::PathBuf;

const CHUNK_SIZE: usize = 4096;
const NUM_STRIPES: u64 = 8;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("open_runo_winfsp_mount_file_ops_it_{name}_{}", std::process::id()));
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

/// マウントに失敗した場合(WinFspサービス未起動など、実行環境依存の理由)は
/// テストをスキップする。`tests/winfsp_mount.rs`と同じ方針。
macro_rules! mount_or_skip {
    ($pool:expr, $mount_point:expr, $dir:expr) => {
        match mount_pool($pool, $mount_point) {
            Ok(host) => host,
            Err(e) => {
                eprintln!("WinFspマウントに失敗したためテストをスキップします: {e:?}");
                std::fs::remove_dir_all(&$dir).ok();
                return;
            }
        }
    };
}

#[test]
fn creating_writing_and_deleting_a_new_file_through_the_mount() {
    let dir = scratch_dir("create_delete");
    let pool = build_pool(&dir); // データセットは1つも事前作成しない
    let mut host = mount_or_skip!(pool, "Y:", dir);

    let file_path = "Y:\\brand_new.txt";
    // 事前にPool::create_datasetを呼んでいない、マウント経由の新規作成。
    std::fs::write(file_path, b"created through the mount").expect("マウント上での新規作成に失敗");
    let read_back = std::fs::read(file_path).expect("作成直後のファイルが読めない");
    assert_eq!(read_back, b"created through the mount");

    std::fs::remove_file(file_path).expect("マウント上でのファイル削除に失敗");
    let err = std::fs::metadata(file_path).expect_err("削除したはずのファイルがまだ存在する");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);

    host.unmount();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn renaming_a_file_through_the_mount_preserves_its_contents() {
    let dir = scratch_dir("rename");
    let pool = build_pool(&dir);
    let mut host = mount_or_skip!(pool, "X:", dir);

    let old_path = "X:\\before.txt";
    let new_path = "X:\\after.txt";
    std::fs::write(old_path, b"unchanged payload").unwrap();

    std::fs::rename(old_path, new_path).expect("マウント上でのリネームに失敗");

    let read_back = std::fs::read(new_path).expect("リネーム後のファイルが読めない");
    assert_eq!(read_back, b"unchanged payload");
    let err = std::fs::metadata(old_path).expect_err("リネーム前の名前がまだ存在する");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);

    host.unmount();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn appending_through_the_mount_extends_the_file_without_overwriting_it() {
    let dir = scratch_dir("append");
    let pool = build_pool(&dir);
    let mut host = mount_or_skip!(pool, "W:", dir);

    let file_path = "W:\\log.txt";
    {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(file_path).unwrap();
        f.write_all(b"line1;").unwrap();
    }
    {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(file_path).unwrap();
        f.write_all(b"line2;").unwrap();
    }

    let mut contents = String::new();
    std::fs::File::open(file_path).unwrap().read_to_string(&mut contents).unwrap();
    assert_eq!(contents, "line1;line2;");

    host.unmount();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn truncating_a_file_through_the_mount_shrinks_its_reported_size() {
    let dir = scratch_dir("truncate");
    let pool = build_pool(&dir);
    let mut host = mount_or_skip!(pool, "V:", dir);

    let file_path = "V:\\shrinkme.bin";
    std::fs::write(file_path, vec![0x99u8; 5000]).unwrap();
    assert_eq!(std::fs::metadata(file_path).unwrap().len(), 5000);

    {
        let f = std::fs::OpenOptions::new().write(true).open(file_path).unwrap();
        f.set_len(10).expect("set_file_sizeによる切り詰めに失敗");
    }

    assert_eq!(std::fs::metadata(file_path).unwrap().len(), 10);
    let read_back = std::fs::read(file_path).unwrap();
    assert_eq!(read_back, vec![0x99u8; 10]);

    host.unmount();
    std::fs::remove_dir_all(&dir).ok();
}
