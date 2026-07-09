//! GF(2^8)の「係数倍してXOR畳み込む」パリティ計算(P/Q/R)を、GF(2)上の
//! ビット行列とビットベクトルの整数内積(mod 2)として再定式化する。
//!
//! 【なぜこの再定式化が要るか】
//! `raidz23_parity::compute_pq`/`compute_pqr`(galois.rsのmul/pow2テーブル
//! 参照)も、`shaders/raidz2_parity.hlsl`等(シフト+XORの反復)も、本質的には
//! 「1バイトずつ」処理するスカラーALU向けの演算であり、NPUの行列演算
//! (GEMM/畳み込み)ユニットを活かせない。
//!
//! GF(2^8)上で「定数cを掛ける」操作は、GF(2^8)がGF(2)上のベクトル空間である
//! ことから、実はGF(2)上の8x8線形写像(行列M_c)そのものである。複数ディスク
//! ぶんをブロック結合すれば、「W(8×8N行列)×X(8N×1のビットベクトル)を
//! (符号なし)整数として内積計算し、各出力ビットをmod 2で戻す」という単純な
//! 整数GEMMに帰着する。これはDirectMLのGEMM/ビット演算オペレータ経由で
//! NPUのMACアレイに載せられる形(生のHLSL Compute Shaderでは原理的に到達
//! できない領域)。
//!
//! 本ファイルはこの再定式化がCPU上で既存のGaloisTables参照実装と完全に
//! 一致することを検証する(実際のGEMM/DirectMLディスパッチへの接続は別途)。

use crate::galois::GaloisTables;

/// GF(2^8)の定数`c`倍を表す8x8のGF(2)線形写像行列。
///
/// `rows[j]`のbit kが「出力ビットjが入力ビットkに依存するか(1)否か(0)」を
/// 表す(=行列の第j行をu8へビットパックしたもの)。
#[derive(Debug, Clone, Copy)]
pub struct GfBitMatrix {
    rows: [u8; 8],
}

impl GfBitMatrix {
    /// GF(2^8)の定数`c`に対応する8x8のGF(2)線形写像行列を構築する。
    ///
    /// 線形写像は基底ベクトルの像だけで完全に決まる: 基底ベクトル`1 << k`
    /// (=値`2^k`のバイト)を`c`倍した結果(`gf.mul`で得る)が、写像行列の
    /// 第k列そのものになる。
    pub fn for_constant(gf: &GaloisTables, c: u8) -> Self {
        let columns: [u8; 8] = std::array::from_fn(|k| gf.mul(1u8 << k, c));
        let mut rows = [0u8; 8];
        for (k, &col) in columns.iter().enumerate() {
            for j in 0..8u32 {
                if (col >> j) & 1 == 1 {
                    rows[j as usize] |= 1 << k;
                }
            }
        }
        Self { rows }
    }

    /// この行列をバイト`x`(8bitのビットベクトルとして)に適用する。
    ///
    /// 出力ビットj = XOR_k(rows[j]のbit k AND xのbit k)
    ///             = popcount(rows[j] AND x) mod 2
    /// という「AND-XOR型の内積」であり、GF(2)上の行列積そのもの。
    /// この結果が`gf.mul(x, c)`(GaloisTables経由の参照実装)と一致することが
    /// 本モジュールの中心的な主張であり、テストで直接検証する。
    pub fn apply(&self, x: u8) -> u8 {
        let mut out = 0u8;
        for (j, &row) in self.rows.iter().enumerate() {
            let bit = (row & x).count_ones() & 1;
            out |= (bit as u8) << j;
        }
        out
    }
}

/// 「ビット行列 + popcount mod 2」経路によるP/Q/R計算のCPU参照実装。
///
/// [`raidz23_parity::compute_pqr`](crate::raidz23_parity::compute_pqr)と
/// 完全に一致するはず(テストで検証)。実際のGEMM化(複数ディスクを1回の
/// 行列積へブロック結合する最適化)はこの関数のような「ディスクごとに
/// 独立に適用してXOR畳み込む」形と数学的に同値であり、正しさはここで
/// 個々の行列(`GfBitMatrix::for_constant`)の正しさを確認すれば十分。
pub fn compute_pqr_via_bitmatrix(
    data_disks: &[&[u8]],
    gf: &GaloisTables,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let stripe_len = data_disks.first().map(|s| s.len()).unwrap_or(0);
    let mut p = vec![0u8; stripe_len];
    let mut q = vec![0u8; stripe_len];
    let mut r = vec![0u8; stripe_len];

    for (i, disk) in data_disks.iter().enumerate() {
        debug_assert_eq!(disk.len(), stripe_len, "全ディスクは同じストライプ長である必要があります");
        let m_p = GfBitMatrix::for_constant(gf, 1); // P用: 係数1(恒等写像)
        let m_q = GfBitMatrix::for_constant(gf, gf.pow2(i as u32));
        let m_r = GfBitMatrix::for_constant(gf, gf.pow2(2 * i as u32));

        for (byte_idx, &b) in disk.iter().enumerate() {
            p[byte_idx] ^= m_p.apply(b);
            q[byte_idx] ^= m_q.apply(b);
            r[byte_idx] ^= m_r.apply(b);
        }
    }

    (p, q, r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmatrix_apply_matches_gf_mul_for_every_byte_value_and_several_constants() {
        let gf = GaloisTables::new();
        // Q用(2^0..2^7)・R用(4^0..4^7 = 2^0,2^2,..,2^14)双方の係数域を網羅。
        let constants: Vec<u8> = (0..8).map(|i| gf.pow2(i)).chain((0..8).map(|i| gf.pow2(2 * i))).collect();

        for &c in &constants {
            let matrix = GfBitMatrix::for_constant(&gf, c);
            for x in 0u32..256 {
                let x = x as u8;
                assert_eq!(
                    matrix.apply(x),
                    gf.mul(x, c),
                    "c={c:#04x}, x={x:#04x}で不一致"
                );
            }
        }
    }

    #[test]
    fn bitmatrix_for_constant_one_is_identity() {
        let gf = GaloisTables::new();
        let matrix = GfBitMatrix::for_constant(&gf, 1);
        for x in 0u32..256 {
            assert_eq!(matrix.apply(x as u8), x as u8);
        }
    }

    #[test]
    fn compute_pqr_via_bitmatrix_matches_galois_table_reference() {
        let gf = GaloisTables::new();
        let d0: Vec<u8> = vec![0x01, 0x9F, 0x10, 0x00];
        let d1: Vec<u8> = vec![0x02, 0x8E, 0x20, 0xFF];
        let d2: Vec<u8> = vec![0x03, 0x7D, 0x30, 0x55];
        let d3: Vec<u8> = vec![0x04, 0x6C, 0x40, 0xAA];
        let d4: Vec<u8> = vec![0x05, 0x5B, 0x50, 0x33];
        let refs: Vec<&[u8]> = vec![&d0, &d1, &d2, &d3, &d4];

        let expected = crate::raidz23_parity::compute_pqr(&refs, &gf);
        let actual = compute_pqr_via_bitmatrix(&refs, &gf);

        assert_eq!(actual, expected);
    }

    #[test]
    fn compute_pqr_via_bitmatrix_matches_reference_for_many_disk_counts() {
        let gf = GaloisTables::new();
        for num_disks in 1..=12usize {
            let disks: Vec<Vec<u8>> = (0..num_disks)
                .map(|i| (0..8u8).map(|b| b.wrapping_mul(7).wrapping_add(i as u8 * 13)).collect())
                .collect();
            let refs: Vec<&[u8]> = disks.iter().map(|d| d.as_slice()).collect();

            let expected = crate::raidz23_parity::compute_pqr(&refs, &gf);
            let actual = compute_pqr_via_bitmatrix(&refs, &gf);

            assert_eq!(actual, expected, "num_disks={num_disks}で不一致");
        }
    }
}
