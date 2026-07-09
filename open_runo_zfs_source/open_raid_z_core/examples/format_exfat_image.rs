//! テスト用: 指定パスのファイルをexFATでフォーマットするだけの小さな
//! 開発用ツール(`orzctl foreign --format exfat`のCLI動作確認用)。
//! 使い方: cargo run --no-default-features --features foreign_fs --example format_exfat_image -- <PATH> <SIZE_MIB>

use exfat_fs::format::{Exfat, FormatVolumeOptionsBuilder};
use exfat_fs::{Label, MB};
use std::io::Cursor;
use std::time::SystemTime;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: format_exfat_image <PATH> <SIZE_MIB>");
    let size_mib: u64 = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(32);
    let size = size_mib * MB as u64;

    let label = Label::new("ORZTEST".to_string()).unwrap();
    let format_options = FormatVolumeOptionsBuilder::default()
        .pack_bitmap(false)
        .full_format(false)
        .dev_size(size)
        .label(label)
        .bytes_per_sector(512)
        .build()
        .unwrap();
    let mut formatter = Exfat::try_from::<SystemTime>(format_options).unwrap();
    let mut buffer = Cursor::new(vec![0u8; size as usize]);
    formatter.write::<SystemTime, Cursor<Vec<u8>>>(&mut buffer).unwrap();
    std::fs::write(path, buffer.into_inner()).unwrap();
    println!("formatted: {path} ({size_mib} MiB, exFAT)");
}
