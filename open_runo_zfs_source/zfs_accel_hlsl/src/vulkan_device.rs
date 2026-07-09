//! Windows以外(Linux/Mac/Android等)向けのNPU/GPU検出(Vulkan版)。
//!
//! [`crate::device`]のD3D12版`imp`モジュールと同じ選定ロジック
//! (NPU的な名前を優先、次にディスクリートGPU)をVulkanの
//! `PhysicalDeviceProperties`を使って行う。Vulkanドライバ/ローダー
//! (`libvulkan.so`/`vulkan-1.dll`)自体が存在しない環境でも、
//! (`vulkan` feature込みで)ビルドは常に成功する
//! (`ash`の`loaded`featureにより実行時に動的ロードするため)。
//! その場合は[`crate::device::DeviceError::NoD3D12Device`]を返し、
//! 呼び出し側([`crate::device::detect_best_accelerator`])がCPU
//! フォールバックへ切り替える。

use crate::device::{AccelDevice, AccelKind, DeviceError};

/// 参考: Intel AI Boost, AMD XDNA (Ryzen AI), Qualcomm Hexagon NPU
fn looks_like_npu(name: &str) -> bool {
    const NPU_MARKERS: &[&str] = &["AI Boost", "XDNA", "Hexagon", "NPU"];
    let lower = name.to_lowercase();
    NPU_MARKERS.iter().any(|marker| lower.contains(&marker.to_lowercase()))
}

/// Vulkan対応デバイスをNPU>ディスクリートGPU>統合GPUの優先順位で選定する。
pub fn detect_best_vulkan_device() -> Result<AccelDevice, DeviceError> {
    let entry = unsafe { ash::Entry::load() }.map_err(|_| DeviceError::NoD3D12Device)?;

    let app_info = ash::vk::ApplicationInfo::default().api_version(ash::vk::API_VERSION_1_1);
    let create_info = ash::vk::InstanceCreateInfo::default().application_info(&app_info);
    let instance = unsafe { entry.create_instance(&create_info, None) }.map_err(|_| DeviceError::NoD3D12Device)?;

    let result = (|| -> Result<AccelDevice, DeviceError> {
        let physical_devices =
            unsafe { instance.enumerate_physical_devices() }.map_err(|_| DeviceError::NoD3D12Device)?;

        let mut best_gpu: Option<AccelDevice> = None;

        for pd in physical_devices {
            let props = unsafe { instance.get_physical_device_properties(pd) };
            let name = {
                let raw = &props.device_name;
                let len = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
                let bytes: Vec<u8> = raw[..len].iter().map(|&c| c as u8).collect();
                String::from_utf8_lossy(&bytes).into_owned()
            };

            if looks_like_npu(&name) {
                return Ok(AccelDevice { kind: AccelKind::Npu, adapter_description: name });
            }

            if props.device_type == ash::vk::PhysicalDeviceType::DISCRETE_GPU {
                // ディスクリートGPUを見つけたら即決定(統合GPUより優先)
                return Ok(AccelDevice { kind: AccelKind::Gpu, adapter_description: name });
            }

            if best_gpu.is_none() && props.device_type == ash::vk::PhysicalDeviceType::INTEGRATED_GPU {
                best_gpu = Some(AccelDevice { kind: AccelKind::Gpu, adapter_description: name });
            }
        }

        best_gpu.ok_or(DeviceError::NoD3D12Device)
    })();

    unsafe { instance.destroy_instance(None) };
    result
}
