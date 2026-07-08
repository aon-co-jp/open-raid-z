//! RAID-Z1パリティ計算。
//!
//! `compute_parity_cpu` は正しさの基準となるCPU参照実装。
//! `compute_parity_accelerated` はNPU/GPUが検出できれば
//! `shaders/raidz_parity.hlsl`(ビルド時にDXILへ事前コンパイル済み、
//! [`crate::compute`]参照)を実際にD3D12 Compute経由でディスパッチし、
//! ディスパッチに失敗した場合(ドライバ非対応・ハードウェア無し等)は
//! CPU実装へフォールバックする。

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

/// GPU/NPUディスパッチ。ディスパッチに失敗した場合はCPU実装へフォールバックする
/// (`device`自体はハードウェア選定時点の情報であり、ディスパッチ用のデバイスは
/// [`crate::compute::dispatch_parity_shader`]内部で選定し直す)。
pub fn compute_parity_accelerated(device: &AccelDevice, data_stripes: &[&[u32]]) -> Vec<u32> {
    match device.kind {
        crate::device::AccelKind::Npu | crate::device::AccelKind::Gpu => {
            let stripe_len = data_stripes.first().map(|s| s.len()).unwrap_or(0);
            let num_disks = data_stripes.len();
            let mut input = Vec::with_capacity(num_disks * stripe_len);
            for stripe in data_stripes {
                input.extend_from_slice(stripe);
            }

            let shader = include_bytes!(concat!(env!("OUT_DIR"), "/raidz_parity.cso"));
            match crate::compute::dispatch_parity_shader(
                shader,
                num_disks as u32,
                stripe_len as u32,
                &input,
                1,
            ) {
                Ok(mut outputs) => outputs.pop().unwrap_or_default(),
                Err(e) => {
                    tracing::warn!(
                        "GPU/NPUディスパッチに失敗したため、CPU実装にフォールバックします (device={}, error={e})",
                        device.adapter_description
                    );
                    compute_parity_cpu(data_stripes)
                }
            }
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
