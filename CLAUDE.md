# 技術スタック・開発ルール(open-raid-z)

このリポジトリ、および関連プロジェクト(`open-runo`/`open-web-server`/
`aruaru-db`)で開発・保守を行う際は、以下を基本方針とする。作業ドライブは
`F:\open-runo`(E:ドライブは2026-07-10に消失、以後Fが実体)。

## 方針転換(2026-07-10)

ユーザー指示により以下へ転換。**Tauri・Poem・WunderGraph Cosmo(有料版含む)は
いずれも不要**。`poem-cosmo-tauri`は廃止し、`open-runo`1リポジトリに統合する。

## フロントエンド

- 専用フレームワークなし。必要になった場合はHTML5/CSS3 + 必要最低限のTypeScriptで
  薄いUIを都度用意する方針(Tauriは使わない)。

## バックエンド・コア

- **Rust**(メイン言語、標準ライブラリ中心): https://www.rust-lang.org/ja/ | https://github.com/rust-lang/rust
- **tokio** + **hyper**(Webフレームワークなしで直接HTTPサーバを自前実装):
  https://tokio.rs/ | https://docs.rs/hyper/latest/hyper/
- Poemを含む既存Webフレームワークは今後使用しない。既存のPoem依存コードは
  順次tokio/hyper直接実装へ移行する。

## API設計思想(参考・概念のみ)

- **VersionLess API**という考え方を参考にする(WunderGraphのブログ/podcast参照)。
- **WunderGraph Cosmo**: あくまで**参考・着想元としてのみ**参照する。
  **有料版を含め実装には絶対に使用しない**。https://github.com/wundergraph/cosmo

## 関連プロジェクト

- **open-runo**(唯一の正本リポジトリ。Pure Rust + tokio/hyper直接実装で
  ゼロから再実装する方針。WEBサイト開発用。poem-cosmo-tauriはここに統合済み):
  https://github.com/aon-co-jp/open-runo
- **poem-cosmo-tauri**(2026-07-10付けで廃止・open-runoへ統合。今後更新しない):
  https://github.com/aon-co-jp/poem-cosmo-tauri
- **open-web-server**: https://github.com/aon-co-jp/open-web-server
- **aruaru-db**: https://github.com/aon-co-jp/aruaru-db
- **open-raid-z**(このリポジトリ): https://github.com/aon-co-jp/open-raid-z
- **rs-to-readme**: https://github.com/aon-co-jp/rs-to-readme

## 運用ルール

- **開発中はこの`CLAUDE.md`を、コード変更のコミット/pushと必ず一緒に
  push する**(内容を更新した場合はもちろん、変更が無い場合も他の変更と
  一緒にコミット対象へ含めておくこと)。
- 実装で迷った場合や、API仕様の詳細確認が必要な場合は、学習データからの
  推測より公式ドキュメント(上記URL)を優先して参照する。
- 作業ドライブ(現在`F:\open-runo`)が変わった場合は、この節を更新し、
  CHAT_HANDOFF.md にも変更の経緯を記録すること。
