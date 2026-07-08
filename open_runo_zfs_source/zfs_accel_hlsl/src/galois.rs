//! RAID-Z2/Z3のReed-Solomonパリティ計算に用いるGF(2^8)(ガロア体)演算。
//!
//! OpenZFSの実装(`vdev_raidz`)と同じ既約多項式
//! x^8 + x^4 + x^3 + x^2 + 1 (= 0x11d)、生成元2を用いる。
//! exp/logテーブルによる乗算・除算・べき乗を提供する。

const PRIMITIVE_POLY: u16 = 0x11d;

pub struct GaloisTables {
    // 512要素にしているのは exp[log(a)+log(b)] の際に mod 255 を省くため。
    exp: [u8; 512],
    log: [u8; 256],
}

impl GaloisTables {
    pub fn new() -> Self {
        let mut exp = [0u8; 512];
        let mut log = [0u8; 256];

        let mut x: u16 = 1;
        for i in 0..255usize {
            exp[i] = x as u8;
            log[x as usize] = i as u8;
            x <<= 1;
            if x & 0x100 != 0 {
                x ^= PRIMITIVE_POLY;
            }
        }
        for i in 255..512 {
            exp[i] = exp[i - 255];
        }

        Self { exp, log }
    }

    /// GF(2^8)上の乗算
    pub fn mul(&self, a: u8, b: u8) -> u8 {
        if a == 0 || b == 0 {
            return 0;
        }
        let sum = self.log[a as usize] as usize + self.log[b as usize] as usize;
        self.exp[sum]
    }

    /// GF(2^8)上の除算(a / b)。bは0であってはならない。
    pub fn div(&self, a: u8, b: u8) -> u8 {
        assert!(b != 0, "GF(2^8)でゼロ除算はできません");
        if a == 0 {
            return 0;
        }
        let diff = 255 + self.log[a as usize] as i32 - self.log[b as usize] as i32;
        self.exp[(diff as usize) % 255]
    }

    /// GF(2^8)上の 2^power (RAID-Z2/Z3のQ/R係数生成に使用)
    pub fn pow2(&self, power: u32) -> u8 {
        let l = (self.log[2] as u32 as usize * power as usize) % 255;
        self.exp[l]
    }
}

impl Default for GaloisTables {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_by_zero_is_zero() {
        let gf = GaloisTables::new();
        assert_eq!(gf.mul(0, 200), 0);
        assert_eq!(gf.mul(200, 0), 0);
    }

    #[test]
    fn mul_by_one_is_identity() {
        let gf = GaloisTables::new();
        for a in 1..=255u8 {
            assert_eq!(gf.mul(a, 1), a);
        }
    }

    /// exp/logテーブルに依存しない、シフト&XORによる直接のGF(2^8)乗算参照実装。
    /// テーブル実装(`GaloisTables::mul`)が既約多項式0x11dの定義通りに
    /// 動作しているかを独立に検証するために用いる。
    fn carryless_mul_reference(mut a: u8, mut b: u8) -> u8 {
        let mut result: u8 = 0;
        for _ in 0..8 {
            if b & 1 != 0 {
                result ^= a;
            }
            let high_bit_set = a & 0x80 != 0;
            a <<= 1;
            if high_bit_set {
                a ^= (PRIMITIVE_POLY & 0xff) as u8;
            }
            b >>= 1;
        }
        result
    }

    #[test]
    fn mul_matches_independent_shift_and_xor_reference() {
        let gf = GaloisTables::new();
        for a in [0x01, 0x02, 0x53, 0x7A, 0xCA, 0xFF] {
            for b in [0x01, 0x02, 0x53, 0x7A, 0xCA, 0xFF] {
                assert_eq!(
                    gf.mul(a, b),
                    carryless_mul_reference(a, b),
                    "mismatch for a={a:#04x}, b={b:#04x}"
                );
            }
        }
    }

    #[test]
    fn div_is_inverse_of_mul() {
        let gf = GaloisTables::new();
        for a in 1..=255u8 {
            for b in 1..=255u8 {
                let product = gf.mul(a, b);
                assert_eq!(gf.div(product, b), a);
            }
        }
    }

    #[test]
    fn pow2_matches_repeated_multiplication() {
        let gf = GaloisTables::new();
        let mut acc = 1u8;
        for power in 0..16u32 {
            assert_eq!(gf.pow2(power), acc);
            acc = gf.mul(acc, 2);
        }
    }
}
