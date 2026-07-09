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
        crate::device::AccelKind::Gpu | crate::device::AccelKind::Npu => {
            #[cfg(feature = "gpu")]
            {
                let shader = if device.kind == crate::device::AccelKind::Npu {
                    // NPU側は現状GPU版と同一アルゴリズムだが、シェーダバイトコードは
                    // `raidnpu_parity.hlsl`由来の別バイナリを使う(経緯は同ファイルの
                    // 先頭コメント参照)。
                    include_bytes!(concat!(env!("OUT_DIR"), "/raidnpu_parity.cso")).as_slice()
                } else {
                    include_bytes!(concat!(env!("OUT_DIR"), "/raidz_parity.cso")).as_slice()
                };
                return dispatch_or_fallback(device, data_stripes, shader);
            }
            // Windows以外(Linux/Mac/Android等)向け: `vulkan` featureが有効なら
            // Vulkan Compute経由でディスパッチする(`gpu`はWindows専用APIの
            // ため、`gpu`が無効なビルドではこちらが実運用経路になる)。
            #[cfg(all(feature = "vulkan", not(feature = "gpu")))]
            {
                return dispatch_or_fallback_vulkan(device, data_stripes);
            }
            #[cfg(not(any(feature = "gpu", feature = "vulkan")))]
            {
                compute_parity_cpu(data_stripes)
            }
        }
        crate::device::AccelKind::CpuFallback => compute_parity_cpu(data_stripes),
    }
}

#[cfg(feature = "gpu")]
fn dispatch_or_fallback(device: &AccelDevice, data_stripes: &[&[u32]], shader: &[u8]) -> Vec<u32> {
    let stripe_len = data_stripes.first().map(|s| s.len()).unwrap_or(0);
    let num_disks = data_stripes.len();
    let mut input = Vec::with_capacity(num_disks * stripe_len);
    for stripe in data_stripes {
        input.extend_from_slice(stripe);
    }

    match crate::compute::dispatch_parity_shader(shader, num_disks as u32, stripe_len as u32, &input, 1) {
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

/// Windows以外(Linux/Mac/Android等)向け: Vulkan Compute経由のディスパッチ
/// (`raidz_parity.comp`由来のSPIR-V。NPU/GPUどちらも現状同一シェーダを使う)。
#[cfg(all(feature = "vulkan", not(feature = "gpu")))]
fn dispatch_or_fallback_vulkan(device: &AccelDevice, data_stripes: &[&[u32]]) -> Vec<u32> {
    let stripe_len = data_stripes.first().map(|s| s.len()).unwrap_or(0);
    let num_disks = data_stripes.len();
    let mut input = Vec::with_capacity(num_disks * stripe_len);
    for stripe in data_stripes {
        input.extend_from_slice(stripe);
    }

    let shader = include_bytes!(concat!(env!("OUT_DIR"), "/raidz_parity.spv"));
    match crate::vulkan_compute::dispatch_parity_shader_vulkan(
        shader,
        num_disks as u32,
        stripe_len as u32,
        &input,
        1,
    ) {
        Ok(mut outputs) => outputs.pop().unwrap_or_default(),
        Err(e) => {
            tracing::warn!(
                "Vulkanディスパッチに失敗したため、CPU実装にフォールバックします (device={}, error={e})",
                device.adapter_description
            );
            compute_parity_cpu(data_stripes)
        }
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

    #[test]
    fn compute_parity_accelerated_matches_cpu_when_hardware_available() {
        let device = match crate::device::detect_best_accelerator() {
            Ok(d) => d,
            Err(_) => {
                eprintln!("D3D12対応アクセラレータが見つからないためテストをスキップします");
                return;
            }
        };
        if device.kind == crate::device::AccelKind::CpuFallback {
            eprintln!("GPU/NPUが見つからないためテストをスキップします");
            return;
        }

        let d0: Vec<u32> = vec![0x0102_0304, 0x1122_3344];
        let d1: Vec<u32> = vec![0xAABB_CCDD, 0x5566_7788];
        let refs: Vec<&[u32]> = vec![&d0, &d1];

        let expected = compute_parity_cpu(&refs);
        let actual = compute_parity_accelerated(&device, &refs);

        assert_eq!(actual, expected);
    }

    // `raidnpu_parity.hlsl`(NPU専用ディスパッチ経路用のシェーダバイトコード)
    // 自体の正しさを、NPU実機が無くてもこのマシンで検出できるD3D12デバイスを
    // 使って検証する(経緯は`compute_parity_accelerated`内のコメント参照)。
    #[cfg(feature = "gpu")]
    #[test]
    fn raidnpu_shader_matches_cpu_when_any_d3d12_device_available() {
        let device = match crate::device::detect_best_accelerator() {
            Ok(d) => d,
            Err(_) => {
                eprintln!("D3D12対応デバイスが見つからないためテストをスキップします");
                return;
            }
        };
        if device.kind == crate::device::AccelKind::CpuFallback {
            eprintln!("D3D12対応デバイスが見つからないためテストをスキップします");
            return;
        }

        let _ = &device; // ハードウェア有無の判定のみに使う(ディスパッチ先は下記で直接指定)。
        let d0: Vec<u32> = vec![0x0102_0304, 0x1122_3344];
        let d1: Vec<u32> = vec![0xAABB_CCDD, 0x5566_7788];
        let refs: Vec<&[u32]> = vec![&d0, &d1];
        let stripe_len = d0.len();

        let expected = compute_parity_cpu(&refs);

        let mut input = Vec::with_capacity(refs.len() * stripe_len);
        for stripe in &refs {
            input.extend_from_slice(stripe);
        }
        let shader = include_bytes!(concat!(env!("OUT_DIR"), "/raidnpu_parity.cso"));
        // `dispatch_or_fallback`とは違い、失敗時にCPU実装へ黒く沈み込まず
        // `expect`で即座に落とす(このテストの目的はシェーダ自体の正しさの
        // 検証であり、失敗を握り消して見かけ上パスさせては意味がない)。
        let mut outputs = crate::compute::dispatch_parity_shader(
            shader,
            refs.len() as u32,
            stripe_len as u32,
            &input,
            1,
        )
        .expect("raidnpu_parity dispatch failed");
        let actual = outputs.pop().unwrap_or_default();

        assert_eq!(actual, expected);
    }
}
