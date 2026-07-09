fn main() {
    // このクレート名(`open_runo_installer_core`)に"installer"を含むため、
    // マニフェスト無しでビルドすると`cargo test`が生成するテストバイナリが
    // WindowsのInstaller Detection Technologyに引っかかり、
    // `ERROR_ELEVATION_REQUIRED`でプロセス起動自体に失敗する
    // (詳細はCargo.tomlの`embed-manifest`コメント参照)。
    // asInvokerのマニフェストを明示的に埋め込むことでこのヒューリスティックを
    // 無効化する。
    //
    // 注意: `embed_manifest()`/`embed_manifest_file()`(このクレートの
    // トップレベルAPI)は`cargo:rustc-link-arg-bins`を固定で発行するため、
    // 本クレートのように`[[bin]]`ターゲットが存在しないライブラリクレートで
    // 呼ぶと「does not have a bin target」でビルド自体が失敗する。
    // 対象は`cargo test`が生成するテストハーネス実行ファイルなので、
    // 代わりに`cargo:rustc-link-arg-tests`を自前で発行する。
    #[cfg(windows)]
    {
        use embed_manifest::new_manifest;

        let out_dir = std::env::var_os("OUT_DIR")
            .map(std::path::PathBuf::from)
            .expect("OUT_DIR is not set");
        let manifest_path = out_dir.join("manifest.xml");
        std::fs::write(&manifest_path, new_manifest("OpenRuno.InstallerCore").to_string())
            .expect("failed to write manifest.xml");

        let target = std::env::var("TARGET").unwrap_or_default();
        if target.contains("msvc") {
            println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
            println!(
                "cargo:rustc-link-arg=/MANIFESTINPUT:{}",
                manifest_path.display()
            );
            println!("cargo:rustc-link-arg=/MANIFESTUAC:NO");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
