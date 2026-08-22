//! RAID6(RAID-Z2/Z3)パリティ計算のCPU SIMDベンチマーク。
//!
//! 既存のGF乗算テーブル引き実装(`compute_pq_reference`/`compute_pqr_reference`)と、
//! 2026-08-22に追加したSIMD実装(`compute_pq`/`compute_pqr`、実行時CPU機能検出で
//! AVX-512BW → AVX2 → SSE2 → スカラーへディスパッチ)を同一入力で比較する。
//!
//! 実行:
//! ```text
//! cargo run --release --no-default-features --example simd_parity_benchmark
//! ```
//!
//! 【正直な開示】この開発機(AMD Ryzen 9 3950X / Zen 2)はAVX-512非搭載のため、
//! ここで実測できるのはAVX2経路まで。AVX-512経路はコンパイル確認のみ。

use std::time::Instant;
use zfs_accel_hlsl::galois::GaloisTables;
use zfs_accel_hlsl::raidz23_parity as pz;
use zfs_accel_hlsl::simd;

fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
    let mut s = seed | 1;
    (0..len)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 24) as u8
        })
        .collect()
}

fn main() {
    let gf = GaloisTables::new();
    println!("検出されたSIMD経路: {}", simd::simd_level().as_str());
    println!("(AVX-512搭載CPUではavx512、Zen2等ではavx2と表示される)\n");

    // NVMe SSDを4〜10枚使ったRAID6構成を想定(データディスク本数 = 総本数 - 2)。
    let chunk = 128 * 1024; // 128KiB/ディスクのストライプチャンク
    let iterations = 200;

    println!("チャンク={}KiB × {}ストライプ", chunk / 1024, iterations);
    println!("{:>10} | {:>12} | {:>12} | {:>8}", "データ本数", "テーブル(ms)", "SIMD(ms)", "倍率");
    println!("{}", "-".repeat(52));

    for num_disks in [2usize, 4, 6, 8] {
        let disks: Vec<Vec<u8>> = (0..num_disks).map(|i| pseudo_random(chunk, 0x1234 + i as u64)).collect();
        let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();

        // 正しさを先に確認(ベンチマークの前提)
        let (rp, rq) = pz::compute_pq_reference(&refs, &gf);
        let (sp, sq) = pz::compute_pq(&refs, &gf);
        assert_eq!((rp, rq), (sp, sq), "SIMD版とテーブル版の結果が一致しません");

        let t0 = Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(pz::compute_pq_reference(&refs, &gf));
        }
        let table_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let t1 = Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(pz::compute_pq(&refs, &gf));
        }
        let simd_ms = t1.elapsed().as_secs_f64() * 1000.0;

        println!(
            "{:>10} | {:>12.1} | {:>12.1} | {:>7.2}x",
            num_disks,
            table_ms,
            simd_ms,
            table_ms / simd_ms
        );
    }

    println!("\n[RAID-Z3(P/Q/R)]");
    println!("{:>10} | {:>12} | {:>12} | {:>8}", "データ本数", "テーブル(ms)", "SIMD(ms)", "倍率");
    println!("{}", "-".repeat(52));
    for num_disks in [4usize, 8] {
        let disks: Vec<Vec<u8>> = (0..num_disks).map(|i| pseudo_random(chunk, 0x99 + i as u64)).collect();
        let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();
        assert_eq!(pz::compute_pqr_reference(&refs, &gf), pz::compute_pqr(&refs, &gf));

        let t0 = Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(pz::compute_pqr_reference(&refs, &gf));
        }
        let table_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let t1 = Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(pz::compute_pqr(&refs, &gf));
        }
        let simd_ms = t1.elapsed().as_secs_f64() * 1000.0;
        println!(
            "{:>10} | {:>12.1} | {:>12.1} | {:>7.2}x",
            num_disks,
            table_ms,
            simd_ms,
            table_ms / simd_ms
        );
    }

    // 任意係数のGF乗算(復旧経路)
    println!("\n[任意係数GF(2^8)乗算 acc ^= src*c(復旧経路)]");
    let src = pseudo_random(chunk, 7);
    let mut acc = pseudo_random(chunk, 11);
    let t0 = Instant::now();
    for i in 0..iterations {
        let c = (i % 254 + 2) as u8;
        for b in 0..chunk {
            acc[b] ^= gf.mul(src[b], c);
        }
    }
    let table_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let mut acc2 = pseudo_random(chunk, 11);
    let t1 = Instant::now();
    for i in 0..iterations {
        let c = (i % 254 + 2) as u8;
        simd::gf_mul_xor_into(&mut acc2, &src, c, &gf);
    }
    let simd_ms = t1.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(acc, acc2, "GF乗算のSIMD版とテーブル版の結果が一致しません");
    println!("テーブル: {table_ms:.1}ms / SIMD: {simd_ms:.1}ms / 倍率: {:.2}x", table_ms / simd_ms);
}
