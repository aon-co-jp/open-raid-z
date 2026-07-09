//! テスト用: 指定パスのファイルをFAT32でフォーマットするだけの小さな
//! 開発用ツール(`orzctl foreign`のCLI動作確認用)。
//! 使い方: cargo run --no-default-features --features foreign_fs --example format_fat32_image -- <PATH> <SIZE_MIB>

use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: format_fat32_image <PATH> <SIZE_MIB>");
    let size_mib: u64 = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(64);

    let mut file = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(path).unwrap();
    file.set_len(size_mib * 1024 * 1024).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    fatfs::format_volume(&mut file, fatfs::FormatVolumeOptions::new().fat_type(fatfs::FatType::Fat32)).unwrap();
    file.flush().unwrap();
    println!("formatted: {path} ({size_mib} MiB, FAT32)");
}
