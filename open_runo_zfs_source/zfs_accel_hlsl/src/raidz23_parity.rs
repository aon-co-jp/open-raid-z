//! RAID-Z2(二重パリティ)/ RAID-Z3(三重パリティ)のパリティ計算。
//!
//! OpenZFSの`vdev_raidz`と同じ考え方で、各ディスクにGF(2^8)上の係数
//! `2^i`(Q用)・`4^i`(R用、`4^i = 2^(2i)`)を掛けてXOR畳み込みすることで
//! シンドローム(P/Q/R)を生成する(RAID6のReed-Solomon符号化と同一の手法)。
//!
//! 【現状の実装状況】
//! - P/Q/R生成(エンコード)はZ2/Z3とも実装・テスト済み。
//! - 障害復旧(デコード)は「1台欠損(P利用)」「2台欠損(P・Q利用、RAID-Z2相当)」
//!   「3台同時欠損(P・Q・R利用、RAID-Z3相当、[`gf_matrix::GfMatrix`]による
//!   3元連立方程式の求解)」まで実装・テスト済み。
//! - GPU/NPUディスパッチ(HLSL経由)は`shaders/raidz2_parity.hlsl`(P/Q)・
//!   `shaders/raidz3_parity.hlsl`(P/Q/R)の両方を実装・配線済み
//!   ([`compute_pq_accelerated`]/[`compute_pqr_accelerated`])。

use crate::device::AccelDevice;
use crate::galois::GaloisTables;
use crate::gf_matrix::GfMatrix;

/// 全ディスクをXORするだけのPパリティ(RAID-Z1と同じ意味論)。
pub fn compute_p(data_disks: &[&[u8]]) -> Vec<u8> {
    let stripe_len = data_disks.first().map(|s| s.len()).unwrap_or(0);
    let mut p = vec![0u8; stripe_len];
    for disk in data_disks {
        debug_assert_eq!(disk.len(), stripe_len, "全ディスクは同じストライプ長である必要があります");
        for (acc, &b) in p.iter_mut().zip(disk.iter()) {
            *acc ^= b;
        }
    }
    p
}

/// RAID-Z2用のP/Qパリティを生成する。
/// P = XOR(D_i)、Q = XOR(D_i * 2^i)
pub fn compute_pq(data_disks: &[&[u8]], gf: &GaloisTables) -> (Vec<u8>, Vec<u8>) {
    let stripe_len = data_disks.first().map(|s| s.len()).unwrap_or(0);
    let mut p = vec![0u8; stripe_len];
    let mut q = vec![0u8; stripe_len];

    for (i, disk) in data_disks.iter().enumerate() {
        debug_assert_eq!(disk.len(), stripe_len, "全ディスクは同じストライプ長である必要があります");
        let coeff = gf.pow2(i as u32);
        for (byte_idx, &b) in disk.iter().enumerate() {
            p[byte_idx] ^= b;
            q[byte_idx] ^= gf.mul(b, coeff);
        }
    }

    (p, q)
}

/// RAID-Z3用のP/Q/Rパリティを生成する。
/// P = XOR(D_i)、Q = XOR(D_i * 2^i)、R = XOR(D_i * 4^i)
pub fn compute_pqr(data_disks: &[&[u8]], gf: &GaloisTables) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let stripe_len = data_disks.first().map(|s| s.len()).unwrap_or(0);
    let mut p = vec![0u8; stripe_len];
    let mut q = vec![0u8; stripe_len];
    let mut r = vec![0u8; stripe_len];

    for (i, disk) in data_disks.iter().enumerate() {
        debug_assert_eq!(disk.len(), stripe_len, "全ディスクは同じストライプ長である必要があります");
        let q_coeff = gf.pow2(i as u32);
        let r_coeff = gf.pow2(2 * i as u32); // 4^i = (2^2)^i = 2^(2i)
        for (byte_idx, &b) in disk.iter().enumerate() {
            p[byte_idx] ^= b;
            q[byte_idx] ^= gf.mul(b, q_coeff);
            r[byte_idx] ^= gf.mul(b, r_coeff);
        }
    }

    (p, q, r)
}

/// RAID-Z2用P/Qパリティ生成のGPU/NPUディスパッチ版。
///
/// `shaders/raidz2_parity.hlsl`(ビルド時にDXILへ事前コンパイル済み)を
/// D3D12 Compute経由でディスパッチする。バイト列は4バイト単位でu32語へ
/// パックしてシェーダへ渡す(GF(2^8)乗算はシェーダ側で1バイトレーンごとに
/// 独立して行う設計、`shaders/raidz2_parity.hlsl`参照)。ディスパッチに
/// 失敗した場合はCPU実装([`compute_pq`])へフォールバックする。
///
/// RAID-Z3のP/Q/R(3出力)版は[`compute_pqr_accelerated`]を使うこと。
pub fn compute_pq_accelerated(
    device: &AccelDevice,
    data_disks: &[&[u8]],
    gf: &GaloisTables,
) -> (Vec<u8>, Vec<u8>) {
    match device.kind {
        crate::device::AccelKind::Gpu => {
            #[cfg(feature = "gpu")]
            {
                let shader = include_bytes!(concat!(env!("OUT_DIR"), "/raidz2_parity.cso"));
                match compute_pq_gpu(data_disks, shader) {
                    Ok(result) => result,
                    Err(e) => {
                        tracing::warn!(
                            "GPUディスパッチに失敗したため、CPU実装にフォールバックします (device={}, error={e})",
                            device.adapter_description
                        );
                        compute_pq(data_disks, gf)
                    }
                }
            }
            #[cfg(not(feature = "gpu"))]
            {
                compute_pq(data_disks, gf)
            }
        }
        // NPU側は現状GPU版と同一アルゴリズムだが、シェーダバイトコードは
        // `raidnpu_z2_parity.hlsl`由来の別バイナリを使う(経緯は同ファイルの
        // 先頭コメント参照)。将来NPU専用の実装(DirectML等)へ切り替える際、
        // GPU側の検証済みディスパッチには影響しない。
        crate::device::AccelKind::Npu => {
            #[cfg(feature = "gpu")]
            {
                let shader = include_bytes!(concat!(env!("OUT_DIR"), "/raidnpu_z2_parity.cso"));
                match compute_pq_gpu(data_disks, shader) {
                    Ok(result) => result,
                    Err(e) => {
                        tracing::warn!(
                            "NPUディスパッチに失敗したため、CPU実装にフォールバックします (device={}, error={e})",
                            device.adapter_description
                        );
                        compute_pq(data_disks, gf)
                    }
                }
            }
            #[cfg(not(feature = "gpu"))]
            {
                compute_pq(data_disks, gf)
            }
        }
        crate::device::AccelKind::CpuFallback => compute_pq(data_disks, gf),
    }
}

#[cfg(feature = "gpu")]
fn compute_pq_gpu(
    data_disks: &[&[u8]],
    shader: &[u8],
) -> crate::compute::ComputeResult<(Vec<u8>, Vec<u8>)> {
    let stripe_len = data_disks.first().map(|s| s.len()).unwrap_or(0);
    assert_eq!(stripe_len % 4, 0, "GPUディスパッチは4バイト境界のストライプ長のみ対応");

    let num_disks = data_disks.len();
    let stripe_len_words = stripe_len / 4;
    let mut input = Vec::with_capacity(num_disks * stripe_len_words);
    for disk in data_disks {
        input.extend_from_slice(&crate::compute::bytes_to_words(disk));
    }

    let outputs = crate::compute::dispatch_parity_shader(
        shader,
        num_disks as u32,
        stripe_len_words as u32,
        &input,
        2,
    )?;

    let p = crate::compute::words_to_bytes(&outputs[0]);
    let q = crate::compute::words_to_bytes(&outputs[1]);
    Ok((p, q))
}

/// RAID-Z3用P/Q/Rパリティ生成のGPU/NPUディスパッチ版。
///
/// `shaders/raidz3_parity.hlsl`(ビルド時にDXILへ事前コンパイル済み)を
/// D3D12 Compute経由でディスパッチする。ディスパッチに失敗した場合はCPU実装
/// ([`compute_pqr`])へフォールバックする。
pub fn compute_pqr_accelerated(
    device: &AccelDevice,
    data_disks: &[&[u8]],
    gf: &GaloisTables,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    match device.kind {
        crate::device::AccelKind::Gpu => {
            #[cfg(feature = "gpu")]
            {
                let shader = include_bytes!(concat!(env!("OUT_DIR"), "/raidz3_parity.cso"));
                match compute_pqr_gpu(data_disks, shader) {
                    Ok(result) => result,
                    Err(e) => {
                        tracing::warn!(
                            "GPUディスパッチに失敗したため、CPU実装にフォールバックします (device={}, error={e})",
                            device.adapter_description
                        );
                        compute_pqr(data_disks, gf)
                    }
                }
            }
            #[cfg(not(feature = "gpu"))]
            {
                compute_pqr(data_disks, gf)
            }
        }
        // NPU側は現状GPU版と同一アルゴリズムだが、シェーダバイトコードは
        // `raidnpu_z3_parity.hlsl`由来の別バイナリを使う(経緯は
        // raidnpu_parity.hlslの先頭コメント参照)。
        crate::device::AccelKind::Npu => {
            #[cfg(feature = "gpu")]
            {
                let shader = include_bytes!(concat!(env!("OUT_DIR"), "/raidnpu_z3_parity.cso"));
                match compute_pqr_gpu(data_disks, shader) {
                    Ok(result) => result,
                    Err(e) => {
                        tracing::warn!(
                            "NPUディスパッチに失敗したため、CPU実装にフォールバックします (device={}, error={e})",
                            device.adapter_description
                        );
                        compute_pqr(data_disks, gf)
                    }
                }
            }
            #[cfg(not(feature = "gpu"))]
            {
                compute_pqr(data_disks, gf)
            }
        }
        crate::device::AccelKind::CpuFallback => compute_pqr(data_disks, gf),
    }
}

#[cfg(feature = "gpu")]
fn compute_pqr_gpu(
    data_disks: &[&[u8]],
    shader: &[u8],
) -> crate::compute::ComputeResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let stripe_len = data_disks.first().map(|s| s.len()).unwrap_or(0);
    assert_eq!(stripe_len % 4, 0, "GPUディスパッチは4バイト境界のストライプ長のみ対応");

    let num_disks = data_disks.len();
    let stripe_len_words = stripe_len / 4;
    let mut input = Vec::with_capacity(num_disks * stripe_len_words);
    for disk in data_disks {
        input.extend_from_slice(&crate::compute::bytes_to_words(disk));
    }

    let outputs = crate::compute::dispatch_parity_shader(
        shader,
        num_disks as u32,
        stripe_len_words as u32,
        &input,
        3,
    )?;

    let p = crate::compute::words_to_bytes(&outputs[0]);
    let q = crate::compute::words_to_bytes(&outputs[1]);
    let r = crate::compute::words_to_bytes(&outputs[2]);
    Ok((p, q, r))
}

/// 1台のデータディスク欠損をPパリティのみで復元する。
/// `known`には欠損ディスク以外の全ディスクを渡す(インデックスは未使用のため
/// 単純なスライス列でよい)。
pub fn reconstruct_single_missing(known: &[&[u8]], p: &[u8]) -> Vec<u8> {
    let mut result = p.to_vec();
    for disk in known {
        for (acc, &b) in result.iter_mut().zip(disk.iter()) {
            *acc ^= b;
        }
    }
    result
}

/// 2台のデータディスク欠損をP・Qパリティで復元する(RAID6と同じ復旧アルゴリズム)。
///
/// `known`は(元のディスクインデックス, データ)のペア列(欠損した2台以外の全ディスク)。
/// `missing`は欠損した2台の元のディスクインデックス(x, y)。
pub fn reconstruct_double_missing(
    known: &[(usize, &[u8])],
    missing: (usize, usize),
    p: &[u8],
    q: &[u8],
    gf: &GaloisTables,
) -> (Vec<u8>, Vec<u8>) {
    let stripe_len = p.len();
    let mut pxy = p.to_vec();
    let mut qxy = q.to_vec();

    for &(idx, disk) in known {
        let coeff = gf.pow2(idx as u32);
        for byte_idx in 0..stripe_len {
            pxy[byte_idx] ^= disk[byte_idx];
            qxy[byte_idx] ^= gf.mul(disk[byte_idx], coeff);
        }
    }

    let (x, y) = missing;
    let gx = gf.pow2(x as u32);
    let gy = gf.pow2(y as u32);
    let denom = gx ^ gy;
    debug_assert_ne!(denom, 0, "欠損インデックスが重複しています");

    let mut dx = vec![0u8; stripe_len];
    let mut dy = vec![0u8; stripe_len];
    for byte_idx in 0..stripe_len {
        let numerator = qxy[byte_idx] ^ gf.mul(gy, pxy[byte_idx]);
        let recovered_x = gf.div(numerator, denom);
        dx[byte_idx] = recovered_x;
        dy[byte_idx] = pxy[byte_idx] ^ recovered_x;
    }

    (dx, dy)
}

/// 3台のデータディスク欠損をP・Q・Rパリティで復元する(RAID-Z3相当)。
///
/// 3元連立方程式
/// ```text
/// [ 1   1   1  ] [Dx]   [Pxyz]
/// [ gx  gy  gz ] [Dy] = [Qxyz]
/// [ hx  hy  hz ] [Dz]   [Rxyz]
/// ```
/// (g_i = 2^i, h_i = 4^i = (2^i)^2)をGF(2^8)上で解く。係数行列はディスクの
/// 組み合わせ(x,y,z)だけで決まるため、ストライプ全体で1回だけ逆行列を計算し、
/// 各バイトへ適用する。
pub fn reconstruct_triple_missing(
    known: &[(usize, &[u8])],
    missing: (usize, usize, usize),
    p: &[u8],
    q: &[u8],
    r: &[u8],
    gf: &GaloisTables,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let stripe_len = p.len();
    let mut pxyz = p.to_vec();
    let mut qxyz = q.to_vec();
    let mut rxyz = r.to_vec();

    for &(idx, disk) in known {
        let q_coeff = gf.pow2(idx as u32);
        let r_coeff = gf.pow2(2 * idx as u32);
        for byte_idx in 0..stripe_len {
            pxyz[byte_idx] ^= disk[byte_idx];
            qxyz[byte_idx] ^= gf.mul(disk[byte_idx], q_coeff);
            rxyz[byte_idx] ^= gf.mul(disk[byte_idx], r_coeff);
        }
    }

    let (x, y, z) = missing;
    let g = |i: usize| gf.pow2(i as u32);
    let h = |i: usize| gf.pow2(2 * i as u32);
    let coeff_matrix = GfMatrix::new(
        3,
        vec![1, 1, 1, g(x), g(y), g(z), h(x), h(y), h(z)],
    );
    let inv = coeff_matrix
        .invert(gf)
        .expect("欠損インデックスが重複しているか、係数行列が特異です");

    let mut dx = vec![0u8; stripe_len];
    let mut dy = vec![0u8; stripe_len];
    let mut dz = vec![0u8; stripe_len];
    for byte_idx in 0..stripe_len {
        let solved = inv.mul_vec(gf, &[pxyz[byte_idx], qxyz[byte_idx], rxyz[byte_idx]]);
        dx[byte_idx] = solved[0];
        dy[byte_idx] = solved[1];
        dz[byte_idx] = solved[2];
    }

    (dx, dy, dz)
}

/// P/Q/Rのうち「生き残っている任意の組み合わせ」を使ってデータディスクの
/// 欠損を復旧する汎用版。
///
/// [`reconstruct_single_missing`]/[`reconstruct_double_missing`]/
/// [`reconstruct_triple_missing`]は「Pが常に生きている」ことを前提にしているが、
/// 実際には故障がデータディスクとパリティディスクにまたがって発生しうる
/// (例: データ2台+Pパリティが同時に壊れ、Q・Rだけが残る)。その場合でも
/// `missing_data.len()`個の独立した方程式さえ確保できれば復旧可能であり、
/// 本関数はその一般形を提供する。
///
/// `available_parity`は生きているパリティを`(種別, データ)`のペアで渡す
/// (種別: 0=P, 1=Q, 2=R)。要素数は`missing_data.len()`以上である必要がある
/// (実際に使うのは先頭`missing_data.len()`件)。
pub fn reconstruct_missing_data_generic(
    known_data: &[(usize, &[u8])],
    missing_data: &[usize],
    available_parity: &[(u8, &[u8])],
    gf: &GaloisTables,
) -> Vec<(usize, Vec<u8>)> {
    if missing_data.is_empty() {
        return vec![];
    }
    let n = missing_data.len();
    assert!(
        available_parity.len() >= n,
        "復旧に必要な数のパリティが揃っていません(必要{n}件、利用可能{}件)",
        available_parity.len()
    );
    let stripe_len = available_parity[0].1.len();
    let exponents: Vec<u32> = available_parity[..n].iter().map(|(e, _)| *e as u32).collect();

    // 各行(パリティ種別)ごとに、既知データ分を差し引いたシンドロームを計算する。
    // P(exponent=0)/Q(exponent=1)/R(exponent=2)はいずれも
    // 「各ディスクにgf.pow2(exponent*disk_index)を掛けてXOR畳み込む」という
    // 同一の式で表せるため、種別によらず同じロジックで処理できる。
    let mut syndromes: Vec<Vec<u8>> = exponents
        .iter()
        .enumerate()
        .map(|(row, _)| available_parity[row].1.to_vec())
        .collect();
    for (row, &exp) in exponents.iter().enumerate() {
        for &(idx, disk) in known_data {
            let coeff = gf.pow2(exp * idx as u32);
            for b in 0..stripe_len {
                syndromes[row][b] ^= gf.mul(disk[b], coeff);
            }
        }
    }

    let mut matrix_data = vec![0u8; n * n];
    for (row, &exp) in exponents.iter().enumerate() {
        for (col, &idx) in missing_data.iter().enumerate() {
            matrix_data[row * n + col] = gf.pow2(exp * idx as u32);
        }
    }
    let inv = GfMatrix::new(n, matrix_data)
        .invert(gf)
        .expect("係数行列が特異です(欠損インデックスの重複、または不正なパリティ組み合わせ)");

    let mut results: Vec<Vec<u8>> = (0..n).map(|_| vec![0u8; stripe_len]).collect();
    for b in 0..stripe_len {
        let target: Vec<u8> = syndromes.iter().map(|s| s[b]).collect();
        let solved = inv.mul_vec(gf, &target);
        for (col, result) in results.iter_mut().enumerate() {
            result[b] = solved[col];
        }
    }

    missing_data.iter().copied().zip(results).collect()
}

/// [`reconstruct_missing_data_generic`]と同じ結果を返すが、シンドローム計算
/// (既知ディスク×パリティ種別ぶんの「係数倍してXOR畳み込む」処理)を
/// `zfs_accel_hlsl::dml_gemm::linear_combine_via_dml_gemm`でGPU/NPUへ
/// オフロードする。これはscrub/resilverが破損を検知した際に実際に走る
/// 復旧計算の主要コストであり、いわば「パリティチェック」の重い部分。
///
/// ディスパッチに失敗した場合(ドライバ非対応・ハードウェア無し等)は
/// [`reconstruct_missing_data_generic`]と同じCPU計算へ完全にフォールバック
/// するため、`device`に何を渡しても結果は変わらない(速度のみが変わる)。
pub fn reconstruct_missing_data_generic_accelerated(
    device: &AccelDevice,
    known_data: &[(usize, &[u8])],
    missing_data: &[usize],
    available_parity: &[(u8, &[u8])],
    gf: &GaloisTables,
) -> Vec<(usize, Vec<u8>)> {
    if missing_data.is_empty() {
        return vec![];
    }
    let n = missing_data.len();
    assert!(
        available_parity.len() >= n,
        "復旧に必要な数のパリティが揃っていません(必要{n}件、利用可能{}件)",
        available_parity.len()
    );
    let stripe_len = available_parity[0].1.len();
    let exponents: Vec<u32> = available_parity[..n].iter().map(|(e, _)| *e as u32).collect();

    let gpu_contributions: Option<Vec<Vec<u8>>> = match device.kind {
        crate::device::AccelKind::CpuFallback => None,
        _ => {
            #[cfg(feature = "gpu")]
            {
                // 各行(パリティ種別)・既知ディスクごとの係数
                // (gf.pow2(exponent*元インデックス))。
                // CPU版(reconstruct_missing_data_generic)と全く同じ式。
                let known_disks: Vec<&[u8]> = known_data.iter().map(|&(_, d)| d).collect();
                let coeffs_per_output: Vec<Vec<u8>> = exponents
                    .iter()
                    .map(|&exp| known_data.iter().map(|&(idx, _)| gf.pow2(exp * idx as u32)).collect())
                    .collect();
                crate::dml_gemm::linear_combine_via_dml_gemm(gf, &known_disks, &coeffs_per_output).ok()
            }
            #[cfg(not(feature = "gpu"))]
            {
                None
            }
        }
    };

    let mut syndromes: Vec<Vec<u8>> = exponents.iter().enumerate().map(|(row, _)| available_parity[row].1.to_vec()).collect();
    match gpu_contributions {
        Some(contributions) => {
            for (row, contribution) in contributions.iter().enumerate() {
                for b in 0..stripe_len {
                    syndromes[row][b] ^= contribution[b];
                }
            }
        }
        None => {
            for (row, &exp) in exponents.iter().enumerate() {
                for &(idx, disk) in known_data {
                    let coeff = gf.pow2(exp * idx as u32);
                    for b in 0..stripe_len {
                        syndromes[row][b] ^= gf.mul(disk[b], coeff);
                    }
                }
            }
        }
    }

    let mut matrix_data = vec![0u8; n * n];
    for (row, &exp) in exponents.iter().enumerate() {
        for (col, &idx) in missing_data.iter().enumerate() {
            matrix_data[row * n + col] = gf.pow2(exp * idx as u32);
        }
    }
    let inv = GfMatrix::new(n, matrix_data)
        .invert(gf)
        .expect("係数行列が特異です(欠損インデックスの重複、または不正なパリティ組み合わせ)");

    let mut results: Vec<Vec<u8>> = (0..n).map(|_| vec![0u8; stripe_len]).collect();
    for b in 0..stripe_len {
        let target: Vec<u8> = syndromes.iter().map(|s| s[b]).collect();
        let solved = inv.mul_vec(gf, &target);
        for (col, result) in results.iter_mut().enumerate() {
            result[b] = solved[col];
        }
    }

    missing_data.iter().copied().zip(results).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pq_generation_matches_manual_calculation() {
        let gf = GaloisTables::new();
        let d0: Vec<u8> = vec![0x01, 0x02];
        let d1: Vec<u8> = vec![0x03, 0x04];
        let d2: Vec<u8> = vec![0x05, 0x06];
        let (p, q) = compute_pq(&[&d0, &d1, &d2], &gf);

        let expected_p: Vec<u8> = (0..2).map(|i| d0[i] ^ d1[i] ^ d2[i]).collect();
        let expected_q: Vec<u8> = (0..2)
            .map(|i| {
                gf.mul(d0[i], gf.pow2(0)) ^ gf.mul(d1[i], gf.pow2(1)) ^ gf.mul(d2[i], gf.pow2(2))
            })
            .collect();

        assert_eq!(p, expected_p);
        assert_eq!(q, expected_q);
    }

    #[test]
    fn pqr_generation_matches_manual_calculation() {
        let gf = GaloisTables::new();
        let d0: Vec<u8> = vec![0x11, 0x22];
        let d1: Vec<u8> = vec![0x33, 0x44];
        let d2: Vec<u8> = vec![0x55, 0x66];
        let (p, q, r) = compute_pqr(&[&d0, &d1, &d2], &gf);

        let expected_r: Vec<u8> = (0..2)
            .map(|i| {
                gf.mul(d0[i], gf.pow2(0)) ^ gf.mul(d1[i], gf.pow2(2)) ^ gf.mul(d2[i], gf.pow2(4))
            })
            .collect();

        assert_eq!(p, compute_p(&[&d0, &d1, &d2]));
        let (_, q_only) = compute_pq(&[&d0, &d1, &d2], &gf);
        assert_eq!(q, q_only);
        assert_eq!(r, expected_r);
    }

    #[test]
    fn reconstruct_single_missing_recovers_original_disk() {
        let disks: [Vec<u8>; 4] = [
            vec![0x01, 0xAA, 0x10],
            vec![0x02, 0xBB, 0x20],
            vec![0x03, 0xCC, 0x30],
            vec![0x04, 0xDD, 0x40],
        ];
        let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();
        let p = compute_p(&refs);

        // ディスク2(0-indexed)が欠損したと仮定
        let known: Vec<&[u8]> = refs
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 2)
            .map(|(_, d)| *d)
            .collect();

        let recovered = reconstruct_single_missing(&known, &p);
        assert_eq!(recovered, disks[2]);
    }

    #[test]
    fn reconstruct_double_missing_recovers_both_original_disks() {
        let gf = GaloisTables::new();
        let disks: [Vec<u8>; 5] = [
            vec![0x01, 0x9F, 0x10, 0x00],
            vec![0x02, 0x8E, 0x20, 0xFF],
            vec![0x03, 0x7D, 0x30, 0x55],
            vec![0x04, 0x6C, 0x40, 0xAA],
            vec![0x05, 0x5B, 0x50, 0x33],
        ];
        let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();
        let (p, q) = compute_pq(&refs, &gf);

        // ディスク1と3(0-indexed)が同時に欠損したと仮定
        let missing = (1usize, 3usize);
        let known: Vec<(usize, &[u8])> = refs
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != missing.0 && *i != missing.1)
            .map(|(i, d)| (i, *d))
            .collect();

        let (recovered_x, recovered_y) =
            reconstruct_double_missing(&known, missing, &p, &q, &gf);

        assert_eq!(recovered_x, disks[missing.0]);
        assert_eq!(recovered_y, disks[missing.1]);
    }

    #[test]
    fn reconstruct_double_missing_works_for_various_index_pairs() {
        let gf = GaloisTables::new();
        let disks: [Vec<u8>; 6] = [
            vec![0x10, 0x01],
            vec![0x20, 0x02],
            vec![0x30, 0x03],
            vec![0x40, 0x04],
            vec![0x50, 0x05],
            vec![0x60, 0x06],
        ];
        let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();
        let (p, q) = compute_pq(&refs, &gf);

        for missing in [(0usize, 1usize), (2, 5), (4, 0), (3, 4)] {
            let known: Vec<(usize, &[u8])> = refs
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != missing.0 && *i != missing.1)
                .map(|(i, d)| (i, *d))
                .collect();

            let (recovered_x, recovered_y) =
                reconstruct_double_missing(&known, missing, &p, &q, &gf);

            assert_eq!(recovered_x, disks[missing.0], "missing pair {missing:?}");
            assert_eq!(recovered_y, disks[missing.1], "missing pair {missing:?}");
        }
    }

    #[test]
    fn reconstruct_triple_missing_recovers_all_three_original_disks() {
        let gf = GaloisTables::new();
        let disks: [Vec<u8>; 7] = [
            vec![0x01, 0x9F, 0x10],
            vec![0x02, 0x8E, 0x20],
            vec![0x03, 0x7D, 0x30],
            vec![0x04, 0x6C, 0x40],
            vec![0x05, 0x5B, 0x50],
            vec![0x06, 0x4A, 0x60],
            vec![0x07, 0x39, 0x70],
        ];
        let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();
        let (p, q, r) = compute_pqr(&refs, &gf);

        // ディスク1, 3, 5(0-indexed)が同時に欠損したと仮定(RAID-Z3の3台故障)
        let missing = (1usize, 3usize, 5usize);
        let known: Vec<(usize, &[u8])> = refs
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != missing.0 && *i != missing.1 && *i != missing.2)
            .map(|(i, d)| (i, *d))
            .collect();

        let (recovered_x, recovered_y, recovered_z) =
            reconstruct_triple_missing(&known, missing, &p, &q, &r, &gf);

        assert_eq!(recovered_x, disks[missing.0]);
        assert_eq!(recovered_y, disks[missing.1]);
        assert_eq!(recovered_z, disks[missing.2]);
    }

    #[test]
    fn reconstruct_triple_missing_works_for_various_index_triples() {
        let gf = GaloisTables::new();
        let disks: [Vec<u8>; 8] = [
            vec![0x10, 0x01],
            vec![0x20, 0x02],
            vec![0x30, 0x03],
            vec![0x40, 0x04],
            vec![0x50, 0x05],
            vec![0x60, 0x06],
            vec![0x70, 0x07],
            vec![0x80, 0x08],
        ];
        let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();
        let (p, q, r) = compute_pqr(&refs, &gf);

        for missing in [(0usize, 1usize, 2usize), (0, 3, 7), (2, 5, 6), (1, 4, 7)] {
            let known: Vec<(usize, &[u8])> = refs
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != missing.0 && *i != missing.1 && *i != missing.2)
                .map(|(i, d)| (i, *d))
                .collect();

            let (rx, ry, rz) = reconstruct_triple_missing(&known, missing, &p, &q, &r, &gf);

            assert_eq!(rx, disks[missing.0], "missing triple {missing:?}");
            assert_eq!(ry, disks[missing.1], "missing triple {missing:?}");
            assert_eq!(rz, disks[missing.2], "missing triple {missing:?}");
        }
    }

    #[test]
    fn generic_reconstruct_matches_specific_variants_for_p_and_q() {
        let gf = GaloisTables::new();
        let disks: [Vec<u8>; 4] = [
            vec![0x11, 0x22],
            vec![0x33, 0x44],
            vec![0x55, 0x66],
            vec![0x77, 0x88],
        ];
        let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();
        let (p, q) = compute_pq(&refs, &gf);

        let known: Vec<(usize, &[u8])> = vec![(0, disks[0].as_slice()), (2, disks[2].as_slice())];
        let available_parity: Vec<(u8, &[u8])> = vec![(0, &p), (1, &q)];
        let recovered = reconstruct_missing_data_generic(&known, &[1, 3], &available_parity, &gf);

        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[0], (1, disks[1].clone()));
        assert_eq!(recovered[1], (3, disks[3].clone()));
    }

    #[test]
    fn accelerated_generic_reconstruct_matches_cpu_only_variant_on_cpu_fallback() {
        // 実機の有無に関わらず常に実行できる回帰テスト: `device.kind`を
        // 明示的にCpuFallbackにして、GEMM経路を通らない場合でも
        // `reconstruct_missing_data_generic`と完全に同じ結果を返すことを確認する。
        let gf = GaloisTables::new();
        let disks: [Vec<u8>; 4] =
            [vec![0x11, 0x22], vec![0x33, 0x44], vec![0x55, 0x66], vec![0x77, 0x88]];
        let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();
        let (p, q) = compute_pq(&refs, &gf);

        let known: Vec<(usize, &[u8])> = vec![(0, disks[0].as_slice()), (2, disks[2].as_slice())];
        let available_parity: Vec<(u8, &[u8])> = vec![(0, &p), (1, &q)];

        let cpu_device = crate::device::AccelDevice {
            kind: crate::device::AccelKind::CpuFallback,
            adapter_description: "test".to_string(),
        };
        let expected = reconstruct_missing_data_generic(&known, &[1, 3], &available_parity, &gf);
        let actual = reconstruct_missing_data_generic_accelerated(
            &cpu_device,
            &known,
            &[1, 3],
            &available_parity,
            &gf,
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn accelerated_generic_reconstruct_matches_cpu_only_variant_when_hardware_available() {
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

        let gf = GaloisTables::new();
        let disks: [Vec<u8>; 7] = [
            vec![0x01, 0x9F, 0x10],
            vec![0x02, 0x8E, 0x20],
            vec![0x03, 0x7D, 0x30],
            vec![0x04, 0x6C, 0x40],
            vec![0x05, 0x5B, 0x50],
            vec![0x06, 0x4A, 0x60],
            vec![0x07, 0x39, 0x70],
        ];
        let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();
        let (p, q, r) = compute_pqr(&refs, &gf);

        // ディスク1,3,5(0-indexed)が同時に欠損したと仮定(reconstruct_triple_missing系のテストと同条件)。
        let missing = [1usize, 3, 5];
        let known: Vec<(usize, &[u8])> = refs
            .iter()
            .enumerate()
            .filter(|(i, _)| !missing.contains(i))
            .map(|(i, d)| (i, *d))
            .collect();
        let available_parity: Vec<(u8, &[u8])> = vec![(0, &p), (1, &q), (2, &r)];

        let expected = reconstruct_missing_data_generic(&known, &missing, &available_parity, &gf);
        let actual =
            reconstruct_missing_data_generic_accelerated(&device, &known, &missing, &available_parity, &gf);

        assert_eq!(actual, expected);
        for (idx, data) in &actual {
            assert_eq!(data, &disks[*idx]);
        }
    }

    #[test]
    fn generic_reconstruct_works_when_p_is_missing_but_q_and_r_survive() {
        // Pパリティ自体が壊れているケース(データディスクの故障と重なった場合)。
        // Q・Rだけを使って復旧できることを確認する。
        let gf = GaloisTables::new();
        let disks: [Vec<u8>; 4] = [
            vec![0x01, 0xF0],
            vec![0x02, 0xE0],
            vec![0x03, 0xD0],
            vec![0x04, 0xC0],
        ];
        let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();
        let (_p, q, r) = compute_pqr(&refs, &gf);

        // データディスク0,2とPパリティが同時に故障、Q・Rのみ生存という想定
        let known: Vec<(usize, &[u8])> = vec![(1, disks[1].as_slice()), (3, disks[3].as_slice())];
        let available_parity: Vec<(u8, &[u8])> = vec![(1, &q), (2, &r)];
        let recovered = reconstruct_missing_data_generic(&known, &[0, 2], &available_parity, &gf);

        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[0], (0, disks[0].clone()));
        assert_eq!(recovered[1], (2, disks[2].clone()));
    }

    #[test]
    fn compute_pq_accelerated_matches_cpu_when_hardware_available() {
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

        let gf = GaloisTables::new();
        let d0: Vec<u8> = vec![0x01, 0x02, 0x03, 0x04];
        let d1: Vec<u8> = vec![0x11, 0x22, 0x33, 0x44];
        let d2: Vec<u8> = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let refs: Vec<&[u8]> = vec![&d0, &d1, &d2];

        let (expected_p, expected_q) = compute_pq(&refs, &gf);
        let (gpu_p, gpu_q) = compute_pq_accelerated(&device, &refs, &gf);

        assert_eq!(gpu_p, expected_p);
        assert_eq!(gpu_q, expected_q);
    }

    #[test]
    fn compute_pqr_accelerated_matches_cpu_when_hardware_available() {
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

        let gf = GaloisTables::new();
        let d0: Vec<u8> = vec![0x01, 0x02, 0x03, 0x04];
        let d1: Vec<u8> = vec![0x11, 0x22, 0x33, 0x44];
        let d2: Vec<u8> = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let d3: Vec<u8> = vec![0x55, 0x66, 0x77, 0x88];
        let refs: Vec<&[u8]> = vec![&d0, &d1, &d2, &d3];

        let (expected_p, expected_q, expected_r) = compute_pqr(&refs, &gf);
        let (gpu_p, gpu_q, gpu_r) = compute_pqr_accelerated(&device, &refs, &gf);

        assert_eq!(gpu_p, expected_p);
        assert_eq!(gpu_q, expected_q);
        assert_eq!(gpu_r, expected_r);
    }

    // 以下2件は`raidnpu_{z2,z3}_parity.hlsl`(NPU専用ディスパッチ経路用の
    // シェーダバイトコード)自体の正しさを検証する。NPU実機がなくても、
    // D3D12コンピュートディスパッチ機構はデバイスがNPUかGPUかを区別しない
    // ため、このマシンで検出できるD3D12デバイス(GPU等)を使って
    // 「raidnpu_*.hlslのアルゴリズムがCPU参照実装と一致する」ことまでは
    // 検証できる(`compute_pq_accelerated`/`compute_pqr_accelerated`が
    // 実際にNPU検出時にこのバイトコードへ切り替えることの検証は別)。
    #[cfg(feature = "gpu")]
    #[test]
    fn raidnpu_z2_shader_matches_cpu_when_any_d3d12_device_available() {
        match crate::device::detect_best_accelerator() {
            Ok(d) if d.kind != crate::device::AccelKind::CpuFallback => {}
            _ => {
                eprintln!("D3D12対応デバイスが見つからないためテストをスキップします");
                return;
            }
        }

        let gf = GaloisTables::new();
        let d0: Vec<u8> = vec![0x01, 0x02, 0x03, 0x04];
        let d1: Vec<u8> = vec![0x11, 0x22, 0x33, 0x44];
        let d2: Vec<u8> = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let refs: Vec<&[u8]> = vec![&d0, &d1, &d2];

        let (expected_p, expected_q) = compute_pq(&refs, &gf);
        let shader = include_bytes!(concat!(env!("OUT_DIR"), "/raidnpu_z2_parity.cso"));
        let (npu_p, npu_q) = compute_pq_gpu(&refs, shader).expect("raidnpu_z2_parity dispatch failed");

        assert_eq!(npu_p, expected_p);
        assert_eq!(npu_q, expected_q);
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn raidnpu_z3_shader_matches_cpu_when_any_d3d12_device_available() {
        match crate::device::detect_best_accelerator() {
            Ok(d) if d.kind != crate::device::AccelKind::CpuFallback => {}
            _ => {
                eprintln!("D3D12対応デバイスが見つからないためテストをスキップします");
                return;
            }
        }

        let gf = GaloisTables::new();
        let d0: Vec<u8> = vec![0x01, 0x02, 0x03, 0x04];
        let d1: Vec<u8> = vec![0x11, 0x22, 0x33, 0x44];
        let d2: Vec<u8> = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let d3: Vec<u8> = vec![0x55, 0x66, 0x77, 0x88];
        let refs: Vec<&[u8]> = vec![&d0, &d1, &d2, &d3];

        let (expected_p, expected_q, expected_r) = compute_pqr(&refs, &gf);
        let shader = include_bytes!(concat!(env!("OUT_DIR"), "/raidnpu_z3_parity.cso"));
        let (npu_p, npu_q, npu_r) =
            compute_pqr_gpu(&refs, shader).expect("raidnpu_z3_parity dispatch failed");

        assert_eq!(npu_p, expected_p);
        assert_eq!(npu_q, expected_q);
        assert_eq!(npu_r, expected_r);
    }

    #[test]
    fn generic_reconstruct_single_missing_with_only_r_available() {
        let gf = GaloisTables::new();
        let disks: [Vec<u8>; 4] = [
            vec![0xAA, 0xBB],
            vec![0xCC, 0xDD],
            vec![0xEE, 0xFF],
            vec![0x11, 0x22],
        ];
        let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();
        let (_p, _q, r) = compute_pqr(&refs, &gf);

        let known: Vec<(usize, &[u8])> = vec![
            (0, disks[0].as_slice()),
            (1, disks[1].as_slice()),
            (3, disks[3].as_slice()),
        ];
        let available_parity: Vec<(u8, &[u8])> = vec![(2, &r)];
        let recovered = reconstruct_missing_data_generic(&known, &[2], &available_parity, &gf);

        assert_eq!(recovered, vec![(2, disks[2].clone())]);
    }
}
