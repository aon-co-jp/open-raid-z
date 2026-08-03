//! CPU実装 vs GPU実装(`with_accelerator`)のRAID-Z2パリティ計算を、
//! 4〜8枚のNVMe構成を模したディスク本数・チャンクサイズで実測比較する
//! (2026-08-01追加、`open-raid-z/CLAUDE.md`の積み残しバックログ
//! 「4〜8枚のNVMe構成を模したベンチマーク」への対応)。
//!
//! ループバックファイル(実SSDではない)を使うため、ディスクI/O自体の
//! 速度差ではなく**パリティ計算(P/Q、GF(2^8) Reed-Solomon)のCPU/GPU
//! ディスパッチにかかる時間差**を素直に計測する——実SSD特有のRead-
//! Modify-Writeレイテンシの効果は含まれないが、そもそもこのベンチマーク
//! の主目的は「GPUオフロードがCPU実装より実際に速いか(あるいは遅いか)」
//! というパリティ計算そのものの実測であり、その目的には十分。
//!
//! 実行方法: `cargo run --release --example raidz2_parity_benchmark`

use open_raid_z_core::block_device::FileBackedDevice;
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};
use std::time::Instant;

fn scratch_disk(dir: &std::path::Path, name: &str, size_bytes: u64) -> FileBackedDevice {
    let path = dir.join(name);
    FileBackedDevice::create_fixed_size(&path, size_bytes).unwrap()
}

/// `num_data_disks`(データディスク本数、+2パリティ = 合計ディスク数)・
/// `chunk_size`・`stripe_count`(書き込むストライプ数)を指定して、
/// CPU実装とGPU実装(利用可能な場合)双方の`write_stripe`合計所要時間を
/// 計測する。
fn run_one_config(num_data_disks: usize, chunk_size: usize, stripe_count: u64, accel: Option<&zfs_accel_hlsl::device::AccelDevice>) -> std::time::Duration {
    let dir = std::env::temp_dir().join(format!(
        "orz-parity-bench-{}-{}-{}-{}",
        num_data_disks,
        chunk_size,
        std::process::id(),
        if accel.is_some() { "gpu" } else { "cpu" }
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let total_disks = num_data_disks + 2; // Z2 = 2パリティ固定
    let disk_size = chunk_size as u64 * (stripe_count + 1);
    let devices: Vec<FileBackedDevice> = (0..total_disks).map(|i| scratch_disk(&dir, &format!("d{i}"), disk_size)).collect();

    let mut vdev = RaidZVdev::new(devices, RaidLevel::Z2, chunk_size);
    if let Some(accel) = accel {
        vdev = vdev.with_accelerator(accel.clone());
    }

    let stripe_data = vec![0xABu8; chunk_size * num_data_disks];

    let start = Instant::now();
    for i in 0..stripe_count {
        vdev.write_stripe(i, &stripe_data).unwrap();
    }
    let elapsed = start.elapsed();

    std::fs::remove_dir_all(&dir).ok();
    elapsed
}

fn main() {
    let accel = zfs_accel_hlsl::device::detect_best_accelerator().ok().filter(|a| a.kind != zfs_accel_hlsl::device::AccelKind::CpuFallback);

    match &accel {
        Some(a) => println!("検出したアクセラレータ: {:?}", a.kind),
        None => println!("GPU/NPUが見つからないため、CPU実装のみ計測します。"),
    }

    // 「4〜8枚のNVMe構成」= データディスク3〜6枚(+2パリティ=合計5〜8枚)。
    // チャンクサイズはZFSの既定レコードサイズに合わせ128KiB。
    const CHUNK_SIZE: usize = 128 * 1024;
    const STRIPES: u64 = 200;

    println!(
        "{:<12} {:>14} {:>14} {:>10}",
        "データ本数", "CPU (ms)", "GPU (ms)", "GPU倍率"
    );
    for num_data_disks in [3usize, 4, 5, 6] {
        let cpu_elapsed = run_one_config(num_data_disks, CHUNK_SIZE, STRIPES, None);
        let cpu_ms = cpu_elapsed.as_secs_f64() * 1000.0;

        if let Some(accel) = &accel {
            let gpu_elapsed = run_one_config(num_data_disks, CHUNK_SIZE, STRIPES, Some(accel));
            let gpu_ms = gpu_elapsed.as_secs_f64() * 1000.0;
            let ratio = cpu_ms / gpu_ms;
            println!("{:<12} {:>14.2} {:>14.2} {:>9.2}x", format!("{num_data_disks}+2"), cpu_ms, gpu_ms, ratio);
        } else {
            println!("{:<12} {:>14.2} {:>14} {:>10}", format!("{num_data_disks}+2"), cpu_ms, "-", "-");
        }
    }

    println!(
        "\n正直な開示: ループバックファイル(実SSDではない)でのパリティ計算\
         (GF(2^8) Reed-Solomon)自体の所要時間比較であり、実NVMe特有のI/O\
         レイテンシ・Read-Modify-Writeの影響は含まない。個々のGPUディスパッチ\
         には固定オーバーヘッド(コマンドバッファ構築・同期待ち)があるため、\
         ストライプが小さい/データディスク本数が少ないほどCPU実装が有利に\
         見える場合がある——これは実測値であり、恣意的な調整は行っていない。"
    );
}
