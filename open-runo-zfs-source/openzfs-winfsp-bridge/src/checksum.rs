//! ZFSの「全書き込みデータへチェックサムを付与し、読み込み時に検証する」
//! データ整合性機能を模した層。
//!
//! 本物のZFSはブロックポインタ木(間接ブロック)にチェックサムを埋め込み、
//! ディスク上に永続化するが、本スキャフォールディングにはまだそのような
//! オンディスクメタデータ構造が無いため、[`crate::vdev::RaidZVdev`]が
//! メモリ上のテーブルとしてチェックサムを保持・検証する簡易実装とする。

use sha2::{Digest, Sha256};

pub type Checksum = [u8; 32];

/// データのチェックサム(SHA-256)を計算する。
pub fn compute_checksum(data: &[u8]) -> Checksum {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_data_produces_same_checksum() {
        assert_eq!(compute_checksum(b"hello"), compute_checksum(b"hello"));
    }

    #[test]
    fn different_data_produces_different_checksum() {
        assert_ne!(compute_checksum(b"hello"), compute_checksum(b"hellp"));
    }

    #[test]
    fn checksum_detects_single_bit_flip() {
        let original = vec![0xAAu8; 64];
        let mut corrupted = original.clone();
        corrupted[30] ^= 0x01; // 1ビットだけ反転(ビットロットの典型例)
        assert_ne!(compute_checksum(&original), compute_checksum(&corrupted));
    }
}
