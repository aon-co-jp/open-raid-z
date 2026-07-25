//! 一時退避先(Email/Googleドライブ等)へ送るジャーナルセグメントの
//! 圧縮/展開処理を、CPU/GPU/NPU(DirectX経由)で切り替え可能にする
//! ハードウェアアクセラレータ抽象化。
//!
//! `open-web-server`(`open_web_server_wire::accel`)が既に採用している
//! `AccelBackend`(CPU/GPU/NPU/HardwareAccelerator切り替え)の設計パターンを
//! 踏襲する(このエコシステム内で確立済みのパターンを再利用し、
//! 車輪の再発明を避ける方針)。
//!
//! **正直な開示(2026-07時点)**: 圧縮アルゴリズム自体のGPU/NPU実装
//! (DirectX経由のDEFLATE/zstd等)は、本エコシステム内に実績が無い
//! (`open-cuda`にはChaCha20の暗号化GPUカーネル実績はあるが、圧縮
//! アルゴリズムのGPU/NPU実装は無い——2026-07-25時点で日英Web検索した
//! `nvCOMP`(NVIDIA製、CUDA専用でDirectX/クロスベンダーではない)以外に、
//! 本エコシステムがすぐ流用できるクロスベンダーのGPU/NPU圧縮クレートは
//! 見つからなかった)。そのため、本モジュールは**CPU実装のみ実際に圧縮/
//! 展開を行い**、GPU/NPUが要求された場合は安全にCPU実装へフォールバック
//! した上で`tracing::warn!`で可視化する、という設計にとどめる
//! (過剰実装回避の既存方針、および`open-web-server`が採用した
//! 「フォールバックを伴う先取りAPI」パターンと同じ)。
use std::io::{Read, Write};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;

use crate::error::{BridgeError, BridgeResult};

/// 圧縮/展開に使うハードウェアバックエンドの希望。
/// `open-web-server`の`AccelBackend`と同じ考え方(希望するバックエンドを
/// 指定し、未実装/利用不可なら安全にCPUへフォールバックする)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccelBackend {
    /// 常にCPU(miniz_oxideバックエンドのDEFLATE)で圧縮/展開する。既定値。
    #[default]
    Cpu,
    /// DirectX(DirectML/DirectCompute)経由のGPUアクセラレータを希望する。
    /// **2026-07時点で圧縮アルゴリズムのGPU実装は未実装のため、常にCPUへ
    /// フォールバックする**(下記`compress`/`decompress`参照)。
    Gpu,
    /// NPU(DirectX経由のニューラル処理ユニット)を希望する。
    /// **2026-07時点で未実装のため、常にCPUへフォールバックする。**
    Npu,
}

impl AccelBackend {
    /// このバックエンドで実際に圧縮/展開処理を行うと、CPU実装への
    /// フォールバックが発生するかどうか(診断・ログ用)。
    pub fn is_fallback_to_cpu(self) -> bool {
        !matches!(self, AccelBackend::Cpu)
    }
}

/// 選択したバックエンドでジャーナルセグメントを圧縮する。
/// GPU/NPUが指定された場合は、警告ログを出した上でCPU実装にフォールバック
/// する(実際にクラッシュ・データ破損することはない、という安全性を優先)。
pub fn compress(backend: AccelBackend, data: &[u8]) -> BridgeResult<Vec<u8>> {
    if backend.is_fallback_to_cpu() {
        tracing::warn!(
            ?backend,
            "圧縮のGPU/NPU(DirectX)アクセラレーションは2026-07時点で未実装のため、CPU実装にフォールバックします"
        );
    }
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .map_err(|e| BridgeError::OffsiteBackupFailed(format!("圧縮に失敗しました: {e}")))?;
    encoder
        .finish()
        .map_err(|e| BridgeError::OffsiteBackupFailed(format!("圧縮の完了処理に失敗しました: {e}")))
}

/// 選択したバックエンドで圧縮済みデータを展開する。
pub fn decompress(backend: AccelBackend, compressed: &[u8]) -> BridgeResult<Vec<u8>> {
    if backend.is_fallback_to_cpu() {
        tracing::warn!(
            ?backend,
            "展開のGPU/NPU(DirectX)アクセラレーションは2026-07時点で未実装のため、CPU実装にフォールバックします"
        );
    }
    let mut decoder = GzDecoder::new(compressed);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| BridgeError::OffsiteBackupFailed(format!("展開に失敗しました: {e}")))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_roundtrip_restores_original_bytes() {
        let original = b"open-raid-z disconnect journal segment payload".repeat(64);
        let compressed = compress(AccelBackend::Cpu, &original).unwrap();
        assert!(compressed.len() < original.len(), "繰り返しデータは圧縮で縮むはず");
        let restored = decompress(AccelBackend::Cpu, &compressed).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn gpu_and_npu_requests_fall_back_to_cpu_and_still_roundtrip() {
        let original = b"gpu/npu requested but must safely fall back to cpu".to_vec();
        for backend in [AccelBackend::Gpu, AccelBackend::Npu] {
            assert!(backend.is_fallback_to_cpu());
            let compressed = compress(backend, &original).unwrap();
            let restored = decompress(backend, &compressed).unwrap();
            assert_eq!(restored, original);
        }
    }
}
