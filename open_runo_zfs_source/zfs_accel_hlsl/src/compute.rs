//! RAID-Zパリティ計算のD3D12 Computeディスパッチ層。
//!
//! [`crate::device::create_best_device`]が選定したアダプタ上に、そのつど
//! コマンドキュー・ルートシグネチャ・パイプラインステートを作成して1回だけ
//! ディスパッチする(状態を使い回さないシンプルな実装。永続化されたコンテキストを
//! 使い回すのは将来の最適化余地)。
//!
//! ルートシグネチャは、シェーダ側のバッファ(`u0`, `u1`, ...)をディスクリプタ
//! ヒープを介さずルートUAV記述子として直接バインドし、`cbuffer Params`(4個の
//! 32bit値)はルート定数として渡す。RWStructuredBuffer(カウンタなし)は
//! ルートUAV記述子で扱える種類のリソースなので、この単純化が成り立つ。

use crate::device::{create_best_device, DeviceError};
use std::ffi::c_void;
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::ID3DBlob;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};

#[derive(Debug, thiserror::Error)]
pub enum ComputeError {
    #[error("アクセラレータの初期化に失敗しました: {0}")]
    Device(#[from] DeviceError),
    #[error("D3D12 API呼び出しに失敗しました: {0}")]
    Win(#[from] windows::core::Error),
    #[error("ルートシグネチャのシリアライズに失敗しました: {0}")]
    RootSignature(String),
}

pub type ComputeResult<T> = Result<T, ComputeError>;

/// 1ワード=4バイト(u32)としてバイト列を読み書きするためのヘルパー。
/// シェーダ側は`uint`単位で扱うため、byte列は4バイト境界であることを前提とする。
pub(crate) fn bytes_to_words(bytes: &[u8]) -> Vec<u32> {
    assert_eq!(bytes.len() % 4, 0, "データ長は4バイトの倍数である必要があります");
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

pub(crate) fn words_to_bytes(words: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(words.len() * 4);
    for w in words {
        out.extend_from_slice(&w.to_le_bytes());
    }
    out
}

fn create_buffer(
    device: &ID3D12Device,
    heap_type: D3D12_HEAP_TYPE,
    size: u64,
    flags: D3D12_RESOURCE_FLAGS,
    initial_state: D3D12_RESOURCE_STATES,
) -> ComputeResult<ID3D12Resource> {
    let heap_props = D3D12_HEAP_PROPERTIES {
        Type: heap_type,
        ..Default::default()
    };
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Alignment: 0,
        Width: size,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: DXGI_FORMAT_UNKNOWN,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        Flags: flags,
    };
    let mut resource: Option<ID3D12Resource> = None;
    unsafe {
        device.CreateCommittedResource(
            &heap_props,
            D3D12_HEAP_FLAG_NONE,
            &desc,
            initial_state,
            None,
            &mut resource,
        )?;
    }
    Ok(resource.expect("CreateCommittedResourceがOkなのにNoneを返しました"))
}

fn upload_to_gpu(device: &ID3D12Device, words: &[u32]) -> ComputeResult<ID3D12Resource> {
    let size = (words.len() * 4).max(4) as u64;
    let upload = create_buffer(
        device,
        D3D12_HEAP_TYPE_UPLOAD,
        size,
        D3D12_RESOURCE_FLAG_NONE,
        D3D12_RESOURCE_STATE_GENERIC_READ,
    )?;
    unsafe {
        let mut mapped: *mut c_void = std::ptr::null_mut();
        upload.Map(0, None, Some(&mut mapped))?;
        std::ptr::copy_nonoverlapping(words.as_ptr() as *const u8, mapped as *mut u8, words.len() * 4);
        upload.Unmap(0, None);
    }
    Ok(upload)
}

fn create_root_signature(device: &ID3D12Device, num_uavs: u32) -> ComputeResult<ID3D12RootSignature> {
    let mut params: Vec<D3D12_ROOT_PARAMETER> = Vec::with_capacity(num_uavs as usize + 1);
    for i in 0..num_uavs {
        params.push(D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_UAV,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                Descriptor: D3D12_ROOT_DESCRIPTOR {
                    ShaderRegister: i,
                    RegisterSpace: 0,
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
        });
    }
    params.push(D3D12_ROOT_PARAMETER {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
        Anonymous: D3D12_ROOT_PARAMETER_0 {
            Constants: D3D12_ROOT_CONSTANTS {
                ShaderRegister: 0,
                RegisterSpace: 0,
                Num32BitValues: 4,
            },
        },
        ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
    });

    let desc = D3D12_ROOT_SIGNATURE_DESC {
        NumParameters: params.len() as u32,
        pParameters: params.as_ptr(),
        NumStaticSamplers: 0,
        pStaticSamplers: std::ptr::null(),
        Flags: D3D12_ROOT_SIGNATURE_FLAG_NONE,
    };

    let mut blob: Option<ID3DBlob> = None;
    let mut error_blob: Option<ID3DBlob> = None;
    let serialize_result =
        unsafe { D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, Some(&mut error_blob)) };
    if serialize_result.is_err() {
        let message = error_blob
            .map(|b| unsafe {
                let ptr = b.GetBufferPointer() as *const u8;
                let len = b.GetBufferSize();
                String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned()
            })
            .unwrap_or_else(|| "詳細不明".to_string());
        return Err(ComputeError::RootSignature(message));
    }
    let blob = blob.expect("シリアライズ成功時はblobが存在するはずです");
    let bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize()) };
    let root_signature: ID3D12RootSignature = unsafe { device.CreateRootSignature(0, bytes)? };
    Ok(root_signature)
}

fn create_pipeline_state(
    device: &ID3D12Device,
    root_signature: &ID3D12RootSignature,
    shader_bytecode: &[u8],
) -> ComputeResult<ID3D12PipelineState> {
    let desc = D3D12_COMPUTE_PIPELINE_STATE_DESC {
        pRootSignature: unsafe { std::mem::transmute_copy(root_signature) },
        CS: D3D12_SHADER_BYTECODE {
            pShaderBytecode: shader_bytecode.as_ptr() as *const c_void,
            BytecodeLength: shader_bytecode.len(),
        },
        NodeMask: 0,
        CachedPSO: D3D12_CACHED_PIPELINE_STATE::default(),
        Flags: D3D12_PIPELINE_STATE_FLAG_NONE,
    };
    let pso: ID3D12PipelineState = unsafe { device.CreateComputePipelineState(&desc)? };
    Ok(pso)
}

fn transition(resource: &ID3D12Resource, before: D3D12_RESOURCE_STATES, after: D3D12_RESOURCE_STATES) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: std::mem::ManuallyDrop::new(Some(resource.clone())),
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                StateBefore: before,
                StateAfter: after,
            }),
        },
    }
}

/// `input_words`(ディスク数分連結済みの入力ワード列)をGPU/NPU上のコンピュートシェーダで
/// 処理し、`num_outputs`個の出力バッファ(それぞれ`stripe_len_words`ワード)を読み戻す。
///
/// `shader_bytecode`はビルド時に`dxc`でコンパイル済みのDXILバイトコード
/// (`build.rs`参照)。失敗した場合は呼び出し側でCPUフォールバックすること。
pub(crate) fn dispatch_parity_shader(
    shader_bytecode: &[u8],
    num_disks: u32,
    stripe_len_words: u32,
    input_words: &[u32],
    num_outputs: usize,
) -> ComputeResult<Vec<Vec<u32>>> {
    let (_accel, device) = create_best_device()?;

    // 優先度をNORMAL(既定値0)ではなくHIGH(100)にすることで、同じGPU上で
    // 動く他の通常優先度プロセス(ブラウザの動画再生・他アプリの描画等)より
    // 先にこのRAID-Zパリティ計算がスケジューリングされやすくする
    // (`GLOBAL_REALTIME`は管理者権限相当・システム全体への影響が大きい
    // ため意図的に避けている。HIGHはアプリケーション単位で安全に使える
    // 範囲の優先度)。詳細は`crate::priority`モジュールのドキュメント参照。
    let queue: ID3D12CommandQueue = unsafe {
        device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_COMPUTE,
            Priority: D3D12_COMMAND_QUEUE_PRIORITY_HIGH.0,
            Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
            NodeMask: 0,
        })?
    };
    let allocator: ID3D12CommandAllocator =
        unsafe { device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_COMPUTE)? };
    let list: ID3D12GraphicsCommandList = unsafe {
        device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_COMPUTE, &allocator, None)?
    };

    let root_signature = create_root_signature(&device, 1 + num_outputs as u32)?;
    let pso = create_pipeline_state(&device, &root_signature, shader_bytecode)?;

    // 入力バッファ: アップロードヒープへ直接書き込み、既定ヒープへコピーしてからUAVとして使う。
    let input_upload = upload_to_gpu(&device, input_words)?;
    let input_size = (input_words.len() * 4) as u64;
    let input_gpu = create_buffer(
        &device,
        D3D12_HEAP_TYPE_DEFAULT,
        input_size,
        D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
        D3D12_RESOURCE_STATE_COPY_DEST,
    )?;

    let output_size = (stripe_len_words as u64 * 4).max(4);
    let mut outputs_gpu = Vec::with_capacity(num_outputs);
    for _ in 0..num_outputs {
        outputs_gpu.push(create_buffer(
            &device,
            D3D12_HEAP_TYPE_DEFAULT,
            output_size,
            D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
            D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
        )?);
    }
    let mut readbacks = Vec::with_capacity(num_outputs);
    for _ in 0..num_outputs {
        readbacks.push(create_buffer(
            &device,
            D3D12_HEAP_TYPE_READBACK,
            output_size,
            D3D12_RESOURCE_FLAG_NONE,
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?);
    }

    unsafe {
        list.CopyResource(&input_gpu, &input_upload);
        list.ResourceBarrier(&[transition(
            &input_gpu,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
        )]);

        list.SetPipelineState(&pso);
        list.SetComputeRootSignature(&root_signature);
        list.SetComputeRootUnorderedAccessView(0, input_gpu.GetGPUVirtualAddress());
        for (i, out_res) in outputs_gpu.iter().enumerate() {
            list.SetComputeRootUnorderedAccessView(1 + i as u32, out_res.GetGPUVirtualAddress());
        }
        let params = [num_disks, stripe_len_words, 0u32, 0u32];
        list.SetComputeRoot32BitConstants(
            1 + num_outputs as u32,
            4,
            params.as_ptr() as *const c_void,
            0,
        );

        let thread_groups = stripe_len_words.div_ceil(256).max(1);
        list.Dispatch(thread_groups, 1, 1);

        let mut barriers = Vec::with_capacity(num_outputs);
        for out_res in &outputs_gpu {
            barriers.push(transition(
                out_res,
                D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            ));
        }
        list.ResourceBarrier(&barriers);
        for (out_res, readback) in outputs_gpu.iter().zip(readbacks.iter()) {
            list.CopyResource(readback, out_res);
        }

        list.Close()?;
        queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>()?)]);

        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        let fence_event = CreateEventW(None, false, false, None)?;
        let fence_value = 1u64;
        queue.Signal(&fence, fence_value)?;
        if fence.GetCompletedValue() < fence_value {
            fence.SetEventOnCompletion(fence_value, fence_event)?;
            WaitForSingleObject(fence_event, INFINITE);
        }
        windows::Win32::Foundation::CloseHandle(fence_event).ok();
    }

    let mut results = Vec::with_capacity(num_outputs);
    for readback in &readbacks {
        let mut mapped: *mut c_void = std::ptr::null_mut();
        unsafe {
            readback.Map(0, None, Some(&mut mapped))?;
            let bytes = std::slice::from_raw_parts(mapped as *const u8, output_size as usize);
            let words = bytes_to_words(&bytes[..(stripe_len_words as usize * 4)]);
            readback.Unmap(0, None);
            results.push(words);
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    // このテストは実機のD3D12対応アダプタ(GPU/NPU)が必要。CI等でハードウェアが
    // 無い環境では`create_best_device`がErrを返すため、その場合はスキップする。
    #[test]
    fn xor_dispatch_matches_cpu_reference_when_hardware_available() {
        if create_best_device().is_err() {
            eprintln!("D3D12対応アクセラレータが見つからないためテストをスキップします");
            return;
        }

        let shader = include_bytes!(concat!(env!("OUT_DIR"), "/raidz_parity.cso"));
        let d0: Vec<u32> = vec![0x1111_1111, 0x2222_2222];
        let d1: Vec<u32> = vec![0x0F0F_0F0F, 0xF0F0_F0F0];
        let num_disks = 2u32;
        let stripe_len_words = 2u32;
        let mut input = d0.clone();
        input.extend_from_slice(&d1);

        let result = dispatch_parity_shader(shader, num_disks, stripe_len_words, &input, 1)
            .expect("GPUディスパッチに失敗しました");

        let expected = crate::raidz_parity::compute_parity_cpu(&[&d0, &d1]);
        assert_eq!(result[0], expected);
    }
}
