//! RAID-Z1パリティ計算。
//!
//! `compute_parity_cpu` は正しさの基準となるCPU参照実装。
//! `compute_parity_gpu` は同じ結果をGPU/NPU(DirectML経由のD3D12 Compute)で
//! 出す想定のスタブで、現段階ではCPU実装に委譲しています
//! (HLSLシェーダのコンパイル・ディスパッチ・リードバックのFFI配線が
//! 実機Windows環境でのテストを要するため)。

use crate::device::AccelDevice;

/// CPU参照実装: 複数ディスクのストライプに対する単純XORパリティ
pub fn compute_parity_cpu(data_stripes: &[&[u32]]) -> Vec<u32> {
    let stripe_len = data_stripes.first().map(|s| s.len()).unwrap_or(0);
    let mut parity = vec![0u32; stripe_len];

    for stripe in data_stripes {
        debug_assert_eq!(stripe.len(), stripe_len, "全ストライプは同じ長さである必要があります");
        for (p, &d) in parity.iter_mut().zip(stripe.iter()) {
            *p ^= d;
        }
    }

    parity
}

/// GPU/NPUディスパッチ(現状はCPUへフォールバック)
///
/// TODO: raidz_parity.hlsl をコンパイルし、`device` が指すD3D12/DirectML
/// デバイス上でディスパッチする実装に置き換える。
pub fn compute_parity_accelerated(device: &AccelDevice, data_stripes: &[&[u32]]) -> Vec<u32> {
    match device.kind {
        crate::device::AccelKind::Npu | crate::device::AccelKind::Gpu => {
            tracing::warn!(
                "GPU/NPUディスパッチは未実装のため、CPU実装にフォールバックします (device={})",
                device.adapter_description
            );
            compute_parity_cpu(data_stripes)
        }
        crate::device::AccelKind::CpuFallback => compute_parity_cpu(data_stripes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xor_parity_matches_manual_calculation() {
        let d0: Vec<u32> = vec![0b1010, 0b0011];
        let d1: Vec<u32> = vec![0b0110, 0b1001];
        let parity = compute_parity_cpu(&[&d0, &d1]);
        assert_eq!(parity, vec![0b1100, 0b1010]);
    }
}
