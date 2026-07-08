fn main() {
    // `winfsp-backend`機能が有効な場合のみ、WinFspのDLLへのdelayload
    // リンクフラグを発行する(featureが無効な環境ではWinFsp SDK/DLLが
    // 無くてもビルドできるようにするため)。
    if std::env::var("CARGO_FEATURE_WINFSP_BACKEND").is_ok() {
        winfsp::build::winfsp_link_delayload();
    }
}
