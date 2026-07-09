//! `foreign_fs`モジュールの実動作確認用スモークテスト。
//!
//! 実USBメモリ/SDカードでの検証は別途必要だが、まずは:
//! 1. 新規イメージファイルを`fatfs::format_volume`でFAT32フォーマットする
//!    (これは他OS標準ツールでFAT32フォーマットした場合と同じオンディスク
//!    構造になる。`fatfs`自体がFAT仕様準拠のフォーマッタを提供している)。
//! 2. `ForeignFatVolume`(本プロジェクトの実装)でファイルを書き込み、
//!    ディレクトリを一覧し、読み戻して内容が一致するかを確認する。
//!
//! 実行方法:
//!   cargo run --no-default-features --features foreign_fs --example foreign_fs_smoke_test

use open_raid_z_core::foreign_fs::ForeignFatVolume;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};

fn main() {
    let img_path = std::env::temp_dir().join("orz_foreign_fs_smoke.img");

    // 1. 64MiBの空イメージを作成し、fatfsでFAT32フォーマットする。
    {
        let mut file = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&img_path).unwrap();
        file.set_len(64 * 1024 * 1024).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        fatfs::format_volume(&mut file, fatfs::FormatVolumeOptions::new().fat_type(fatfs::FatType::Fat32)).unwrap();
        file.flush().unwrap();
    }
    println!("[1/4] FAT32フォーマット完了: {}", img_path.display());

    // 2. ForeignFatVolumeで開き、ファイル・サブディレクトリを書き込む。
    let volume = ForeignFatVolume::open(&img_path).expect("ボリュームを開けませんでした");
    volume.write_file("/hello.txt", b"hello from open-raid-z foreign_fs").expect("hello.txt書き込み失敗");
    println!("[2/4] /hello.txt を書き込みました");

    // 3. 読み戻して内容が一致するか確認する。
    let read_back = volume.read_file("/hello.txt").expect("hello.txt読み取り失敗");
    assert_eq!(read_back, b"hello from open-raid-z foreign_fs");
    println!("[3/4] /hello.txt を読み戻し、内容一致を確認しました");

    // 4. ルートディレクトリを一覧し、書き込んだファイルが見えるか確認する。
    let entries = volume.list_dir("/").expect("一覧取得失敗");
    let found = entries.iter().find(|e| e.name.eq_ignore_ascii_case("hello.txt"));
    match found {
        Some(e) => println!("[4/4] ディレクトリ一覧で確認: name={} size={} is_dir={}", e.name, e.size_bytes, e.is_dir),
        None => panic!("hello.txtがディレクトリ一覧に見つかりませんでした: {entries:?}"),
    }

    println!("\n全項目: 成功。");
    let _ = std::fs::remove_file(&img_path);
}
