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

fn find_glslc() -> String {
    if Command::new("glslc").arg("--version").output().is_ok() {
        return "glslc".to_string();
    }
    panic!(
        "glslc(GLSL to SPIR-Vコンパイラ)が見つかりません。PATHへ追加するか、\
         Vulkan SDKに含まれるglslcをインストールしてください。"
    );
}

fn compile_glsl_shader(glslc: &str, src: &str, out_dir: &std::path::Path, out_name: &str) {
    let out_path = out_dir.join(out_name);
    let status = Command::new(glslc)
        .arg("-fshader-stage=compute")
        .arg("-o")
        .arg(&out_path)
        .arg(src)
        .status()
        .unwrap_or_else(|e| panic!("glslcの起動に失敗しました({glslc}): {e}"));
    if !status.success() {
        panic!("GLSLシェーダのコンパイルに失敗しました: {src}");
    }
    println!("cargo:rerun-if-changed={src}");
}

fn main() {
    // `gpu` feature が無効な場合(CPUフォールバックのみ使う場合)は、
    // dxc(DirectX Shader Compiler)が無い環境でもビルドできるよう
    // シェーダのコンパイル自体をスキップする。
    if std::env::var("CARGO_FEATURE_GPU").is_ok() {
        let dxc = find_dxc();
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

        compile_shader(&dxc, "shaders/raidz_parity.hlsl", &out_dir, "raidz_parity.cso");
        compile_shader(&dxc, "shaders/raidz2_parity.hlsl", &out_dir, "raidz2_parity.cso");
        compile_shader(&dxc, "shaders/raidz3_parity.hlsl", &out_dir, "raidz3_parity.cso");

        // NPU専用ディスパッチ経路(現状はGPU版と同一アルゴリズムだが、
        // `AccelKind::Npu`用に別のシェーダバイトコードとして分離しておく。
        // 理由は各shaders/raidnpu_*.hlslの先頭コメント参照)。
        compile_shader(&dxc, "shaders/raidnpu_parity.hlsl", &out_dir, "raidnpu_parity.cso");
        compile_shader(&dxc, "shaders/raidnpu_z2_parity.hlsl", &out_dir, "raidnpu_z2_parity.cso");
        compile_shader(&dxc, "shaders/raidnpu_z3_parity.hlsl", &out_dir, "raidnpu_z3_parity.cso");
    }

    // `vulkan` feature: Windows以外(Linux/Mac/Android等)向けのGPU/NPU
    // アクセラレーション経路。dxc/HLSLとは独立にビルド時コンパイルする
    // (`gpu`と両方無効なCPU専用ビルドではこちらもスキップする)。
    if std::env::var("CARGO_FEATURE_VULKAN").is_ok() {
        let glslc = find_glslc();
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        compile_glsl_shader(&glslc, "shaders/raidz_parity.comp", &out_dir, "raidz_parity.spv");
    }
}
