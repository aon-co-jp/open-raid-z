//! `foreign_fs`のext2/ext4読み取りブリッジ(`ForeignExt4Volume`)の統合テスト。
//!
//! フィクスチャ`tests/fixtures/ext4_small.img`は、WSL2 Ubuntu上で
//! 以下のコマンドにより生成した512KiBのext4イメージ(root不要):
//!
//! ```sh
//! dd if=/dev/zero of=ext4_small.img bs=1024 count=512
//! mkfs.ext4 -q -b 1024 -O ^has_journal -L ORZTEST ext4_small.img
//! echo "hello from ext4 fixture" > hello.txt          # 24バイト(改行含む)
//! printf "%s" "0123456789abcdef" > bin16.dat          # 16バイト
//! debugfs -w -R "write hello.txt hello.txt" ext4_small.img
//! debugfs -w -R "mkdir subdir" ext4_small.img
//! debugfs -w -R "write bin16.dat subdir/nested.dat" ext4_small.img
//! ```
//!
//! `mkfs.ext4`(e2fsprogs 1.47系)の実出力を読むことで、自前実装ではなく
//! 「本物のext4を読める」ことを担保する。

#![cfg(feature = "foreign_fs")]

use open_raid_z_core::foreign_fs::ForeignExt4Volume;
use std::path::PathBuf;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ext4_small.img")
}

#[test]
fn opens_real_mkfs_ext4_image() {
    ForeignExt4Volume::open(fixture_path()).expect("mkfs.ext4製のイメージを開けること");
}

#[test]
fn open_rejects_non_ext4_image() {
    // 明らかにext4ではない入力(このテストソース自身)はエラーになること。
    let err = ForeignExt4Volume::open(file!());
    assert!(err.is_err(), "非ext4ファイルはエラーになるべき");
}

#[test]
fn lists_root_directory() {
    let vol = ForeignExt4Volume::open(fixture_path()).unwrap();
    let entries = vol.list_dir("/").unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"hello.txt"), "ルートにhello.txtが見えること: {names:?}");
    assert!(names.contains(&"subdir"), "ルートにsubdirが見えること: {names:?}");
    assert!(names.contains(&"lost+found"), "mkfs.ext4が作るlost+foundも見えること: {names:?}");
    // "."/".."は一覧に含めない(FAT32/exFAT版と同じ振る舞い)
    assert!(!names.contains(&".") && !names.contains(&".."));

    let hello = entries.iter().find(|e| e.name == "hello.txt").unwrap();
    assert!(!hello.is_dir);
    assert_eq!(hello.size_bytes, 24, "hello.txtのサイズ(24バイト)が取れること");
    let subdir = entries.iter().find(|e| e.name == "subdir").unwrap();
    assert!(subdir.is_dir);
}

#[test]
fn lists_subdirectory() {
    let vol = ForeignExt4Volume::open(fixture_path()).unwrap();
    let entries = vol.list_dir("/subdir").unwrap();
    let nested = entries.iter().find(|e| e.name == "nested.dat").expect("subdir/nested.datが見えること");
    assert!(!nested.is_dir);
    assert_eq!(nested.size_bytes, 16);
}

#[test]
fn reads_file_contents_byte_exact() {
    let vol = ForeignExt4Volume::open(fixture_path()).unwrap();
    assert_eq!(vol.read_file("/hello.txt").unwrap(), b"hello from ext4 fixture\n");
    assert_eq!(vol.read_file("/subdir/nested.dat").unwrap(), b"0123456789abcdef");
}

#[test]
fn path_normalization_accepts_windows_style_and_relative() {
    let vol = ForeignExt4Volume::open(fixture_path()).unwrap();
    // 先頭スラッシュ無し・バックスラッシュ始まりでも同じファイルへ届くこと
    assert_eq!(vol.read_file("hello.txt").unwrap(), b"hello from ext4 fixture\n");
    assert_eq!(vol.read_file("\\hello.txt").unwrap(), b"hello from ext4 fixture\n");
    assert!(!vol.list_dir("").unwrap().is_empty(), "空文字はルート扱い");
}

#[test]
fn missing_paths_return_errors() {
    let vol = ForeignExt4Volume::open(fixture_path()).unwrap();
    assert!(vol.read_file("/no_such_file.txt").is_err());
    assert!(vol.list_dir("/no_such_dir").is_err());
    // ディレクトリをファイルとして読もうとした場合もエラー
    assert!(vol.read_file("/subdir").is_err());
}

#[test]
fn write_operations_are_rejected_as_read_only() {
    let vol = ForeignExt4Volume::open(fixture_path()).unwrap();
    assert!(vol.write_file("/new.txt", b"x").is_err(), "書き込みは読み取り専用エラー");
    assert!(vol.create_dir("/newdir").is_err(), "mkdirは読み取り専用エラー");
    assert!(vol.remove("/hello.txt").is_err(), "削除は読み取り専用エラー");
    // エラーになっても既存内容は無傷であること
    assert_eq!(vol.read_file("/hello.txt").unwrap(), b"hello from ext4 fixture\n");
}
