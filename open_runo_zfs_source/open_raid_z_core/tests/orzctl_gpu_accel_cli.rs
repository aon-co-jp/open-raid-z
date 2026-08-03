//! `orzctl`(実運用CLI)が実際にGPU/NPUアクセラレータの検出・接続経路を
//! 通ることを、コンパイル済みバイナリをサブプロセスとして実行して検証する
//! (2026-08-01追加)。
//!
//! **背景**: `RaidZVdev`/`vdev.rs`にはP-parity(XOR)・Q-parity
//! (Reed-Solomon)ともGPU実装・実機検証済みの`with_accelerator`経路が
//! 以前から存在していたが、`orzctl`の各サブコマンド(`create`/`status`/
//! `mount`)はこれまで素の`RaidZVdev::new(...)`しか呼んでおらず、実際の
//! CLI経由では一度もGPUアクセラレータが使われていなかった(死んだコード
//! だった)。このテストは、ユニットテストレベルの`RaidZVdev`検証
//! (`src/vdev.rs`の`accel_tests`)とは別に、**実際に配布されるバイナリを
//! 実際にサブプロセスとして起動し**、標準エラー出力に検出結果の
//! ログ(ハードウェアアクセラレータ使用/CPUフォールバックのいずれか)が
//! 実際に出力されることを確認する——モックではなく実バイナリでの検証。

use std::process::Command;

fn orzctl_bin() -> &'static str {
    env!("CARGO_BIN_EXE_orzctl")
}

fn scratch_disk(dir: &std::path::Path, name: &str, size_bytes: u64) -> std::path::PathBuf {
    let path = dir.join(name);
    open_raid_z_core::block_device::FileBackedDevice::create_fixed_size(&path, size_bytes).unwrap();
    path
}

#[test]
fn orzctl_create_and_status_report_accelerator_selection_on_stderr() {
    let dir = std::env::temp_dir().join(format!("orzctl-gpu-accel-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // Z2(RAID6相当)は3データ+2パリティの最小構成。
    let disks: Vec<std::path::PathBuf> =
        ["d0", "d1", "d2", "p", "q"].iter().map(|n| scratch_disk(&dir, n, 4096 * 16)).collect();
    let disk_args: Vec<String> = disks.iter().map(|p| p.to_string_lossy().to_string()).collect();

    let create_output = Command::new(orzctl_bin())
        .arg("create")
        .args(["--level", "z2", "--chunk-size", "4096", "--stripes", "8", "--dataset", "test-dataset"])
        .args(&disk_args)
        .output()
        .expect("failed to spawn orzctl create");

    let create_stderr = String::from_utf8_lossy(&create_output.stderr);
    assert!(
        create_output.status.success(),
        "orzctl create failed: stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&create_output.stdout),
        create_stderr
    );
    // 実行環境にGPU/NPUがある場合と無い場合のどちらでも、build_vdevの
    // 検出結果ログのいずれか一方が実際に出力されていることを確認する
    // (「実装したが誰にも呼ばれていない」状態ではないことの直接証拠)。
    assert!(
        create_stderr.contains("ハードウェアアクセラレータを使用します") || create_stderr.contains("CPUで行います"),
        "expected accelerator-selection log on stderr, got: {create_stderr:?}"
    );

    let status_output = Command::new(orzctl_bin())
        .arg("status")
        .args(["--level", "z2", "--chunk-size", "4096", "--stripes", "8"])
        .args(&disk_args)
        .output()
        .expect("failed to spawn orzctl status");
    let status_stderr = String::from_utf8_lossy(&status_output.stderr);
    let status_stdout = String::from_utf8_lossy(&status_output.stdout);
    assert!(status_output.status.success(), "orzctl status failed: stdout={status_stdout:?} stderr={status_stderr:?}");
    assert!(
        status_stderr.contains("ハードウェアアクセラレータを使用します") || status_stderr.contains("CPUで行います"),
        "expected accelerator-selection log on stderr, got: {status_stderr:?}"
    );
    // `status`は保存済みプールを開き直しての問い合わせのため、
    // createで作成したデータセットが実際に見えることも確認する
    // (アクセラレータ接続によってプールの永続化/読み出し自体が
    // 壊れていないことの裏付け)。
    assert!(status_stdout.contains("test-dataset"), "expected dataset name in status output, got: {status_stdout:?}");

    std::fs::remove_dir_all(&dir).ok();
}
