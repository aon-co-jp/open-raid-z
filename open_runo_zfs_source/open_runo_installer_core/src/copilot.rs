//! 「契約不要の独自学習機能のCopilotのようなAI」の第一段階。
//!
//! 【設計方針】外部API契約(クラウドLLM等)には依存しない。まずは
//! ディスク構成・アクセラレータ有無から助言を生成するルールベース実装
//! ([`HeuristicAdvisor`])で始め、[`Advisor`]トレイトの後ろに隠すことで、
//! 将来ローカルLLM推論(DirectML経由でNPU/GPUを使う等)へ実装だけを
//! 差し替えられるようにする(呼び出し側・UIは`Advisor`トレイトにしか
//! 依存しないため無変更で済む)。
//!
//! 現時点のスコープは「インストーラーのディスク/RAID構成への助言」だが、
//! `AdviceContext`/`Advice`は将来的にプールの使用率・scrub結果なども
//! 取り込めるよう、汎用的な形で設計している(「ファイル操作全般」への
//! 拡張はこの土台の上に段階的に積む想定)。

use crate::hardware::{AcceleratorInfo, DiskInfo};
use serde::Serialize;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// ユーザーが既に導入済みかもしれないローカルLLMサーバーの検知結果。
///
/// 【スコープ】今回は「動いているかどうか」を検知する骨組みのみ実装する。
/// 実際にプロンプトを送って応答を解析する連携は、どのAPI形式
/// (Ollama独自 / OpenAI互換 等)に対応するかを詰めた上で後続で実装する。
#[derive(Debug, Clone, Serialize)]
pub struct LocalLlmInfo {
    pub detected: bool,
    /// 検知できた場合の候補名(例: "Ollama")。検知方法はポート疎通のみの
    /// 簡易判定なので、実際にそのソフトウェアである保証はない参考情報。
    pub candidate: Option<String>,
    pub endpoint: Option<String>,
}

/// よく使われるローカルLLMサーバーのデフォルトポートへ、ごく短いタイムアウトで
/// TCP接続を試みるだけの軽量な検知(実際のAPI疎通・応答解析は行わない)。
/// UIをブロックしないよう、1候補あたり200msでタイムアウトする。
pub fn detect_local_llm() -> LocalLlmInfo {
    const CANDIDATES: &[(&str, &str)] = &[
        ("Ollama", "127.0.0.1:11434"),
        ("LM Studio (OpenAI互換)", "127.0.0.1:1234"),
    ];

    for (name, addr) in CANDIDATES {
        if let Some(info) = try_connect(name, addr) {
            return info;
        }
    }

    LocalLlmInfo {
        detected: false,
        candidate: None,
        endpoint: None,
    }
}

fn try_connect(name: &str, addr: &str) -> Option<LocalLlmInfo> {
    let socket_addr: SocketAddr = addr.to_socket_addrs().ok()?.next()?;
    TcpStream::connect_timeout(&socket_addr, Duration::from_millis(200)).ok()?;
    Some(LocalLlmInfo {
        detected: true,
        candidate: Some(name.to_string()),
        endpoint: Some(addr.to_string()),
    })
}

#[derive(Debug, Clone, Serialize)]
pub enum AdviceSeverity {
    Info,
    Suggestion,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
pub struct Advice {
    pub severity: AdviceSeverity,
    pub title: String,
    pub detail: String,
}

/// 助言生成に必要な状況(コンテキスト)。将来、プール使用率や
/// scrubレポートなどのフィールドを追加していく想定。
#[derive(Debug, Clone)]
pub struct AdviceContext {
    pub disks: Vec<DiskInfo>,
    pub accelerator: AcceleratorInfo,
    pub cpu_cores: usize,
    pub local_llm: LocalLlmInfo,
}

impl AdviceContext {
    /// 現在のマシンを自動スキャンしてコンテキストを構築する
    /// (ディスク一覧・アクセラレータ・CPUコア数・ローカルLLMの検知)。
    pub fn scan_current_machine() -> Self {
        Self {
            disks: crate::hardware::list_physical_disks(),
            accelerator: crate::hardware::detect_accelerator(),
            cpu_cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
            local_llm: detect_local_llm(),
        }
    }
}

/// 助言生成の抽象化。`HeuristicAdvisor`(ルールベース)と、将来の
/// ローカルLLM推論実装の両方がこのトレイトを実装する想定。
pub trait Advisor {
    fn advise(&self, context: &AdviceContext) -> Vec<Advice>;
}

/// ルールベースの助言生成器(第一段階の実装)。
pub struct HeuristicAdvisor;

const GIB: u64 = 1024 * 1024 * 1024;

impl Advisor for HeuristicAdvisor {
    fn advise(&self, context: &AdviceContext) -> Vec<Advice> {
        let mut advice = Vec::new();
        let n = context.disks.len();

        match n {
            0 => {
                advice.push(Advice {
                    severity: AdviceSeverity::Warning,
                    title: "ディスクが検出されていません".to_string(),
                    detail: "管理者権限で実行されているか確認してください(物理ディスクの列挙には管理者権限が必要です)。".to_string(),
                });
                return advice;
            }
            1 => {
                advice.push(Advice {
                    severity: AdviceSeverity::Warning,
                    title: "冗長構成にできません".to_string(),
                    detail: "ディスクが1台のみのため、RAID0(ストライプのみ・冗長性なし)以外は構成できません。可能であれば2台目以降を追加してください。".to_string(),
                });
            }
            2 => {
                advice.push(Advice {
                    severity: AdviceSeverity::Suggestion,
                    title: "RAID1(ミラー)を推奨".to_string(),
                    detail: "2台構成では、片方が故障してももう片方から復旧できるRAID1(ミラー)が最も安全です。".to_string(),
                });
            }
            3 => {
                advice.push(Advice {
                    severity: AdviceSeverity::Suggestion,
                    title: "RAID5またはRAID-Z1相当を推奨".to_string(),
                    detail: "3台構成では、1台分の容量をパリティに使うRAID5で、1台までの故障に耐えつつ実効容量を確保できます。".to_string(),
                });
            }
            _ => {
                if n % 2 == 0 {
                    advice.push(Advice {
                        severity: AdviceSeverity::Suggestion,
                        title: "RAID10(性能重視)またはRAID6/Z2(容量重視)".to_string(),
                        detail: format!(
                            "{n}台の偶数構成です。読み書き性能を優先するならRAID10(ミラーのストライプ)、実効容量を優先するなら2台分のパリティで2台同時故障に耐えるRAID6/RAID-Z2が向いています。"
                        ),
                    });
                } else {
                    advice.push(Advice {
                        severity: AdviceSeverity::Suggestion,
                        title: "RAID6/Z2(または3重パリティのZ3)を推奨".to_string(),
                        detail: format!(
                            "{n}台の奇数構成です。2台同時故障に耐えるRAID6/RAID-Z2、より高い信頼性が必要なら3台同時故障に耐えるRAID-Z3を検討してください。"
                        ),
                    });
                }
            }
        }

        if n >= 2 {
            let sizes: Vec<u64> = context.disks.iter().map(|d| d.size_bytes).collect();
            let min = *sizes.iter().min().unwrap();
            let max = *sizes.iter().max().unwrap();
            // 最小ディスクの2倍を超える差がある場合、無駄になる容量が大きいと判断する。
            if max > min.saturating_mul(2) && min > 0 {
                let wasted_per_disk_gib = (max - min) / GIB;
                advice.push(Advice {
                    severity: AdviceSeverity::Info,
                    title: "ディスク容量にばらつきがあります".to_string(),
                    detail: format!(
                        "最小{}GiB・最大{}GiBと差が大きいため、RAIDでは最小ディスクの容量が全体の上限になります(大きいディスク側で最大約{wasted_per_disk_gib}GiB/台が使われません)。",
                        min / GIB,
                        max / GIB
                    ),
                });
            }
        }

        match context.accelerator.kind.as_str() {
            "Npu" => advice.push(Advice {
                severity: AdviceSeverity::Info,
                title: "NPUアクセラレーションが利用可能です".to_string(),
                detail: format!("検出されたNPU({})でパリティ計算をオフロードできます。", context.accelerator.description),
            }),
            "Gpu" => advice.push(Advice {
                severity: AdviceSeverity::Info,
                title: "GPUアクセラレーションが利用可能です".to_string(),
                detail: format!("検出されたGPU({})でパリティ計算をオフロードできます。", context.accelerator.description),
            }),
            _ => {
                advice.push(Advice {
                    severity: AdviceSeverity::Info,
                    title: "CPUフォールバックで動作します".to_string(),
                    detail: "NPU/GPUが検出できなかったため、パリティ計算はCPUで行われます(動作に問題はありませんが、大容量プールでは処理がやや遅くなる場合があります)。".to_string(),
                });
                if context.cpu_cores <= 2 {
                    advice.push(Advice {
                        severity: AdviceSeverity::Suggestion,
                        title: "CPUコア数が少ないため軽量なRAIDレベルを推奨".to_string(),
                        detail: format!(
                            "検出されたCPUコア数は{}です。NPU/GPUが無くコア数も少ない環境では、パリティ計算が軽いRAID0/RAID1(パリティ計算不要)を優先すると快適です。",
                            context.cpu_cores
                        ),
                    });
                }
            }
        }

        if context.local_llm.detected {
            let candidate = context.local_llm.candidate.as_deref().unwrap_or("不明なローカルLLM");
            advice.push(Advice {
                severity: AdviceSeverity::Info,
                title: "ローカルLLMを検知しました".to_string(),
                detail: format!(
                    "{candidate}が{}で動作しているようです。現時点では検知のみで、実際の問い合わせ連携は今後の対応予定です(外部契約不要のまま、より柔軟な提案に拡張できます)。",
                    context.local_llm.endpoint.as_deref().unwrap_or("不明なアドレス")
                ),
            });
        }

        advice
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(sizes: &[u64], accel_kind: &str) -> AdviceContext {
        ctx_with(sizes, accel_kind, 8, false)
    }

    fn ctx_with(sizes: &[u64], accel_kind: &str, cpu_cores: usize, local_llm_detected: bool) -> AdviceContext {
        AdviceContext {
            disks: sizes
                .iter()
                .enumerate()
                .map(|(i, &size_bytes)| DiskInfo {
                    path: format!("\\\\.\\PhysicalDrive{i}"),
                    index: i as u32,
                    size_bytes,
                })
                .collect(),
            accelerator: AcceleratorInfo {
                kind: accel_kind.to_string(),
                description: "test".to_string(),
            },
            cpu_cores,
            local_llm: LocalLlmInfo {
                detected: local_llm_detected,
                candidate: local_llm_detected.then(|| "Ollama".to_string()),
                endpoint: local_llm_detected.then(|| "127.0.0.1:11434".to_string()),
            },
        }
    }

    #[test]
    fn zero_disks_warns_and_returns_early() {
        let advice = HeuristicAdvisor.advise(&ctx(&[], "CpuFallback"));
        assert_eq!(advice.len(), 1);
        assert!(matches!(advice[0].severity, AdviceSeverity::Warning));
    }

    #[test]
    fn single_disk_warns_about_no_redundancy() {
        let advice = HeuristicAdvisor.advise(&ctx(&[100 * GIB], "CpuFallback"));
        assert!(advice.iter().any(|a| matches!(a.severity, AdviceSeverity::Warning)));
    }

    #[test]
    fn two_disks_suggests_mirror() {
        let advice = HeuristicAdvisor.advise(&ctx(&[100 * GIB, 100 * GIB], "Gpu"));
        assert!(advice.iter().any(|a| a.title.contains("RAID1")));
    }

    #[test]
    fn four_disks_suggests_raid10_or_raid6() {
        let advice = HeuristicAdvisor.advise(&ctx(&[100 * GIB; 4], "Gpu"));
        assert!(advice.iter().any(|a| a.title.contains("RAID10")));
    }

    #[test]
    fn five_disks_suggests_raid6_or_z3() {
        let advice = HeuristicAdvisor.advise(&ctx(&[100 * GIB; 5], "Npu"));
        assert!(advice.iter().any(|a| a.title.contains("RAID6")));
    }

    #[test]
    fn mismatched_disk_sizes_trigger_wasted_capacity_warning() {
        let advice = HeuristicAdvisor.advise(&ctx(&[500 * GIB, 100 * GIB], "Gpu"));
        assert!(advice.iter().any(|a| a.title.contains("ばらつき")));
    }

    #[test]
    fn similar_disk_sizes_do_not_trigger_wasted_capacity_warning() {
        let advice = HeuristicAdvisor.advise(&ctx(&[100 * GIB, 110 * GIB], "Gpu"));
        assert!(!advice.iter().any(|a| a.title.contains("ばらつき")));
    }

    #[test]
    fn npu_detected_mentions_npu() {
        let advice = HeuristicAdvisor.advise(&ctx(&[100 * GIB, 100 * GIB], "Npu"));
        assert!(advice.iter().any(|a| a.title.contains("NPU")));
    }

    #[test]
    fn no_accelerator_mentions_cpu_fallback() {
        let advice = HeuristicAdvisor.advise(&ctx(&[100 * GIB, 100 * GIB], "CpuFallback"));
        assert!(advice.iter().any(|a| a.title.contains("CPU")));
    }

    #[test]
    fn low_core_count_without_accelerator_suggests_lightweight_raid() {
        let advice = HeuristicAdvisor.advise(&ctx_with(&[100 * GIB, 100 * GIB], "CpuFallback", 2, false));
        assert!(advice.iter().any(|a| a.title.contains("軽量")));
    }

    #[test]
    fn high_core_count_without_accelerator_does_not_suggest_lightweight_raid() {
        let advice = HeuristicAdvisor.advise(&ctx_with(&[100 * GIB, 100 * GIB], "CpuFallback", 16, false));
        assert!(!advice.iter().any(|a| a.title.contains("軽量")));
    }

    #[test]
    fn detected_local_llm_is_mentioned_in_advice() {
        let advice = HeuristicAdvisor.advise(&ctx_with(&[100 * GIB, 100 * GIB], "Gpu", 8, true));
        assert!(advice.iter().any(|a| a.title.contains("ローカルLLM")));
    }

    #[test]
    fn no_local_llm_is_not_mentioned() {
        let advice = HeuristicAdvisor.advise(&ctx_with(&[100 * GIB, 100 * GIB], "Gpu", 8, false));
        assert!(!advice.iter().any(|a| a.title.contains("ローカルLLM")));
    }

    #[test]
    fn detect_local_llm_completes_quickly_without_hanging() {
        let start = std::time::Instant::now();
        let _ = detect_local_llm();
        // 候補2件 x 200msタイムアウトなので、どちらも繋がらない場合でも
        // 1秒未満で完了するはず(UIをブロックしない軽量実装であることの検証)。
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn scan_current_machine_completes_and_reports_at_least_one_cpu_core() {
        let ctx = AdviceContext::scan_current_machine();
        assert!(ctx.cpu_cores >= 1);
    }
}
