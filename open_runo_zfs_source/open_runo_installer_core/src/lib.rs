//! open_runo_installer_core
//!
//! OpenRunoインストーラー(Tauriアプリ、`../open_runo_installer`)が使う
//! ディスク検出・構成助言(Copilot風アドバイザー)・zpool初期化プレビューの
//! ロジック層。意図的にTauriへ依存しない(モジュールdoc / Cargo.toml参照)。

pub mod copilot;
pub mod hardware;
pub mod zpool_wizard;
