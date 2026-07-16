//! `ForeignExfatVolume`(exFAT読み書き)の実動作確認用スモークテスト。
//!
//! `exfat-fs`(0.1系、読み取り専用)から`hadris-fat`(`write`+`exfat`
//! feature)へ移行したことで、FAT32版(`foreign_fs_smoke_test.rs`)と
//! 同じ「ファイルを書き込んで読み戻す」往復検証ができるようになった。
//!
//! 実行方法:
//!   cargo run --no-default-features --features foreign_fs --example exfat_smoke_test

use hadris_fat::exfat::{format_exfat, ExFatFormatOptions};
use open_raid_z_core::foreign_fs::ForeignExfatVolume;
use std::fs::OpenOptions;

fn main() {
    let img_path = std::env::temp_dir().join("orz_exfat_smoke.img");

    // 1. 32MiBの空exFATボリュームをフォーマットする。
    let size: u64 = 32 * 1024 * 1024;
    let file =
        OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&img_path).expect("イメージ作成に失敗");
    file.set_len(size).expect("サイズ設定に失敗");
    let options = ExFatFormatOptions::default().with_label("ORZTEST");
    format_exfat(file, size, &options).expect("フォーマットに失敗");
    println!("[1/4] exFATフォーマット完了: {}", img_path.display());

    // 2. ForeignExfatVolumeで開けることを確認する。
    let volume = ForeignExfatVolume::open(&img_path).expect("exFATボリュームを開けませんでした");
    println!("[2/4] ForeignExfatVolume::openに成功");

    // 3. ルート直下が空であることを確認する(フォーマット直後はファイルが無いため)。
    let entries = volume.list_dir("/").expect("ルートディレクトリの一覧取得に失敗");
    assert!(entries.is_empty(), "フォーマット直後のルートは空であるはずですが、{entries:?} が見つかりました");
    println!("[3/4] ルートディレクトリが空であることを確認");

    // 4. ファイルを書き込んで読み戻す(往復検証)。
    let payload = b"hello, exFAT write support via hadris-fat\n";
    volume.write_file("hello.txt", payload).expect("書き込みに失敗");
    let read_back = volume.read_file("hello.txt").expect("読み戻しに失敗");
    assert_eq!(read_back, payload, "書き込んだ内容と読み戻した内容が一致しません");
    let entries = volume.list_dir("/").expect("ルートディレクトリの一覧取得に失敗");
    assert_eq!(entries.len(), 1, "ルートには1件だけあるはずです");
    assert_eq!(entries[0].name, "hello.txt");
    assert_eq!(entries[0].size_bytes, payload.len() as u64);
    println!("[4/4] ファイルの書き込み・読み戻し・一覧確認に成功");

    println!("\n全項目: 成功(exFAT読み書きパスの実証)。");
    let _ = std::fs::remove_file(&img_path);
}
