//! GF(2^8)上の小規模正方行列に対するガウス消去法。
//!
//! RAID-Z3で3台のデータディスクが同時に欠損した場合、P/Q/Rパリティから
//! 元データを復元するには3元連立一次方程式(GF(2^8)上)を解く必要がある。
//! 本モジュールはその汎用的な行列逆変換を提供する。

use crate::galois::GaloisTables;

/// n x n の正方行列(GF(2^8)要素、行優先で格納)
#[derive(Debug, Clone)]
pub struct GfMatrix {
    n: usize,
    data: Vec<u8>,
}

impl GfMatrix {
    pub fn new(n: usize, data: Vec<u8>) -> Self {
        assert_eq!(data.len(), n * n, "行列サイズが一致しません");
        Self { n, data }
    }

    fn get(&self, r: usize, c: usize) -> u8 {
        self.data[r * self.n + c]
    }

    /// ガウス消去法による逆行列計算。行列が特異(逆行列を持たない)場合はNone。
    pub fn invert(&self, gf: &GaloisTables) -> Option<GfMatrix> {
        let n = self.n;
        let mut a = self.data.clone();
        let mut inv = vec![0u8; n * n];
        for i in 0..n {
            inv[i * n + i] = 1;
        }

        for col in 0..n {
            let pivot_row = (col..n).find(|&r| a[r * n + col] != 0)?;
            if pivot_row != col {
                for c in 0..n {
                    a.swap(col * n + c, pivot_row * n + c);
                    inv.swap(col * n + c, pivot_row * n + c);
                }
            }

            let pivot_val = a[col * n + col];
            let pivot_inv = gf.div(1, pivot_val);
            for c in 0..n {
                a[col * n + c] = gf.mul(a[col * n + c], pivot_inv);
                inv[col * n + c] = gf.mul(inv[col * n + c], pivot_inv);
            }

            for r in 0..n {
                if r == col {
                    continue;
                }
                let factor = a[r * n + col];
                if factor == 0 {
                    continue;
                }
                for c in 0..n {
                    a[r * n + c] ^= gf.mul(factor, a[col * n + c]);
                    inv[r * n + c] ^= gf.mul(factor, inv[col * n + c]);
                }
            }
        }

        Some(GfMatrix { n, data: inv })
    }

    /// 行列とベクトルの積(GF(2^8)上)
    pub fn mul_vec(&self, gf: &GaloisTables, v: &[u8]) -> Vec<u8> {
        assert_eq!(v.len(), self.n);
        (0..self.n)
            .map(|r| (0..self.n).fold(0u8, |acc, c| acc ^ gf.mul(self.get(r, c), v[c])))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_matrix_inverts_to_itself() {
        let gf = GaloisTables::new();
        let identity = GfMatrix::new(3, vec![1, 0, 0, 0, 1, 0, 0, 0, 1]);
        let inv = identity.invert(&gf).unwrap();
        assert_eq!(inv.data, identity.data);
    }

    #[test]
    fn inverting_then_multiplying_recovers_original_vector() {
        let gf = GaloisTables::new();
        // RAID-Z3で実際に使う形の係数行列(1, 2^i, 4^i の3行)
        let g = |i: u32| gf.pow2(i);
        let h = |i: u32| gf.pow2(2 * i);
        let m = GfMatrix::new(
            3,
            vec![1, 1, 1, g(0), g(1), g(2), h(0), h(1), h(2)],
        );
        let inv = m.invert(&gf).expect("この行列は特異ではないはず");

        let original = vec![0x12u8, 0x34, 0x56];
        let encoded = m.mul_vec(&gf, &original);
        let decoded = inv.mul_vec(&gf, &encoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn singular_matrix_returns_none() {
        let gf = GaloisTables::new();
        // 2行が同一なので特異行列
        let m = GfMatrix::new(3, vec![1, 2, 3, 1, 2, 3, 4, 5, 6]);
        assert!(m.invert(&gf).is_none());
    }
}
