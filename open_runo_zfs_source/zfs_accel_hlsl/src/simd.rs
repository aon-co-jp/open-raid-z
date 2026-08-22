//! RAID-Z2/Z3(RAID6相当)のP/Q/Rシンドローム計算に用いる、
//! CPU SIMD(AVX-512 / AVX2 / SSE2)による高速化カーネル。
//!
//! 【設計方針】
//! - **実行時CPU機能検出**(`std::is_x86_feature_detected!`)で経路を選び、
//!   AVX-512非搭載CPU・非x86アーキテクチャでは必ずスカラー実装へ
//!   安全にフォールバックする(ビルド自体はどの環境でも通る)。
//! - GF(2^8)の乗算テーブル(exp/log)を引かず、Linuxカーネルの
//!   `lib/raid6/avx512.c`・H. Peter Anvin "The mathematics of RAID-6" と
//!   同じ**「×2 = 左1ビットシフト + 最上位ビットが立っていたバイトにだけ
//!   既約多項式 0x1D をXOR」**という定式化を用いる。これにより
//!   バイト単位のGF乗算がSIMDのバイト並列演算だけで表現でき、
//!   ギャザー(テーブル引き)が不要になる。
//! - シンドロームはHorner法で畳み込む:
//!   `Q = Σ D_i · 2^i` は `q = 0; for i in (0..n).rev() { q = q*2 ^ D_i }`、
//!   `R = Σ D_i · 4^i` は同様に `q = q*4 ^ D_i` で得られる
//!   (係数テーブルを持たない分、`compute_pq`の素朴実装と数値的に
//!   完全一致することを単体テストで検証している)。
//!
//! 【正直な開示】
//! この開発機のCPUはAMD Ryzen 9 3950X(Zen 2)でAVX-512を**搭載していない**。
//! そのためAVX-512経路は「コンパイル可能・ロジックはスカラー/AVX2経路と
//! 同一の定式化」までしか確認できておらず、**実機での実行・ベンチマークは
//! 未実施**である。AVX2/SSE2/スカラー経路は実機で実行・検証済み。

/// 実行時に選択されたSIMD経路(ログ・ベンチマーク表示用)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdLevel {
    /// AVX-512F + AVX-512BW(64バイト/命令)
    Avx512,
    /// AVX2(32バイト/命令)
    Avx2,
    /// SSE2(16バイト/命令)
    Sse2,
    /// SIMD無し(u64語単位のビットトリック)
    Scalar,
}

impl SimdLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            SimdLevel::Avx512 => "avx512",
            SimdLevel::Avx2 => "avx2",
            SimdLevel::Sse2 => "sse2",
            SimdLevel::Scalar => "scalar",
        }
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_level() -> SimdLevel {
    // AVX-512はF(基盤)とBW(バイト/ワード演算)の両方が必要
    // (バイト単位の比較・加算・マスク生成にBWを使うため)。
    if std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx512bw") {
        return SimdLevel::Avx512;
    }
    if std::is_x86_feature_detected!("avx2") {
        return SimdLevel::Avx2;
    }
    if std::is_x86_feature_detected!("sse2") {
        return SimdLevel::Sse2;
    }
    SimdLevel::Scalar
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn detect_level() -> SimdLevel {
    SimdLevel::Scalar
}

/// この実行環境で実際に使われるSIMD経路を返す(初回のみCPU機能検出)。
pub fn simd_level() -> SimdLevel {
    use std::sync::OnceLock;
    static LEVEL: OnceLock<SimdLevel> = OnceLock::new();
    *LEVEL.get_or_init(detect_level)
}

// ---------------------------------------------------------------------------
// スカラー(u64ビットトリック)実装 — 全アーキテクチャ共通のフォールバック
// ---------------------------------------------------------------------------

const MASK_HIGH: u64 = 0x8080_8080_8080_8080;
const MASK_LOW7: u64 = 0x7f7f_7f7f_7f7f_7f7f;
const POLY: u64 = 0x1d1d_1d1d_1d1d_1d1d;

/// u64語に詰めた8バイトを、それぞれGF(2^8)上で2倍する。
#[inline(always)]
fn mul2_u64(w: u64) -> u64 {
    let high = w & MASK_HIGH;
    // 最上位ビットが立っているバイトを 0xFF に展開する
    let mask = (high >> 7).wrapping_mul(0xff);
    ((w & MASK_LOW7) << 1) ^ (mask & POLY)
}

fn xor_into_scalar(dst: &mut [u8], src: &[u8]) {
    for (d, &s) in dst.iter_mut().zip(src.iter()) {
        *d ^= s;
    }
}

fn mul2_xor_into_scalar(acc: &mut [u8], src: &[u8]) {
    let n = acc.len();
    let mut i = 0;
    while i + 8 <= n {
        let a = u64::from_le_bytes(acc[i..i + 8].try_into().unwrap());
        let s = u64::from_le_bytes(src[i..i + 8].try_into().unwrap());
        acc[i..i + 8].copy_from_slice(&(mul2_u64(a) ^ s).to_le_bytes());
        i += 8;
    }
    while i < n {
        acc[i] = mul2_byte(acc[i]) ^ src[i];
        i += 1;
    }
}

fn mul4_xor_into_scalar(acc: &mut [u8], src: &[u8]) {
    let n = acc.len();
    let mut i = 0;
    while i + 8 <= n {
        let a = u64::from_le_bytes(acc[i..i + 8].try_into().unwrap());
        let s = u64::from_le_bytes(src[i..i + 8].try_into().unwrap());
        acc[i..i + 8].copy_from_slice(&(mul2_u64(mul2_u64(a)) ^ s).to_le_bytes());
        i += 8;
    }
    while i < n {
        acc[i] = mul2_byte(mul2_byte(acc[i])) ^ src[i];
        i += 1;
    }
}

/// 1バイトのGF(2^8)上の2倍(端数処理・テスト用)。
#[inline(always)]
pub fn mul2_byte(b: u8) -> u8 {
    ((b << 1) ^ if b & 0x80 != 0 { 0x1d } else { 0x00 }) as u8
}

// ---------------------------------------------------------------------------
// x86 SIMD 実装
// ---------------------------------------------------------------------------

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    // ---- SSE2 ----
    #[target_feature(enable = "sse2")]
    pub unsafe fn xor_into(dst: &mut [u8], src: &[u8]) -> usize {
        let n = dst.len().min(src.len());
        let mut i = 0;
        while i + 16 <= n {
            let a = _mm_loadu_si128(dst.as_ptr().add(i) as *const __m128i);
            let b = _mm_loadu_si128(src.as_ptr().add(i) as *const __m128i);
            _mm_storeu_si128(dst.as_mut_ptr().add(i) as *mut __m128i, _mm_xor_si128(a, b));
            i += 16;
        }
        i
    }

    #[target_feature(enable = "sse2")]
    unsafe fn mul2_sse2(v: __m128i) -> __m128i {
        let zero = _mm_setzero_si128();
        // 符号付き比較で「最上位ビットが立っているバイト」を 0xFF にする
        let mask = _mm_cmpgt_epi8(zero, v);
        let shifted = _mm_add_epi8(v, v); // バイト単位の1ビット左シフト
        _mm_xor_si128(shifted, _mm_and_si128(mask, _mm_set1_epi8(0x1du8 as i8)))
    }

    #[target_feature(enable = "sse2")]
    pub unsafe fn mul2_xor_into(acc: &mut [u8], src: &[u8], times: u32) -> usize {
        let n = acc.len().min(src.len());
        let mut i = 0;
        while i + 16 <= n {
            let mut a = _mm_loadu_si128(acc.as_ptr().add(i) as *const __m128i);
            for _ in 0..times {
                a = mul2_sse2(a);
            }
            let s = _mm_loadu_si128(src.as_ptr().add(i) as *const __m128i);
            _mm_storeu_si128(acc.as_mut_ptr().add(i) as *mut __m128i, _mm_xor_si128(a, s));
            i += 16;
        }
        i
    }

    // ---- AVX2 ----
    #[target_feature(enable = "avx2")]
    pub unsafe fn xor_into_avx2(dst: &mut [u8], src: &[u8]) -> usize {
        let n = dst.len().min(src.len());
        let mut i = 0;
        while i + 32 <= n {
            let a = _mm256_loadu_si256(dst.as_ptr().add(i) as *const __m256i);
            let b = _mm256_loadu_si256(src.as_ptr().add(i) as *const __m256i);
            _mm256_storeu_si256(dst.as_mut_ptr().add(i) as *mut __m256i, _mm256_xor_si256(a, b));
            i += 32;
        }
        i
    }

    #[target_feature(enable = "avx2")]
    unsafe fn mul2_avx2(v: __m256i) -> __m256i {
        let zero = _mm256_setzero_si256();
        let mask = _mm256_cmpgt_epi8(zero, v);
        let shifted = _mm256_add_epi8(v, v);
        _mm256_xor_si256(shifted, _mm256_and_si256(mask, _mm256_set1_epi8(0x1du8 as i8)))
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn mul2_xor_into_avx2(acc: &mut [u8], src: &[u8], times: u32) -> usize {
        let n = acc.len().min(src.len());
        let mut i = 0;
        while i + 32 <= n {
            let mut a = _mm256_loadu_si256(acc.as_ptr().add(i) as *const __m256i);
            for _ in 0..times {
                a = mul2_avx2(a);
            }
            let s = _mm256_loadu_si256(src.as_ptr().add(i) as *const __m256i);
            _mm256_storeu_si256(acc.as_mut_ptr().add(i) as *mut __m256i, _mm256_xor_si256(a, s));
            i += 32;
        }
        i
    }

    // ---- AVX-512(F + BW) ----
    // 【正直な開示】この開発機(Ryzen 9 3950X / Zen 2)はAVX-512非搭載のため、
    // 以下の関数はコンパイル確認のみで、実行・ベンチマークは未実施。
    #[target_feature(enable = "avx512f,avx512bw")]
    pub unsafe fn xor_into_avx512(dst: &mut [u8], src: &[u8]) -> usize {
        let n = dst.len().min(src.len());
        let mut i = 0;
        while i + 64 <= n {
            let a = _mm512_loadu_si512(dst.as_ptr().add(i) as *const __m512i);
            let b = _mm512_loadu_si512(src.as_ptr().add(i) as *const __m512i);
            _mm512_storeu_si512(dst.as_mut_ptr().add(i) as *mut __m512i, _mm512_xor_si512(a, b));
            i += 64;
        }
        i
    }

    #[target_feature(enable = "avx512f,avx512bw")]
    unsafe fn mul2_avx512(v: __m512i) -> __m512i {
        // AVX-512はマスクレジスタ(k)を持つため、比較→ブレンドが1命令ずつで済む。
        let msb = _mm512_movepi8_mask(v); // 各バイトの最上位ビットをkマスクへ
        let shifted = _mm512_add_epi8(v, v);
        let poly = _mm512_maskz_set1_epi8(msb, 0x1du8 as i8);
        _mm512_xor_si512(shifted, poly)
    }

    #[target_feature(enable = "avx512f,avx512bw")]
    pub unsafe fn mul2_xor_into_avx512(acc: &mut [u8], src: &[u8], times: u32) -> usize {
        let n = acc.len().min(src.len());
        let mut i = 0;
        while i + 64 <= n {
            let mut a = _mm512_loadu_si512(acc.as_ptr().add(i) as *const __m512i);
            for _ in 0..times {
                a = mul2_avx512(a);
            }
            let s = _mm512_loadu_si512(src.as_ptr().add(i) as *const __m512i);
            _mm512_storeu_si512(acc.as_mut_ptr().add(i) as *mut __m512i, _mm512_xor_si512(a, s));
            i += 64;
        }
        i
    }

    // ---- 任意係数のGF(2^8)乗算(split-table + PSHUFB) ----
    // ISA-L(Intel Intelligent Storage Acceleration Library)の
    // `gf_vect_mad`系カーネルと同じ定式化:
    //   b = (b_hi<<4) | b_lo  なので  c·b = c·b_lo ^ c·b_hi
    // 各16要素のテーブルをレジスタへ載せ、PSHUFB(バイト単位テーブル引き)で
    // 16/32/64バイトを一度に変換する。
    #[target_feature(enable = "ssse3")]
    pub unsafe fn gf_mul_xor_into_ssse3(acc: &mut [u8], src: &[u8], lo: &[u8; 16], hi: &[u8; 16]) -> usize {
        let n = acc.len().min(src.len());
        let lo_v = _mm_loadu_si128(lo.as_ptr() as *const __m128i);
        let hi_v = _mm_loadu_si128(hi.as_ptr() as *const __m128i);
        let nibble = _mm_set1_epi8(0x0f);
        let mut i = 0;
        while i + 16 <= n {
            let s = _mm_loadu_si128(src.as_ptr().add(i) as *const __m128i);
            let l = _mm_and_si128(s, nibble);
            let h = _mm_and_si128(_mm_srli_epi64(s, 4), nibble);
            let prod = _mm_xor_si128(_mm_shuffle_epi8(lo_v, l), _mm_shuffle_epi8(hi_v, h));
            let a = _mm_loadu_si128(acc.as_ptr().add(i) as *const __m128i);
            _mm_storeu_si128(acc.as_mut_ptr().add(i) as *mut __m128i, _mm_xor_si128(a, prod));
            i += 16;
        }
        i
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn gf_mul_xor_into_avx2(acc: &mut [u8], src: &[u8], lo: &[u8; 16], hi: &[u8; 16]) -> usize {
        let n = acc.len().min(src.len());
        // 128bitテーブルを上下レーンへブロードキャスト(PSHUFBはレーン内で閉じるため)
        let lo_v = _mm256_broadcastsi128_si256(_mm_loadu_si128(lo.as_ptr() as *const __m128i));
        let hi_v = _mm256_broadcastsi128_si256(_mm_loadu_si128(hi.as_ptr() as *const __m128i));
        let nibble = _mm256_set1_epi8(0x0f);
        let mut i = 0;
        while i + 32 <= n {
            let s = _mm256_loadu_si256(src.as_ptr().add(i) as *const __m256i);
            let l = _mm256_and_si256(s, nibble);
            let h = _mm256_and_si256(_mm256_srli_epi64(s, 4), nibble);
            let prod = _mm256_xor_si256(_mm256_shuffle_epi8(lo_v, l), _mm256_shuffle_epi8(hi_v, h));
            let a = _mm256_loadu_si256(acc.as_ptr().add(i) as *const __m256i);
            _mm256_storeu_si256(acc.as_mut_ptr().add(i) as *mut __m256i, _mm256_xor_si256(a, prod));
            i += 32;
        }
        i
    }

    /// 【正直な開示】AVX-512BW版。この開発機はAVX-512非搭載のため実機未検証。
    #[target_feature(enable = "avx512f,avx512bw")]
    pub unsafe fn gf_mul_xor_into_avx512(acc: &mut [u8], src: &[u8], lo: &[u8; 16], hi: &[u8; 16]) -> usize {
        let n = acc.len().min(src.len());
        let lo128 = _mm_loadu_si128(lo.as_ptr() as *const __m128i);
        let hi128 = _mm_loadu_si128(hi.as_ptr() as *const __m128i);
        // 512bitの4レーンすべてへ同じ16バイトテーブルを複製する。
        let lo_v = _mm512_broadcast_i32x4(lo128);
        let hi_v = _mm512_broadcast_i32x4(hi128);
        let nibble = _mm512_set1_epi8(0x0f);
        let mut i = 0;
        while i + 64 <= n {
            let s = _mm512_loadu_si512(src.as_ptr().add(i) as *const __m512i);
            let l = _mm512_and_si512(s, nibble);
            let h = _mm512_and_si512(_mm512_srli_epi64(s, 4), nibble);
            let prod = _mm512_xor_si512(_mm512_shuffle_epi8(lo_v, l), _mm512_shuffle_epi8(hi_v, h));
            let a = _mm512_loadu_si512(acc.as_ptr().add(i) as *const __m512i);
            _mm512_storeu_si512(acc.as_mut_ptr().add(i) as *mut __m512i, _mm512_xor_si512(a, prod));
            i += 64;
        }
        i
    }
}

// ---------------------------------------------------------------------------
// 公開API(実行時ディスパッチ)
// ---------------------------------------------------------------------------

/// `dst ^= src`(バイト単位XOR、Pパリティの畳み込み)。
pub fn xor_into(dst: &mut [u8], src: &[u8]) {
    debug_assert_eq!(dst.len(), src.len());
    let n = dst.len().min(src.len());
    #[allow(unused_assignments)]
    let mut done = 0usize;
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        done = match simd_level() {
            SimdLevel::Avx512 => unsafe { x86::xor_into_avx512(&mut dst[..n], &src[..n]) },
            SimdLevel::Avx2 => unsafe { x86::xor_into_avx2(&mut dst[..n], &src[..n]) },
            SimdLevel::Sse2 => unsafe { x86::xor_into(&mut dst[..n], &src[..n]) },
            SimdLevel::Scalar => 0,
        };
    }
    if done < n {
        xor_into_scalar(&mut dst[done..n], &src[done..n]);
    }
}

/// `acc = acc*2 ^ src`(Qシンドロームのホーナー法1ステップ)。
pub fn mul2_xor_into(acc: &mut [u8], src: &[u8]) {
    mul_pow2_xor_into(acc, src, 1);
}

/// `acc = acc*4 ^ src`(Rシンドロームのホーナー法1ステップ)。
pub fn mul4_xor_into(acc: &mut [u8], src: &[u8]) {
    mul_pow2_xor_into(acc, src, 2);
}

/// `acc = acc * 2^times ^ src`。
fn mul_pow2_xor_into(acc: &mut [u8], src: &[u8], times: u32) {
    debug_assert_eq!(acc.len(), src.len());
    let n = acc.len().min(src.len());
    #[allow(unused_mut)]
    #[allow(unused_assignments)]
    let mut done = 0usize;
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        done = match simd_level() {
            SimdLevel::Avx512 => unsafe { x86::mul2_xor_into_avx512(&mut acc[..n], &src[..n], times) },
            SimdLevel::Avx2 => unsafe { x86::mul2_xor_into_avx2(&mut acc[..n], &src[..n], times) },
            SimdLevel::Sse2 => unsafe { x86::mul2_xor_into(&mut acc[..n], &src[..n], times) },
            SimdLevel::Scalar => 0,
        };
    }
    if done < n {
        match times {
            1 => mul2_xor_into_scalar(&mut acc[done..n], &src[done..n]),
            2 => mul4_xor_into_scalar(&mut acc[done..n], &src[done..n]),
            _ => {
                for i in done..n {
                    let mut v = acc[i];
                    for _ in 0..times {
                        v = mul2_byte(v);
                    }
                    acc[i] = v ^ src[i];
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 任意係数のGF(2^8)乗算(復旧〈デコード〉経路で使う)
// ---------------------------------------------------------------------------

/// 係数`c`に対する split-table(下位ニブル用・上位ニブル用の16要素表)を作る。
/// `c·b = c·(b & 0x0f) ^ c·(b & 0xf0)` という分解に基づく(ISA-Lと同じ手法)。
pub fn gf_split_tables(gf: &crate::galois::GaloisTables, c: u8) -> ([u8; 16], [u8; 16]) {
    let mut lo = [0u8; 16];
    let mut hi = [0u8; 16];
    for i in 0..16u8 {
        lo[i as usize] = gf.mul(c, i);
        hi[i as usize] = gf.mul(c, i << 4);
    }
    (lo, hi)
}

/// `acc ^= src * coeff`(GF(2^8)、任意係数)。
///
/// AVX-512BW → AVX2 → SSSE3 → スカラー(テーブル引き)の順に実行時ディスパッチする。
///
/// 【PCLMULQDQを採用しなかった理由(調査結果、正直な記録)】
/// PCLMULQDQ(キャリーレス乗算)は64ビット多項式同士の乗算命令であり、
/// 8バイトを1語に詰めたバイト並列のGF(2^8)乗算に使うと**隣接バイト間へ
/// 積のビットが溢れて混入する**(バイト境界で分離できない)。実際、
/// Linuxカーネル`lib/raid6/`もIntel ISA-Lも、RAID6のバイト単位GF演算には
/// PCLMULQDQではなく「シフト+0x1D XOR」(係数2固定の場合)と
/// 「PSHUFB split-table」(任意係数の場合)を使っている。PCLMULQDQが
/// 本領を発揮するのはCRC32/GF(2^128)(GHASH)といった**単一の広い多項式**を
/// 扱う場合であり、本リポジトリのチェックサムはSHA-256のため該当しない
/// (SHA-256側は`sha2`クレートが`cpufeatures`で**SHA-NIを実行時検出**して
/// 使うため、既にハードウェア高速化されている)。
pub fn gf_mul_xor_into(acc: &mut [u8], src: &[u8], coeff: u8, gf: &crate::galois::GaloisTables) {
    debug_assert_eq!(acc.len(), src.len());
    let n = acc.len().min(src.len());
    if coeff == 0 {
        return;
    }
    if coeff == 1 {
        xor_into(&mut acc[..n], &src[..n]);
        return;
    }
    let (lo, hi) = gf_split_tables(gf, coeff);
    #[allow(unused_assignments)]
    let mut done = 0usize;
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        done = match simd_level() {
            SimdLevel::Avx512 => unsafe { x86::gf_mul_xor_into_avx512(&mut acc[..n], &src[..n], &lo, &hi) },
            SimdLevel::Avx2 => unsafe { x86::gf_mul_xor_into_avx2(&mut acc[..n], &src[..n], &lo, &hi) },
            // SSSE3(PSHUFB)はSSE2より1世代新しいため、別途検出する。
            SimdLevel::Sse2 => {
                if std::is_x86_feature_detected!("ssse3") {
                    unsafe { x86::gf_mul_xor_into_ssse3(&mut acc[..n], &src[..n], &lo, &hi) }
                } else {
                    0
                }
            }
            SimdLevel::Scalar => 0,
        };
    }
    for i in done..n {
        acc[i] ^= gf.mul(src[i], coeff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galois::GaloisTables;

    #[test]
    fn mul2_byte_matches_galois_table() {
        let gf = GaloisTables::new();
        for b in 0..=255u8 {
            assert_eq!(mul2_byte(b), gf.mul(b, 2), "b={b:#04x}");
        }
    }

    #[test]
    fn mul2_u64_matches_bytewise() {
        for seed in 0..64u64 {
            let w = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let expect: Vec<u8> = w.to_le_bytes().iter().map(|&b| mul2_byte(b)).collect();
            assert_eq!(mul2_u64(w).to_le_bytes().to_vec(), expect);
        }
    }

    fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
        let mut s = seed | 1;
        (0..len)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                (s >> 24) as u8
            })
            .collect()
    }

    /// SIMD経路(この機ではAVX2)とスカラー経路が完全に一致することを、
    /// SIMD幅の倍数にならない端数長を含めて検証する。
    #[test]
    fn simd_paths_match_scalar_for_all_lengths() {
        for len in [0usize, 1, 7, 8, 15, 16, 31, 32, 63, 64, 65, 127, 129, 1000] {
            let src = pseudo_random(len, 0xABCD_1234 + len as u64);
            let base = pseudo_random(len, 0x5555_0000 + len as u64);

            // xor_into
            let mut a = base.clone();
            let mut b = base.clone();
            xor_into(&mut a, &src);
            xor_into_scalar(&mut b, &src);
            assert_eq!(a, b, "xor_into mismatch at len={len}");

            // mul2_xor_into
            let mut a = base.clone();
            let mut b = base.clone();
            mul2_xor_into(&mut a, &src);
            mul2_xor_into_scalar(&mut b, &src);
            assert_eq!(a, b, "mul2_xor_into mismatch at len={len}");

            // mul4_xor_into
            let mut a = base.clone();
            let mut b = base.clone();
            mul4_xor_into(&mut a, &src);
            mul4_xor_into_scalar(&mut b, &src);
            assert_eq!(a, b, "mul4_xor_into mismatch at len={len}");
        }
    }

    #[test]
    fn gf_mul_xor_into_matches_table_reference_for_all_coefficients() {
        let gf = GaloisTables::new();
        for &len in &[0usize, 1, 15, 16, 17, 31, 32, 63, 64, 65, 1000] {
            let src = pseudo_random(len, 0x77 + len as u64);
            let base = pseudo_random(len, 0x99 + len as u64);
            for coeff in 0..=255u8 {
                let mut a = base.clone();
                gf_mul_xor_into(&mut a, &src, coeff, &gf);
                let expect: Vec<u8> =
                    base.iter().zip(src.iter()).map(|(&b, &s)| b ^ gf.mul(s, coeff)).collect();
                assert_eq!(a, expect, "coeff={coeff} len={len}");
            }
        }
    }

    #[test]
    fn detected_level_is_reported() {
        // 実行環境で実際に選ばれた経路を記録(ベンチマークの根拠になる)。
        let level = simd_level();
        assert!(!level.as_str().is_empty());
    }
}
