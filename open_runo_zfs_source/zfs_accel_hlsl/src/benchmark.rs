//! NPU/GPU/CPUの実性能を計測するベンチマーク機能。
//!
//! 【目的】PC・タブレット・スマートフォンなど搭載デバイスによってNPU/GPUの
//! 実効性能は大きく異なる(統合GPUと専用GPU、世代の新旧、NPUの有無等)。
//! 単に「検出できたから使う」のではなく、**実際にこのマシン上でCPUより
//! 速いのかどうかを計測**した上で、より速い方を選べるようにする。
//!
//! また、1台単体だけでなく、複数台(RAID構成)にまたがってNPU/GPUを使う
//! 運用を想定し、各ノードの性能を個別に計測して比較できるようにしておく
//! (実際のノード間分散スケジューリングは将来の拡張範囲。まずは
//! 「各ノード上でNPU/GPU/CPUのどれが実際に速いか」を数値で示す)。
//!
//! 優先度確保(他プロセスより優先してNPU/GPUパワーを使う)については、
//! [`crate::compute`]のD3D12コマンドキュー生成(`D3D12_COMMAND_QUEUE_
//! PRIORITY_HIGH`)、[`crate::vulkan_compute`]のVulkanキュー優先度
//! (1.0=最大)を参照。OSのGPUスケジューラが実際にどこまで尊重するかは
//! ドライバ依存だが、アプリケーション単位で安全に指定できる範囲では
//! 常に最大優先度を要求している。

use crate::device::AccelDevice;
use std::time::{Duration, Instant};

/// 1回のベンチマーク実行結果。
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// 計測対象(例: "CPU"、"GPU: NVIDIA GeForce GT 730"等)。
    pub label: String,
    /// 処理したデータ総量(バイト、全ディスク合計・全繰り返し合計ではなく
    /// 1回あたり)。
    pub data_size_bytes: usize,
    /// 実行に要した時間(繰り返し回数の平均)。
    pub elapsed: Duration,
    /// スループット(MB/s)。`data_size_bytes / elapsed`から算出。
    pub throughput_mb_per_sec: f64,
}

impl BenchmarkResult {
    fn new(label: String, data_size_bytes: usize, elapsed: Duration) -> Self {
        let mb = data_size_bytes as f64 / (1024.0 * 1024.0);
        let secs = elapsed.as_secs_f64().max(f64::EPSILON);
        Self { label, data_size_bytes, elapsed, throughput_mb_per_sec: mb / secs }
    }
}

/// ベンチマークに使う疑似データを生成する(毎回同じ内容で再現性を持たせる。
/// 実際のRAID-Zパリティ計算はデータ内容に依存しない一定コストのため、
/// 疑似乱数で十分)。
fn make_test_stripes(num_disks: usize, stripe_len_words: usize) -> Vec<Vec<u32>> {
    (0..num_disks)
        .map(|disk_idx| {
            (0..stripe_len_words)
                .map(|i| (disk_idx as u32).wrapping_mul(2654435761).wrapping_add(i as u32))
                .collect()
        })
        .collect()
}

/// RAID-Z1(XOR)パリティ計算のスループットを計測する。
/// `iterations`回計算を繰り返し、合計時間から平均スループットを算出する。
pub fn benchmark_xor_parity_cpu(num_disks: usize, stripe_len_words: usize, iterations: u32) -> BenchmarkResult {
    let stripes = make_test_stripes(num_disks, stripe_len_words);
    let refs: Vec<&[u32]> = stripes.iter().map(|s| s.as_slice()).collect();
    let data_size_bytes = num_disks * stripe_len_words * 4;

    let start = Instant::now();
    for _ in 0..iterations {
        std::hint::black_box(crate::raidz_parity::compute_parity_cpu(&refs));
    }
    let elapsed = start.elapsed() / iterations.max(1);

    BenchmarkResult::new("CPU".to_string(), data_size_bytes, elapsed)
}

/// RAID-Z1(XOR)パリティ計算のスループットを、検出済みのNPU/GPUで計測する。
/// ディスパッチに失敗した場合(ハードウェア無し等)は`None`を返す
/// (`compute_parity_accelerated`はCPUへ自動フォールバックしてしまうため、
/// ここでは「本当にアクセラレータが使われたか」を明示的に判定する必要が
/// あり、`device.kind`が`CpuFallback`の場合は最初から計測しない)。
pub fn benchmark_xor_parity_accelerated(
    device: &AccelDevice,
    num_disks: usize,
    stripe_len_words: usize,
    iterations: u32,
) -> Option<BenchmarkResult> {
    if device.kind == crate::device::AccelKind::CpuFallback {
        return None;
    }
    let stripes = make_test_stripes(num_disks, stripe_len_words);
    let refs: Vec<&[u32]> = stripes.iter().map(|s| s.as_slice()).collect();
    let data_size_bytes = num_disks * stripe_len_words * 4;

    let start = Instant::now();
    for _ in 0..iterations {
        std::hint::black_box(crate::raidz_parity::compute_parity_accelerated(device, &refs));
    }
    let elapsed = start.elapsed() / iterations.max(1);

    let label = format!("{:?}: {}", device.kind, device.adapter_description);
    Some(BenchmarkResult::new(label, data_size_bytes, elapsed))
}

/// このマシンで検出できる全アクセラレータ(NPU/GPU)+CPUの性能を、
/// それぞれ同一条件で計測して並べる。速い順に並べ替えはせず、検出順
/// (CPU→検出された各デバイス)のまま返す(呼び出し側で用途に応じて
/// ソート・比較すればよい)。
///
/// 既定では4MB(1MB×4ディスク相当)を10回計算する設定にしている
/// (実機での計測時間が数十ms〜数百ms程度に収まる、実用的なデフォルト)。
pub fn benchmark_all_available() -> Vec<BenchmarkResult> {
    const NUM_DISKS: usize = 4;
    const STRIPE_LEN_WORDS: usize = 256 * 1024; // 1MiB相当(4バイト単位)
    const ITERATIONS: u32 = 10;

    let mut results = vec![benchmark_xor_parity_cpu(NUM_DISKS, STRIPE_LEN_WORDS, ITERATIONS)];

    for device in crate::device::list_all_accelerators() {
        if let Some(r) =
            benchmark_xor_parity_accelerated(&device, NUM_DISKS, STRIPE_LEN_WORDS, ITERATIONS)
        {
            results.push(r);
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_benchmark_reports_a_positive_throughput() {
        let result = benchmark_xor_parity_cpu(4, 4096, 5);
        assert!(result.throughput_mb_per_sec > 0.0);
        assert_eq!(result.label, "CPU");
    }

    #[test]
    fn benchmark_all_available_always_includes_cpu_and_completes_without_panicking() {
        let results = benchmark_all_available();
        assert!(!results.is_empty(), "CPUの結果は必ず含まれるはず");
        assert_eq!(results[0].label, "CPU");
        for r in &results {
            println!("{}: {:.1} MB/s", r.label, r.throughput_mb_per_sec);
            assert!(r.throughput_mb_per_sec > 0.0);
        }
    }
}
