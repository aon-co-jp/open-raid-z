//! shaders/*.hlsl を事前にDXIL(コンパイル済みシェーダバイトコード)へ変換する。
//!
//! `dxc`(DirectX Shader Compiler。Windows SDK または Vulkan SDK に同梱)を
//! 実行時ではなくビルド時に呼び出すことで、実行バイナリはランタイムHLSL
//! コンパイル(D3DCompile系API)に依存せずに済む。

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn find_dxc() -> String {
    if Command::new("dxc").arg("-help").output().is_ok() {
        return "dxc".to_string();
    }
    panic!(
        "dxc(DirectX Shader Compiler)が見つかりません。PATHへ追加するか、\
         Windows SDKまたはVulkan SDKに含まれるdxc.exeをインストールしてください。"
    );
}

fn compile_shader(dxc: &str, src: &str, out_dir: &std::path::Path, out_name: &str) {
    let out_path = out_dir.join(out_name);
    let status = Command::new(dxc)
        .args(["-T", "cs_6_0", "-E", "CSMain", "-Fo"])
        .arg(&out_path)
        .arg(src)
        .status()
        .unwrap_or_else(|e| panic!("dxcの起動に失敗しました({dxc}): {e}"));
    if !status.success() {
        panic!("HLSLシェーダのコンパイルに失敗しました: {src}");
    }
    println!("cargo:rerun-if-changed={src}");
}

fn main() {
    // `gpu` feature が無効な場合(CPUフォールバックのみ使う場合)は、
    // dxc(DirectX Shader Compiler)が無い環境でもビルドできるよう
    // シェーダのコンパイル自体をスキップする。
    if std::env::var("CARGO_FEATURE_GPU").is_err() {
        return;
    }

    let dxc = find_dxc();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    compile_shader(&dxc, "shaders/raidz_parity.hlsl", &out_dir, "raidz_parity.cso");
    compile_shader(&dxc, "shaders/raidz2_parity.hlsl", &out_dir, "raidz2_parity.cso");
}
