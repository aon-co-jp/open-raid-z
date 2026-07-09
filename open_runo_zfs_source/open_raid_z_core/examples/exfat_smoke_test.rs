//! `ForeignExfatVolume`(exFAT読み取り)の実動作確認用スモークテスト。
//!
//! 【重要な制約】上流クレート`exfat-fs`(0.1系)は現時点でファイルの
//! **書き込みに対応していない**(ボリュームのフォーマットのみ対応)。
//! そのため、このテストは「ファイルを書き込んで読み戻す」という
//! `foreign_fs_smoke_test.rs`(FAT32版)と同じ往復検証はできない。
//! 代わりに、実際にexFAT仕様準拠のボリューム(ブートセクタ・FAT・
//! アロケーションビットマップ・アップケーステーブルを含む、正規の
//! 空exFATボリューム)をフォーマットし、`ForeignExfatVolume`がそれを
//! 正しく開けること・ルート直下が空であることを確認する
//! (構造的な読み取りパスの正しさの検証)。
//!
//! 実行方法:
//!   cargo run --no-default-features --features foreign_fs --example exfat_smoke_test

use exfat_fs::format::{Exfat, FormatVolumeOptionsBuilder};
use exfat_fs::{Label, MB};
use open_raid_z_core::foreign_fs::ForeignExfatVolume;
use std::io::Cursor;
use std::time::SystemTime;

fn main() {
    let img_path = std::env::temp_dir().join("orz_exfat_smoke.img");

    // 1. 32MiBの空exFATボリュームをメモリ上でフォーマットし、ファイルへ保存する。
    let size: u64 = 32 * MB as u64;
    let label = Label::new("ORZTEST".to_string()).expect("ラベルの作成に失敗");
    let format_options = FormatVolumeOptionsBuilder::default()
        .pack_bitmap(false)
        .full_format(false)
        .dev_size(size)
        .label(label)
        .bytes_per_sector(512)
        .build()
        .expect("フォーマットオプションの構築に失敗");
    let mut formatter = Exfat::try_from::<SystemTime>(format_options).expect("フォーマッタの初期化に失敗");
    let mut buffer = Cursor::new(vec![0u8; size as usize]);
    formatter.write::<SystemTime, Cursor<Vec<u8>>>(&mut buffer).expect("フォーマット書き込みに失敗");
    std::fs::write(&img_path, buffer.into_inner()).expect("イメージファイルの保存に失敗");
    println!("[1/3] exFATフォーマット完了: {}", img_path.display());

    // 2. ForeignExfatVolumeで開けることを確認する。
    let volume = ForeignExfatVolume::open(&img_path).expect("exFATボリュームを開けませんでした");
    println!("[2/3] ForeignExfatVolume::openに成功");

    // 3. ルート直下が空であることを確認する(フォーマット直後はファイルが無いため)。
    let entries = volume.list_dir("/").expect("ルートディレクトリの一覧取得に失敗");
    assert!(entries.is_empty(), "フォーマット直後のルートは空であるはずですが、{entries:?} が見つかりました");
    println!("[3/3] ルートディレクトリが空であることを確認");

    println!("\n全項目: 成功(exFAT読み取りパスの構造的検証)。");
    let _ = std::fs::remove_file(&img_path);
}
