fn main() {
    // `winfsp_backend`機能が有効な場合のみ、WinFspのDLLへのdelayload
    // リンクフラグを発行する(featureが無効な環境ではWinFsp SDK/DLLが
    // 無くてもビルドできるようにするため)。
    //
    // 注意: `winfsp`クレートは[build-dependencies]でも`optional = true`と
    // している(Cargo.toml参照)。そのためfeature無効時は`winfsp`クレート
    // 自体がこのbuild.rsから見えなくなるので、実行時の環境変数チェック
    // (`std::env::var(...).is_ok()`)ではなく`#[cfg(feature = ...)]`で
    // コンパイル時に呼び出し自体を除去する必要がある。
    #[cfg(feature = "winfsp_backend")]
    winfsp::build::winfsp_link_delayload();
}
