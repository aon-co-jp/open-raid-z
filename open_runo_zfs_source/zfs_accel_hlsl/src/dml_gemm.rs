//! GF(2)ビット行列(`bitmatrix.rs`)によるRAID-Z2/Z3パリティ計算の、
//! DirectML GEMMオペレータ経由でのディスパッチ実装。
//!
//! 【背景】`raidz23_parity::compute_pq_accelerated`/`compute_pqr_accelerated`
//! (生のHLSL Compute Shader経由、`raidz2_parity.hlsl`等)は、NPU上で実行
//! できてもNPUの本領である行列演算(GEMM/畳み込み)ユニットには乗らない
//! (生のCompute Shaderは汎用ALUで動く)。`bitmatrix.rs`で証明した
//! 「GF(2^8)の定数倍 = GF(2)上の8x8線形写像」という事実を使い、複数ディスク
//! ぶんをブロック結合すれば、パリティ計算全体を
//! 「W(8*パリティ数 × 8*ディスク数のGF(2)行列) × X(8*ディスク数 ×
//! ストライプ長のビット行列)」という1回の整数GEMMに帰着できる
//! (各要素をmod 2で戻す後処理つき)。これがDirectMLの`DML_OPERATOR_GEMM`
//! 経由でNPUのMAC/テンソルユニットに載る形。
//!
//! 【重要な注意】本モジュールはproduction dispatch
//! (`raidz23_parity::compute_pq_accelerated`等、および`vdev::RaidZVdev`)には
//! 配線していない。理由:
//! 1. 実機NPUが無く、生Compute Shader版との速度比較ができない
//!    (正しさは実機GPU(D3D12対応デバイス全般)で検証済み、テスト参照)。
//! 2. DirectMLのオペレータ作成・コンパイルはコストが軽くなく、書き込みごとに
//!    毎回これを行うのは非現実的。ディスク構成(ディスク数・ストライプ長)が
//!    変わらない間はコンパイル済みオペレータをキャッシュして再利用する設計
//!    (プール単位で1回だけ構築)が必要で、これは次フェーズの課題。
//!
//! 実行フロー(DirectMLの標準的な使い方): D3D12デバイス→IDMLDevice→
//! オペレータ作成→コンパイル→(必要なら)初期化ディスパッチ→
//! 入出力バッファのバインド→実行ディスパッチ→リードバック。

use crate::bitmatrix::GfBitMatrix;
use crate::compute::{ComputeError, ComputeResult};
use crate::galois::GaloisTables;
use std::ffi::c_void;
use windows::core::Interface;
use windows::Win32::AI::MachineLearning::DirectML::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};

/// `coeff_for_disk(i)`をP/Q/R用の係数として、ディスクごとの`GfBitMatrix`から
/// 「8行 × 8*num_disks列」の重み行列(行優先でflatten済み)を1パリティ種別ぶん構築する。
fn weight_rows_for(gf: &GaloisTables, num_disks: usize, coeff_for_disk: impl Fn(usize) -> u8) -> Vec<f32> {
    let mut rows = vec![0f32; 8 * (8 * num_disks)];
    let cols = 8 * num_disks;
    for i in 0..num_disks {
        let matrix = GfBitMatrix::for_constant(gf, coeff_for_disk(i));
        for j in 0..8 {
            for k in 0..8 {
                rows[j * cols + (i * 8 + k)] = matrix.bit(j, k) as f32;
            }
        }
    }
    rows
}

/// 複数パリティ種別ぶんの重み行(各`8 x 8*num_disks`)を縦に連結し、
/// 「8*パリティ数 × 8*ディスク数」の1つの行列にする。
fn stack_weight_rows(parity_rows: &[Vec<f32>], cols: usize) -> Vec<f32> {
    let mut combined = Vec::with_capacity(parity_rows.iter().map(|r| r.len()).sum());
    for rows in parity_rows {
        debug_assert_eq!(rows.len() % cols, 0);
        combined.extend_from_slice(rows);
    }
    combined
}

/// `data_disks`から「8*num_disks行 × stripe_len列」のビット行列(0.0/1.0)を
/// 行優先でflattenして構築する。行(i*8+k)・列mの値 = disk[i][m]のbit k。
fn data_bit_matrix(data_disks: &[&[u8]]) -> (Vec<f32>, usize) {
    let num_disks = data_disks.len();
    let stripe_len = data_disks.first().map(|d| d.len()).unwrap_or(0);
    let mut x = vec![0f32; (8 * num_disks) * stripe_len];
    for (i, disk) in data_disks.iter().enumerate() {
        debug_assert_eq!(disk.len(), stripe_len, "全ディスクは同じストライプ長である必要があります");
        for k in 0..8usize {
            for (m, &byte) in disk.iter().enumerate() {
                let bit = (byte >> k) & 1;
                x[(i * 8 + k) * stripe_len + m] = bit as f32;
            }
        }
    }
    (x, stripe_len)
}

/// GEMMの生の出力(`out_rows x stripe_len`、行優先)を、8行ずつ1バイトへ
/// mod 2 + ビットパックして`out_rows/8`本のバイト列(各`stripe_len`バイト)に戻す。
fn unpack_mod2_rows_to_bytes(raw: &[f32], out_rows: usize, stripe_len: usize) -> Vec<Vec<u8>> {
    debug_assert_eq!(out_rows % 8, 0);
    let num_parities = out_rows / 8;
    let mut results = vec![vec![0u8; stripe_len]; num_parities];
    for p in 0..num_parities {
        for j in 0..8usize {
            let row = p * 8 + j;
            for m in 0..stripe_len {
                let bit = (raw[row * stripe_len + m].round() as i64) & 1;
                results[p][m] |= (bit as u8) << j;
            }
        }
    }
    results
}

/// RAID-Z2用P/Qパリティを、DirectML GEMM経由で(mod 2のGF(2)行列積として)計算する。
pub fn compute_pq_via_dml_gemm(data_disks: &[&[u8]], gf: &GaloisTables) -> ComputeResult<(Vec<u8>, Vec<u8>)> {
    let num_disks = data_disks.len();
    let cols = 8 * num_disks;
    let p_rows = weight_rows_for(gf, num_disks, |_| 1);
    let q_rows = weight_rows_for(gf, num_disks, |i| gf.pow2(i as u32));
    let w = stack_weight_rows(&[p_rows, q_rows], cols);
    let (x, stripe_len) = data_bit_matrix(data_disks);

    let raw = dispatch_gemm(&w, 16, cols as u32, &x, stripe_len as u32)?;
    let mut bytes = unpack_mod2_rows_to_bytes(&raw, 16, stripe_len);
    let q = bytes.pop().unwrap();
    let p = bytes.pop().unwrap();
    Ok((p, q))
}

/// RAID-Z3用P/Q/Rパリティを、DirectML GEMM経由で(mod 2のGF(2)行列積として)計算する。
pub fn compute_pqr_via_dml_gemm(
    data_disks: &[&[u8]],
    gf: &GaloisTables,
) -> ComputeResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let num_disks = data_disks.len();
    let cols = 8 * num_disks;
    let p_rows = weight_rows_for(gf, num_disks, |_| 1);
    let q_rows = weight_rows_for(gf, num_disks, |i| gf.pow2(i as u32));
    let r_rows = weight_rows_for(gf, num_disks, |i| gf.pow2(2 * i as u32));
    let w = stack_weight_rows(&[p_rows, q_rows, r_rows], cols);
    let (x, stripe_len) = data_bit_matrix(data_disks);

    let raw = dispatch_gemm(&w, 24, cols as u32, &x, stripe_len as u32)?;
    let mut bytes = unpack_mod2_rows_to_bytes(&raw, 24, stripe_len);
    let r = bytes.pop().unwrap();
    let q = bytes.pop().unwrap();
    let p = bytes.pop().unwrap();
    Ok((p, q, r))
}

fn create_device() -> ComputeResult<ID3D12Device> {
    use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};

    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }?;
    let mut index = 0u32;
    loop {
        let adapter = unsafe { factory.EnumAdapters1(index) }
            .map_err(|_| ComputeError::RootSignature("D3D12対応アダプタが見つかりません".to_string()))?;
        index += 1;
        let mut device: Option<ID3D12Device> = None;
        if unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }.is_ok() {
            if let Some(d) = device {
                return Ok(d);
            }
        }
    }
}

fn create_buffer(
    device: &ID3D12Device,
    heap_type: D3D12_HEAP_TYPE,
    size: u64,
    flags: D3D12_RESOURCE_FLAGS,
    state: D3D12_RESOURCE_STATES,
) -> ComputeResult<ID3D12Resource> {
    let heap_props = D3D12_HEAP_PROPERTIES { Type: heap_type, ..Default::default() };
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Alignment: 0,
        Width: size.max(4),
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN,
        SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        Flags: flags,
    };
    let mut resource: Option<ID3D12Resource> = None;
    unsafe { device.CreateCommittedResource(&heap_props, D3D12_HEAP_FLAG_NONE, &desc, state, None, &mut resource) }?;
    resource.ok_or_else(|| ComputeError::RootSignature("バッファの作成に失敗しました".to_string()))
}

fn transition(resource: &ID3D12Resource, before: D3D12_RESOURCE_STATES, after: D3D12_RESOURCE_STATES) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: unsafe { std::mem::transmute_copy(resource) },
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                StateBefore: before,
                StateAfter: after,
            }),
        },
    }
}

fn upload_buffer(device: &ID3D12Device, data: &[f32]) -> ComputeResult<ID3D12Resource> {
    let size = (data.len() * std::mem::size_of::<f32>()) as u64;
    let res = create_buffer(device, D3D12_HEAP_TYPE_UPLOAD, size, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_GENERIC_READ)?;
    unsafe {
        let mut mapped: *mut c_void = std::ptr::null_mut();
        res.Map(0, None, Some(&mut mapped))?;
        std::ptr::copy_nonoverlapping(data.as_ptr() as *const u8, mapped as *mut u8, size as usize);
        res.Unmap(0, None);
    }
    Ok(res)
}

/// `a`(a_rows x k, 行優先, 0.0/1.0のGF(2)行列) × `b`(k x m, 行優先,
/// 0.0/1.0のビット行列)をDirectMLのGEMMオペレータでディスパッチし、
/// 生の(mod 2還元前の)`a_rows x m`の整数値(f32格納)を返す。
///
/// 全ての値は0/1の積算(最大`k`)であり、f32で正確に表現できる範囲に収まる
/// ため、GEMMの浮動小数点演算に丸め誤差の影響は無い。
fn dispatch_gemm(a: &[f32], a_rows: u32, k: u32, b: &[f32], m: u32) -> ComputeResult<Vec<f32>> {
    let device = create_device()?;

    let mut dml_device: Option<IDMLDevice> = None;
    unsafe { DMLCreateDevice(&device, DML_CREATE_DEVICE_FLAG_NONE, &mut dml_device) }?;
    let dml_device = dml_device.ok_or_else(|| ComputeError::RootSignature("DirectMLデバイスの作成に失敗しました".to_string()))?;

    let elem = std::mem::size_of::<f32>() as u64;
    let a_bytes = (a_rows * k) as u64 * elem;
    let b_bytes = (k * m) as u64 * elem;
    let out_bytes = (a_rows * m) as u64 * elem;

    let a_sizes: [u32; 4] = [1, 1, a_rows, k];
    let b_sizes: [u32; 4] = [1, 1, k, m];
    let out_sizes: [u32; 4] = [1, 1, a_rows, m];

    let make_tensor = |sizes: &[u32; 4], total_bytes: u64| DML_BUFFER_TENSOR_DESC {
        DataType: DML_TENSOR_DATA_TYPE_FLOAT32,
        Flags: DML_TENSOR_FLAG_NONE,
        DimensionCount: 4,
        Sizes: sizes.as_ptr(),
        Strides: std::ptr::null(),
        TotalTensorSizeInBytes: total_bytes,
        GuaranteedBaseOffsetAlignment: 0,
    };
    let a_buf = make_tensor(&a_sizes, a_bytes);
    let b_buf = make_tensor(&b_sizes, b_bytes);
    let out_buf = make_tensor(&out_sizes, out_bytes);
    let a_tensor = DML_TENSOR_DESC { Type: DML_TENSOR_TYPE_BUFFER, Desc: &a_buf as *const _ as *const c_void };
    let b_tensor = DML_TENSOR_DESC { Type: DML_TENSOR_TYPE_BUFFER, Desc: &b_buf as *const _ as *const c_void };
    let out_tensor = DML_TENSOR_DESC { Type: DML_TENSOR_TYPE_BUFFER, Desc: &out_buf as *const _ as *const c_void };

    let gemm_desc = DML_GEMM_OPERATOR_DESC {
        ATensor: &a_tensor,
        BTensor: &b_tensor,
        CTensor: std::ptr::null(),
        OutputTensor: &out_tensor,
        TransA: DML_MATRIX_TRANSFORM_NONE,
        TransB: DML_MATRIX_TRANSFORM_NONE,
        Alpha: 1.0,
        Beta: 0.0,
        FusedActivation: std::ptr::null(),
    };
    let op_desc = DML_OPERATOR_DESC { Type: DML_OPERATOR_GEMM, Desc: &gemm_desc as *const _ as *const c_void };

    let mut op: Option<IDMLOperator> = None;
    unsafe { dml_device.CreateOperator(&op_desc, &mut op) }?;
    let op = op.ok_or_else(|| ComputeError::RootSignature("GEMMオペレータの作成に失敗しました".to_string()))?;

    let mut compiled: Option<IDMLCompiledOperator> = None;
    unsafe { dml_device.CompileOperator(&op, DML_EXECUTION_FLAG_NONE, &mut compiled) }?;
    let compiled = compiled.ok_or_else(|| ComputeError::RootSignature("GEMMオペレータのコンパイルに失敗しました".to_string()))?;

    let initializer: IDMLOperatorInitializer = unsafe { dml_device.CreateOperatorInitializer(Some(&[Some(compiled.clone())])) }?;
    let init_dispatchable: IDMLDispatchable = initializer.cast()?;
    let compiled_dispatchable: IDMLDispatchable = compiled.cast()?;

    let init_props = unsafe { init_dispatchable.GetBindingProperties() };
    let exec_props = unsafe { compiled_dispatchable.GetBindingProperties() };

    let descriptor_count = init_props.RequiredDescriptorCount.max(exec_props.RequiredDescriptorCount).max(1);
    let heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
        Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
        NumDescriptors: descriptor_count,
        Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
        NodeMask: 0,
    };
    let descriptor_heap: ID3D12DescriptorHeap = unsafe { device.CreateDescriptorHeap(&heap_desc) }?;

    let queue: ID3D12CommandQueue = unsafe {
        device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_COMPUTE,
            Priority: 0,
            Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
            NodeMask: 0,
        })
    }?;
    let allocator: ID3D12CommandAllocator = unsafe { device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_COMPUTE) }?;
    let list: ID3D12GraphicsCommandList =
        unsafe { device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_COMPUTE, &allocator, None) }?;
    unsafe { list.SetDescriptorHeaps(&[Some(descriptor_heap.clone())]) };

    let cpu_start = unsafe { descriptor_heap.GetCPUDescriptorHandleForHeapStart() };
    let gpu_start = unsafe { descriptor_heap.GetGPUDescriptorHandleForHeapStart() };

    let persistent_resource = if exec_props.PersistentResourceSize > 0 {
        Some(create_buffer(
            &device,
            D3D12_HEAP_TYPE_DEFAULT,
            exec_props.PersistentResourceSize,
            D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
            D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
        )?)
    } else {
        None
    };

    // === 初期化ディスパッチ(GEMMは学習済み永続状態を持たないため通常は無処理) ===
    {
        let binding_table: IDMLBindingTable = unsafe {
            dml_device.CreateBindingTable(Some(&DML_BINDING_TABLE_DESC {
                Dispatchable: std::mem::ManuallyDrop::new(Some(init_dispatchable.clone())),
                CPUDescriptorHandle: cpu_start,
                GPUDescriptorHandle: gpu_start,
                SizeInDescriptors: descriptor_count,
            }))
        }?;
        if init_props.TemporaryResourceSize > 0 {
            let res = create_buffer(
                &device,
                D3D12_HEAP_TYPE_DEFAULT,
                init_props.TemporaryResourceSize,
                D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
                D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
            )?;
            let binding = DML_BUFFER_BINDING { Buffer: std::mem::ManuallyDrop::new(Some(res)), Offset: 0, SizeInBytes: init_props.TemporaryResourceSize };
            let desc = DML_BINDING_DESC { Type: DML_BINDING_TYPE_BUFFER, Desc: &binding as *const _ as *const c_void };
            unsafe { binding_table.BindTemporaryResource(Some(&desc as *const _)) };
        }
        if let Some(res) = &persistent_resource {
            let binding = DML_BUFFER_BINDING { Buffer: std::mem::ManuallyDrop::new(Some(res.clone())), Offset: 0, SizeInBytes: exec_props.PersistentResourceSize };
            let desc = DML_BINDING_DESC { Type: DML_BINDING_TYPE_BUFFER, Desc: &binding as *const _ as *const c_void };
            unsafe { binding_table.BindOutputs(Some(&[desc])) };
        }
        let recorder: IDMLCommandRecorder = unsafe { dml_device.CreateCommandRecorder() }?;
        unsafe { recorder.RecordDispatch(&list, &init_dispatchable, &binding_table) };
    }

    // === A/Bのアップロード + 実行ディスパッチ ===
    let a_gpu = create_buffer(&device, D3D12_HEAP_TYPE_DEFAULT, a_bytes, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_DEST)?;
    let b_gpu = create_buffer(&device, D3D12_HEAP_TYPE_DEFAULT, b_bytes, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_DEST)?;
    let out_gpu = create_buffer(&device, D3D12_HEAP_TYPE_DEFAULT, out_bytes, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_UNORDERED_ACCESS)?;
    let a_upload = upload_buffer(&device, a)?;
    let b_upload = upload_buffer(&device, b)?;

    unsafe {
        list.CopyResource(&a_gpu, &a_upload);
        list.CopyResource(&b_gpu, &b_upload);
        list.ResourceBarrier(&[
            transition(&a_gpu, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_UNORDERED_ACCESS),
            transition(&b_gpu, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_UNORDERED_ACCESS),
        ]);
    }

    {
        let binding_table: IDMLBindingTable = unsafe {
            dml_device.CreateBindingTable(Some(&DML_BINDING_TABLE_DESC {
                Dispatchable: std::mem::ManuallyDrop::new(Some(compiled_dispatchable.clone())),
                CPUDescriptorHandle: cpu_start,
                GPUDescriptorHandle: gpu_start,
                SizeInDescriptors: descriptor_count,
            }))
        }?;

        let a_binding = DML_BUFFER_BINDING { Buffer: std::mem::ManuallyDrop::new(Some(a_gpu.clone())), Offset: 0, SizeInBytes: a_bytes };
        let b_binding = DML_BUFFER_BINDING { Buffer: std::mem::ManuallyDrop::new(Some(b_gpu.clone())), Offset: 0, SizeInBytes: b_bytes };
        let a_desc = DML_BINDING_DESC { Type: DML_BINDING_TYPE_BUFFER, Desc: &a_binding as *const _ as *const c_void };
        let b_desc = DML_BINDING_DESC { Type: DML_BINDING_TYPE_BUFFER, Desc: &b_binding as *const _ as *const c_void };
        // GEMMはA/B/Cの3入力スロットを持つ演算子として作成されるため
        // (CTensor=nullでも)、BindInputsには常に3件渡す必要がある
        // (未使用のCはType=NONEのプレースホルダ)。
        let c_desc = DML_BINDING_DESC { Type: DML_BINDING_TYPE_NONE, Desc: std::ptr::null() };
        unsafe { binding_table.BindInputs(Some(&[a_desc, b_desc, c_desc])) };

        let out_binding = DML_BUFFER_BINDING { Buffer: std::mem::ManuallyDrop::new(Some(out_gpu.clone())), Offset: 0, SizeInBytes: out_bytes };
        let out_desc = DML_BINDING_DESC { Type: DML_BINDING_TYPE_BUFFER, Desc: &out_binding as *const _ as *const c_void };
        unsafe { binding_table.BindOutputs(Some(&[out_desc])) };

        if exec_props.TemporaryResourceSize > 0 {
            let res = create_buffer(
                &device,
                D3D12_HEAP_TYPE_DEFAULT,
                exec_props.TemporaryResourceSize,
                D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
                D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
            )?;
            let binding = DML_BUFFER_BINDING { Buffer: std::mem::ManuallyDrop::new(Some(res)), Offset: 0, SizeInBytes: exec_props.TemporaryResourceSize };
            let desc = DML_BINDING_DESC { Type: DML_BINDING_TYPE_BUFFER, Desc: &binding as *const _ as *const c_void };
            unsafe { binding_table.BindTemporaryResource(Some(&desc as *const _)) };
        }
        if let Some(res) = &persistent_resource {
            let binding = DML_BUFFER_BINDING { Buffer: std::mem::ManuallyDrop::new(Some(res.clone())), Offset: 0, SizeInBytes: exec_props.PersistentResourceSize };
            let desc = DML_BINDING_DESC { Type: DML_BINDING_TYPE_BUFFER, Desc: &binding as *const _ as *const c_void };
            unsafe { binding_table.BindPersistentResource(Some(&desc as *const _)) };
        }

        let recorder: IDMLCommandRecorder = unsafe { dml_device.CreateCommandRecorder() }?;
        unsafe { recorder.RecordDispatch(&list, &compiled_dispatchable, &binding_table) };
    }

    // === リードバック ===
    let readback = create_buffer(&device, D3D12_HEAP_TYPE_READBACK, out_bytes, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST)?;
    unsafe {
        list.ResourceBarrier(&[transition(&out_gpu, D3D12_RESOURCE_STATE_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
        list.CopyResource(&readback, &out_gpu);
        list.Close()?;
        queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>()?)]);

        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        let event = CreateEventW(None, false, false, None)?;
        queue.Signal(&fence, 1)?;
        if fence.GetCompletedValue() < 1 {
            fence.SetEventOnCompletion(1, event)?;
            WaitForSingleObject(event, INFINITE);
        }
        windows::Win32::Foundation::CloseHandle(event).ok();

        let mut mapped: *mut c_void = std::ptr::null_mut();
        readback.Map(0, None, Some(&mut mapped))?;
        let out_slice = std::slice::from_raw_parts(mapped as *const f32, (a_rows * m) as usize);
        let result = out_slice.to_vec();
        readback.Unmap(0, None);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn any_d3d12_device_available() -> bool {
        create_device().is_ok()
    }

    #[test]
    fn compute_pq_via_dml_gemm_matches_cpu_reference() {
        if !any_d3d12_device_available() {
            eprintln!("D3D12対応デバイスが見つからないためテストをスキップします");
            return;
        }
        let gf = GaloisTables::new();
        let d0: Vec<u8> = vec![0x01, 0x02, 0x03, 0x04];
        let d1: Vec<u8> = vec![0x11, 0x22, 0x33, 0x44];
        let d2: Vec<u8> = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let refs: Vec<&[u8]> = vec![&d0, &d1, &d2];

        let expected = crate::raidz23_parity::compute_pq(&refs, &gf);
        let actual = compute_pq_via_dml_gemm(&refs, &gf).expect("dml gemm dispatch failed");

        assert_eq!(actual, expected);
    }

    #[test]
    fn compute_pqr_via_dml_gemm_matches_cpu_reference() {
        if !any_d3d12_device_available() {
            eprintln!("D3D12対応デバイスが見つからないためテストをスキップします");
            return;
        }
        let gf = GaloisTables::new();
        let d0: Vec<u8> = vec![0x01, 0x02, 0x03, 0x04];
        let d1: Vec<u8> = vec![0x11, 0x22, 0x33, 0x44];
        let d2: Vec<u8> = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let d3: Vec<u8> = vec![0x55, 0x66, 0x77, 0x88];
        let refs: Vec<&[u8]> = vec![&d0, &d1, &d2, &d3];

        let expected = crate::raidz23_parity::compute_pqr(&refs, &gf);
        let actual = compute_pqr_via_dml_gemm(&refs, &gf).expect("dml gemm dispatch failed");

        assert_eq!(actual, expected);
    }

    #[test]
    fn compute_pqr_via_dml_gemm_matches_reference_for_several_disk_counts() {
        if !any_d3d12_device_available() {
            eprintln!("D3D12対応デバイスが見つからないためテストをスキップします");
            return;
        }
        let gf = GaloisTables::new();
        for num_disks in [2usize, 3, 5, 8] {
            let disks: Vec<Vec<u8>> = (0..num_disks)
                .map(|i| (0..4u8).map(|b| b.wrapping_mul(53).wrapping_add(i as u8 * 17).wrapping_add(3)).collect())
                .collect();
            let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();

            let expected = crate::raidz23_parity::compute_pqr(&refs, &gf);
            let actual = compute_pqr_via_dml_gemm(&refs, &gf).expect("dml gemm dispatch failed");

            assert_eq!(actual, expected, "num_disks={num_disks}で不一致");
        }
    }
}
