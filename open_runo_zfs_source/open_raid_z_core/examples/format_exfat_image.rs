//! テスト用: 指定パスのファイルをexFATでフォーマットするだけの小さな
//! 開発用ツール(`orzctl foreign --format exfat`のCLI動作確認用)。
//! 使い方: cargo run --no-default-features --features foreign_fs --example format_exfat_image -- <PATH> <SIZE_MIB>

use hadris_fat::exfat::{format_exfat, ExFatFormatOptions};
use std::fs::OpenOptions;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: format_exfat_image <PATH> <SIZE_MIB>");
    let size_mib: u64 = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(32);
    let size = size_mib * 1024 * 1024;

    let file = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(path).unwrap();
    file.set_len(size).unwrap();

    let options = ExFatFormatOptions::default().with_label("ORZTEST");
    format_exfat(file, size, &options).unwrap();
    println!("formatted: {path} ({size_mib} MiB, exFAT)");
}
