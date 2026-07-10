//! RAID-Zパリティ計算のVulkan Computeディスパッチ層。
//!
//! [`crate::compute`](D3D12/DirectML版)と同じ役割・同じ呼び出し
//! シグネチャを、Windows以外(Linux/Mac/Android等)でも動くVulkan APIで
//! 提供する。シンプルさを優先し、D3D12版のような専用アップロード/
//! リードバックヒープの分離は行わず、ホスト可視(HOST_VISIBLE |
//! HOST_COHERENT)なメモリへ直接書き込み・読み出しする(正しさを優先した
//! 初回実装。デバイスローカルメモリへのステージングは将来の最適化余地)。

use ash::vk;

#[derive(Debug, thiserror::Error)]
pub enum VulkanComputeError {
    #[error("Vulkanローダー/インスタンスの初期化に失敗しました")]
    NoInstance,
    #[error("Vulkan対応デバイスが見つかりません")]
    NoDevice,
    #[error("Vulkan API呼び出しに失敗しました: {0:?}")]
    Vk(vk::Result),
}

impl From<vk::Result> for VulkanComputeError {
    fn from(e: vk::Result) -> Self {
        VulkanComputeError::Vk(e)
    }
}

pub type VulkanComputeResult<T> = Result<T, VulkanComputeError>;

/// 1ワード=4バイト(u32)としてバイト列を読み書きするためのヘルパー。
/// `crate::compute`(D3D12版、`gpu` feature専用)にある同名関数と全く同じ
/// 実装だが、`vulkan` featureのみ有効な(=`gpu`が無効な)ビルドでも使える
/// よう、こちらは`gpu`に依存せず定義する。
pub(crate) fn bytes_to_words(bytes: &[u8]) -> Vec<u32> {
    assert_eq!(bytes.len() % 4, 0, "データ長は4バイトの倍数である必要があります");
    bytes.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

pub(crate) fn words_to_bytes(words: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(words.len() * 4);
    for w in words {
        out.extend_from_slice(&w.to_le_bytes());
    }
    out
}

fn find_memory_type(
    mem_props: &vk::PhysicalDeviceMemoryProperties,
    type_bits: u32,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    (0..mem_props.memory_type_count).find(|&i| {
        (type_bits & (1 << i)) != 0 && mem_props.memory_types[i as usize].property_flags.contains(flags)
    })
}

/// ホスト可視なストレージバッファを作成し、確保したメモリへバインドする。
fn create_host_visible_storage_buffer(
    device: &ash::Device,
    mem_props: &vk::PhysicalDeviceMemoryProperties,
    size: u64,
) -> VulkanComputeResult<(vk::Buffer, vk::DeviceMemory)> {
    let buffer_info = vk::BufferCreateInfo::default()
        .size(size.max(4))
        .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer = unsafe { device.create_buffer(&buffer_info, None) }?;
    let mem_req = unsafe { device.get_buffer_memory_requirements(buffer) };
    let mem_type_index = find_memory_type(
        mem_props,
        mem_req.memory_type_bits,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )
    .ok_or(VulkanComputeError::NoDevice)?;
    let alloc_info =
        vk::MemoryAllocateInfo::default().allocation_size(mem_req.size).memory_type_index(mem_type_index);
    let memory = unsafe { device.allocate_memory(&alloc_info, None) }?;
    unsafe { device.bind_buffer_memory(buffer, memory, 0) }?;
    Ok((buffer, memory))
}

fn write_words(device: &ash::Device, memory: vk::DeviceMemory, words: &[u32]) -> VulkanComputeResult<()> {
    let size = (words.len() * 4).max(4) as u64;
    let ptr = unsafe { device.map_memory(memory, 0, size, vk::MemoryMapFlags::empty()) }? as *mut u32;
    unsafe { std::ptr::copy_nonoverlapping(words.as_ptr(), ptr, words.len()) };
    unsafe { device.unmap_memory(memory) };
    Ok(())
}

fn read_words(device: &ash::Device, memory: vk::DeviceMemory, len_words: usize) -> VulkanComputeResult<Vec<u32>> {
    let size = (len_words * 4).max(4) as u64;
    let ptr = unsafe { device.map_memory(memory, 0, size, vk::MemoryMapFlags::empty()) }? as *const u32;
    let words = unsafe { std::slice::from_raw_parts(ptr, len_words) }.to_vec();
    unsafe { device.unmap_memory(memory) };
    Ok(words)
}

/// `input_words`(ディスク数分連結済みの入力ワード列)をGPU/NPU上のVulkan
/// コンピュートシェーダで処理し、`num_outputs`個の出力バッファ(それぞれ
/// `stripe_len_words`ワード)を読み戻す。[`crate::compute::dispatch_parity_shader`]
/// (D3D12版)と同じ引数・戻り値の形。
///
/// `spirv_bytes`はビルド時に`glslc`でコンパイル済みのSPIR-Vバイトコード
/// (`build.rs`参照)。失敗した場合は呼び出し側でCPUフォールバックすること。
pub(crate) fn dispatch_parity_shader_vulkan(
    spirv_bytes: &[u8],
    num_disks: u32,
    stripe_len_words: u32,
    input_words: &[u32],
    num_outputs: usize,
) -> VulkanComputeResult<Vec<Vec<u32>>> {
    let entry = unsafe { ash::Entry::load() }.map_err(|_| VulkanComputeError::NoInstance)?;
    let app_info = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_1);
    let instance_info = vk::InstanceCreateInfo::default().application_info(&app_info);
    let instance = unsafe { entry.create_instance(&instance_info, None) }.map_err(|_| VulkanComputeError::NoInstance)?;

    let result = dispatch_with_instance(&instance, spirv_bytes, num_disks, stripe_len_words, input_words, num_outputs);

    unsafe { instance.destroy_instance(None) };
    result
}

fn dispatch_with_instance(
    instance: &ash::Instance,
    spirv_bytes: &[u8],
    num_disks: u32,
    stripe_len_words: u32,
    input_words: &[u32],
    num_outputs: usize,
) -> VulkanComputeResult<Vec<Vec<u32>>> {
    let physical_device = unsafe { instance.enumerate_physical_devices() }?
        .into_iter()
        .next()
        .ok_or(VulkanComputeError::NoDevice)?;

    let queue_family_index = unsafe { instance.get_physical_device_queue_family_properties(physical_device) }
        .iter()
        .position(|qf| qf.queue_flags.contains(vk::QueueFlags::COMPUTE))
        .ok_or(VulkanComputeError::NoDevice)? as u32;

    let queue_priorities = [1.0f32];
    let queue_create_info = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family_index)
        .queue_priorities(&queue_priorities);
    let queue_create_infos = [queue_create_info];
    let device_create_info = vk::DeviceCreateInfo::default().queue_create_infos(&queue_create_infos);
    let device = unsafe { instance.create_device(physical_device, &device_create_info, None) }?;
    let queue = unsafe { device.get_device_queue(queue_family_index, 0) };
    let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };

    let result = (|| -> VulkanComputeResult<Vec<Vec<u32>>> {
        // 入力バッファ(全ディスク連結済み)
        let input_size = (input_words.len() * 4) as u64;
        let (input_buf, input_mem) = create_host_visible_storage_buffer(&device, &mem_props, input_size)?;
        write_words(&device, input_mem, input_words)?;

        // 出力バッファ(num_outputs個)
        let output_size = (stripe_len_words as u64) * 4;
        let mut output_bufs = Vec::with_capacity(num_outputs);
        let mut output_mems = Vec::with_capacity(num_outputs);
        for _ in 0..num_outputs {
            let (buf, mem) = create_host_visible_storage_buffer(&device, &mem_props, output_size)?;
            output_bufs.push(buf);
            output_mems.push(mem);
        }

        // シェーダモジュール
        let code = ash::util::read_spv(&mut std::io::Cursor::new(spirv_bytes)).map_err(|_| VulkanComputeError::NoDevice)?;
        let module_info = vk::ShaderModuleCreateInfo::default().code(&code);
        let shader_module = unsafe { device.create_shader_module(&module_info, None) }?;

        // ディスクリプタセットレイアウト: binding0=入力, binding1..=出力
        let num_bindings = 1 + num_outputs;
        let bindings: Vec<vk::DescriptorSetLayoutBinding> = (0..num_bindings)
            .map(|i| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(i as u32)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE)
            })
            .collect();
        let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let descriptor_set_layout = unsafe { device.create_descriptor_set_layout(&layout_info, None) }?;

        // Params(num_disks, stripe_len_words)をpush constantで渡す
        let push_constant_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .offset(0)
            .size(8);
        let set_layouts = [descriptor_set_layout];
        let push_constant_ranges = [push_constant_range];
        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&set_layouts)
            .push_constant_ranges(&push_constant_ranges);
        let pipeline_layout = unsafe { device.create_pipeline_layout(&pipeline_layout_info, None) }?;

        let entry_point = std::ffi::CString::new("main").unwrap();
        let stage_info = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader_module)
            .name(&entry_point);
        let pipeline_info =
            vk::ComputePipelineCreateInfo::default().stage(stage_info).layout(pipeline_layout);
        let pipelines = unsafe {
            device.create_compute_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
        }
        .map_err(|(_, e)| e)?;
        let pipeline = pipelines[0];

        // ディスクリプタプール・セット
        let pool_size = vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::STORAGE_BUFFER)
            .descriptor_count(num_bindings as u32);
        let pool_sizes = [pool_size];
        let pool_info = vk::DescriptorPoolCreateInfo::default().pool_sizes(&pool_sizes).max_sets(1);
        let descriptor_pool = unsafe { device.create_descriptor_pool(&pool_info, None) }?;
        let set_layouts_alloc = [descriptor_set_layout];
        let alloc_info =
            vk::DescriptorSetAllocateInfo::default().descriptor_pool(descriptor_pool).set_layouts(&set_layouts_alloc);
        let descriptor_sets = unsafe { device.allocate_descriptor_sets(&alloc_info) }?;
        let descriptor_set = descriptor_sets[0];

        let mut buffer_infos: Vec<vk::DescriptorBufferInfo> = Vec::with_capacity(num_bindings);
        buffer_infos.push(vk::DescriptorBufferInfo::default().buffer(input_buf).offset(0).range(vk::WHOLE_SIZE));
        for &buf in &output_bufs {
            buffer_infos.push(vk::DescriptorBufferInfo::default().buffer(buf).offset(0).range(vk::WHOLE_SIZE));
        }
        let writes: Vec<vk::WriteDescriptorSet> = buffer_infos
            .iter()
            .enumerate()
            .map(|(i, info)| {
                vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_set)
                    .dst_binding(i as u32)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(std::slice::from_ref(info))
            })
            .collect();
        unsafe { device.update_descriptor_sets(&writes, &[]) };

        // コマンドプール・バッファ
        let cmd_pool_info = vk::CommandPoolCreateInfo::default().queue_family_index(queue_family_index);
        let cmd_pool = unsafe { device.create_command_pool(&cmd_pool_info, None) }?;
        let cmd_alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(cmd_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let cmd_bufs = unsafe { device.allocate_command_buffers(&cmd_alloc_info) }?;
        let cmd_buf = cmd_bufs[0];

        let begin_info = vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe { device.begin_command_buffer(cmd_buf, &begin_info) }?;
        unsafe {
            device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::COMPUTE, pipeline);
            device.cmd_bind_descriptor_sets(
                cmd_buf,
                vk::PipelineBindPoint::COMPUTE,
                pipeline_layout,
                0,
                &[descriptor_set],
                &[],
            );
            let params = [num_disks, stripe_len_words];
            device.cmd_push_constants(
                cmd_buf,
                pipeline_layout,
                vk::ShaderStageFlags::COMPUTE,
                0,
                std::slice::from_raw_parts(params.as_ptr() as *const u8, 8),
            );
            let thread_groups = stripe_len_words.div_ceil(256).max(1);
            device.cmd_dispatch(cmd_buf, thread_groups, 1, 1);
        }
        unsafe { device.end_command_buffer(cmd_buf) }?;

        let cmd_bufs_submit = [cmd_buf];
        let submit_info = vk::SubmitInfo::default().command_buffers(&cmd_bufs_submit);
        let fence_info = vk::FenceCreateInfo::default();
        let fence = unsafe { device.create_fence(&fence_info, None) }?;
        unsafe { device.queue_submit(queue, &[submit_info], fence) }?;
        unsafe { device.wait_for_fences(&[fence], true, u64::MAX) }?;
        unsafe { device.destroy_fence(fence, None) };

        let mut results = Vec::with_capacity(num_outputs);
        for &mem in &output_mems {
            results.push(read_words(&device, mem, stripe_len_words as usize)?);
        }

        unsafe {
            device.destroy_command_pool(cmd_pool, None);
            device.destroy_descriptor_pool(descriptor_pool, None);
            device.destroy_pipeline(pipeline, None);
            device.destroy_pipeline_layout(pipeline_layout, None);
            device.destroy_descriptor_set_layout(descriptor_set_layout, None);
            device.destroy_shader_module(shader_module, None);
            device.destroy_buffer(input_buf, None);
            device.free_memory(input_mem, None);
            for (&buf, &mem) in output_bufs.iter().zip(output_mems.iter()) {
                device.destroy_buffer(buf, None);
                device.free_memory(mem, None);
            }
        }

        Ok(results)
    })();

    unsafe { device.destroy_device(None) };
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xor_dispatch_matches_cpu_reference_when_vulkan_available() {
        let shader = include_bytes!(concat!(env!("OUT_DIR"), "/raidz_parity.spv"));
        let d0: Vec<u32> = vec![0x1111_1111, 0x2222_2222];
        let d1: Vec<u32> = vec![0x0F0F_0F0F, 0xF0F0_F0F0];
        let num_disks = 2u32;
        let stripe_len_words = 2u32;
        let mut input = d0.clone();
        input.extend_from_slice(&d1);

        let result = match dispatch_parity_shader_vulkan(shader, num_disks, stripe_len_words, &input, 1) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Vulkan対応アクセラレータが見つからないためテストをスキップします: {e}");
                return;
            }
        };

        let expected = crate::raidz_parity::compute_parity_cpu(&[&d0, &d1]);
        assert_eq!(result[0], expected);
    }
}
