//! NPU / GPU の有無を検出し、利用可能なアクセラレータを自動選択する。
//!
//! 優先順位: NPU (存在すれば) > GPU (存在すれば) > CPUフォールバック
//!
//! DirectMLはD3D12デバイスの上に構築されるため、NPU/GPUのどちらであっても
//! 同一のDirectML Deviceインターフェースからディスパッチできる点が
//! このアーキテクチャの利点です(コード分岐が最小限で済む)。
//!
//! `gpu` feature が無効な場合(dxc/Windows SDKが無い環境向けのCPU専用
//! ビルド)は、D3D12/DXGIを一切呼び出さず常に[`AccelKind::CpuFallback`]を
//! 返す軽量な実装に切り替わる。型([`AccelDevice`]など)はfeatureに関わらず
//! 常に公開されるため、呼び出し側([`crate::raidz_parity`]等)はfeatureの
//! 有無を意識せずビルドできる。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("DirectX 12対応デバイスが見つかりません")]
    NoD3D12Device,
    #[error("DirectMLデバイス作成に失敗しました: {0}")]
    DmlCreationFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelKind {
    Npu,
    Gpu,
    CpuFallback,
}

#[derive(Debug, Clone)]
pub struct AccelDevice {
    pub kind: AccelKind,
    pub adapter_description: String,
}

/// アダプタ名からGPUベンダーを判定する(Intel/AMD/NVIDIA/Qualcomm。
/// いずれにも一致しなければ`Unknown`)。インストーラーUIの「今のGPU
/// (INTEL、AMD、nVIDIA対応状況)」表示のためのベンダー分類。
pub fn classify_vendor(description: &str) -> &'static str {
    let lower = description.to_lowercase();
    if lower.contains("nvidia") || lower.contains("geforce") || lower.contains("quadro") || lower.contains("rtx") {
        "NVIDIA"
    } else if lower.contains("amd") || lower.contains("radeon") {
        "AMD"
    } else if lower.contains("intel") {
        "Intel"
    } else if lower.contains("qualcomm") || lower.contains("adreno") || lower.contains("hexagon") {
        "Qualcomm"
    } else {
        "Unknown"
    }
}

/// システム上の**全ての**NPU/GPUアダプタを列挙する(ベストな1台だけを
/// 選ぶ[`detect_best_accelerator`]とは異なり、インストーラーUIの
/// 「今のGPU(複数なら複数)」表示のために全件返す)。
/// ソフトウェアアダプタ(WARP等)は除外する。見つからない場合は空配列。
pub fn list_all_accelerators() -> Vec<AccelDevice> {
    #[cfg(feature = "gpu")]
    {
        let list = imp::list_all_devices();
        if !list.is_empty() {
            return list;
        }
    }
    #[cfg(feature = "vulkan")]
    {
        if let Ok(accel) = crate::vulkan_device::detect_best_vulkan_device() {
            return vec![accel];
        }
    }
    Vec::new()
}

/// システム上のアダプタを列挙し、NPU/GPUの優先順位で選択する。
/// どちらも見つからない場合はCPUフォールバックを返す(安全側のデフォルト)。
///
/// 【優先順位】`gpu`(D3D12/DirectML、Windows専用)が有効かつ実際にデバイスが
/// 見つかればそれを使う。見つからない場合(またはそもそも`gpu`が無効な
/// 非Windowsビルド)は`vulkan`(Vulkan Compute、Windows以外向け)を試す。
/// どちらも見つからなければCPUフォールバック。
pub fn detect_best_accelerator() -> Result<AccelDevice, DeviceError> {
    #[cfg(feature = "gpu")]
    {
        match imp::create_best_device() {
            Ok((accel, _device)) => return Ok(accel),
            Err(DeviceError::NoD3D12Device) => {} // vulkan/CPUへフォールスルー
            Err(e) => return Err(e),
        }
    }

    #[cfg(feature = "vulkan")]
    {
        if let Ok(accel) = crate::vulkan_device::detect_best_vulkan_device() {
            return Ok(accel);
        }
    }

    Ok(AccelDevice {
        kind: AccelKind::CpuFallback,
        adapter_description: "CPU (NPU/GPU adapter not found or unavailable)".to_string(),
    })
}

#[cfg(feature = "gpu")]
pub(crate) use imp::create_best_device;

#[cfg(feature = "gpu")]
mod imp {
    use super::{AccelDevice, AccelKind, DeviceError};
    use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
    use windows::Win32::Graphics::Direct3D12::{D3D12CreateDevice, ID3D12Device};
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE,
    };

    /// アダプタ名(DXGI_ADAPTER_DESC1.Description)にNPU的な識別子が
    /// 含まれるかを判定する。ベンダー依存のため既知の文字列との
    /// 部分一致で近似判定する。
    ///
    /// 参考: Intel AI Boost, AMD XDNA (Ryzen AI), Qualcomm Hexagon NPU
    fn looks_like_npu(description: &str) -> bool {
        const NPU_MARKERS: &[&str] = &["AI Boost", "XDNA", "Hexagon", "NPU"];
        let lower = description.to_lowercase();
        NPU_MARKERS
            .iter()
            .any(|marker| lower.contains(&marker.to_lowercase()))
    }

    fn adapter_description(adapter: &IDXGIAdapter1) -> windows::core::Result<(String, u32)> {
        let desc = unsafe { adapter.GetDesc1()? };
        let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
        let description = String::from_utf16_lossy(&desc.Description[..len]);
        Ok((description, desc.Flags))
    }

    /// 指定アダプタ上にD3D12デバイスを実際に作成し、成功すれば返す。
    fn try_create_d3d12_device(adapter: &IDXGIAdapter1) -> Option<ID3D12Device> {
        let mut device: Option<ID3D12Device> = None;
        unsafe { D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }.ok()?;
        device
    }

    /// [`super::detect_best_accelerator`]と同じ選定ロジックだが、実際に作成した
    /// `ID3D12Device`もあわせて返す。GPU/NPUへディスパッチする側([`crate::compute`])が
    /// 選定と同じデバイスをそのまま使い回せるようにするためのもの。
    pub fn create_best_device() -> Result<(AccelDevice, ID3D12Device), DeviceError> {
        let factory: IDXGIFactory1 =
            unsafe { CreateDXGIFactory1() }.map_err(|_| DeviceError::NoD3D12Device)?;

        let mut best_gpu: Option<(AccelDevice, ID3D12Device)> = None;

        let mut index = 0u32;
        loop {
            let adapter = match unsafe { factory.EnumAdapters1(index) } {
                Ok(adapter) => adapter,
                Err(_) => break, // DXGI_ERROR_NOT_FOUND: 列挙終了
            };
            index += 1;

            let (description, flags) = match adapter_description(&adapter) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // ソフトウェアアダプタ(WARPなど)は物理NPU/GPUではないため除外
            if flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }

            let Some(device) = try_create_d3d12_device(&adapter) else {
                continue;
            };

            if looks_like_npu(&description) {
                // NPUは最優先なので見つかり次第確定して返す
                return Ok((
                    AccelDevice {
                        kind: AccelKind::Npu,
                        adapter_description: description,
                    },
                    device,
                ));
            }

            if best_gpu.is_none() {
                best_gpu = Some((
                    AccelDevice {
                        kind: AccelKind::Gpu,
                        adapter_description: description,
                    },
                    device,
                ));
            }
        }

        if let Some(gpu) = best_gpu {
            return Ok(gpu);
        }

        Err(DeviceError::NoD3D12Device)
    }

    /// [`super::list_all_accelerators`]向け: ソフトウェアアダプタを除いた
    /// 全物理アダプタを、NPU的な名前ならNpu、それ以外はGpuとして列挙する
    /// (`create_best_device`と異なり、最初の1台で確定せず全件走査する)。
    pub fn list_all_devices() -> Vec<AccelDevice> {
        let Ok(factory) = (unsafe { CreateDXGIFactory1() }) else {
            return Vec::new();
        };
        let factory: IDXGIFactory1 = factory;

        let mut devices = Vec::new();
        let mut index = 0u32;
        loop {
            let adapter = match unsafe { factory.EnumAdapters1(index) } {
                Ok(adapter) => adapter,
                Err(_) => break,
            };
            index += 1;

            let (description, flags) = match adapter_description(&adapter) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }
            if try_create_d3d12_device(&adapter).is_none() {
                continue;
            }

            let kind = if looks_like_npu(&description) { AccelKind::Npu } else { AccelKind::Gpu };
            devices.push(AccelDevice { kind, adapter_description: description });
        }
        devices
    }
}

#[cfg(not(feature = "gpu"))]
mod imp {
    use super::{AccelDevice, DeviceError};

    /// CPU専用ビルド(`gpu` feature無効)ではD3D12を一切呼び出さず、
    /// 常に「デバイス無し」を返す。呼び出し側の[`super::detect_best_accelerator`]が
    /// これを[`super::AccelKind::CpuFallback`]へ変換する。
    pub fn create_best_device() -> Result<(AccelDevice, ()), DeviceError> {
        Err(DeviceError::NoD3D12Device)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_all_accelerators_finds_at_least_the_best_device_on_this_machine() {
        let all = list_all_accelerators();
        println!("detected accelerators: {all:?}");
        if let Ok(best) = detect_best_accelerator() {
            if best.kind != AccelKind::CpuFallback {
                assert!(!all.is_empty(), "detect_best_accelerator found a device but list_all_accelerators found none");
            }
        }
    }

    #[test]
    fn classify_vendor_recognizes_known_vendor_strings() {
        assert_eq!(classify_vendor("NVIDIA GeForce GT 730"), "NVIDIA");
        assert_eq!(classify_vendor("AMD Radeon RX 6600"), "AMD");
        assert_eq!(classify_vendor("Intel(R) UHD Graphics 630"), "Intel");
        assert_eq!(classify_vendor("Qualcomm(R) Adreno(TM) 690"), "Qualcomm");
        assert_eq!(classify_vendor("Totally Unknown Adapter"), "Unknown");
    }
}
