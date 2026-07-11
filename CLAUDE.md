# 技術スタック・開発ルール(open-raid-z)

このリポジトリ、および関連プロジェクト(`open-runo`/`open-web-server`/
`aruaru-db`)で開発・保守を行う際は、以下を基本方針とする。作業ドライブは
`F:\open-runo`(E:ドライブは2026-07-10に消失、以後Fが実体)。

## 方針転換(2026-07-10、最終確定)

ユーザー指示により以下へ転換・確定。**Tauri・Poem・WunderGraph Cosmo(有料版
含む)を外部パッケージ/ライブラリとして直接依存させることはしない**。ただし
各ツールが提供する**機能・API形状・体験には互換性を保ち**、Rust標準ライブラリ
+ tokio/hyper で自前実装して置き換える(依存だけを断ち、機能面の互換性は
維持する)。**`poem-cosmo-tauri` と `open-runo` は2リポジトリを同時並行で
開発する**(2026-07-10、再確定)。どちらもTauri/Poemを含まない構成。
実装(例: crates/open-runo-routerのPoem→tokio/hyper移行)はpoem-cosmo-tauri
側で先行させ、動作確認できたファイルをopen-runoへミラーする運用とする。

## poem-cosmo-tauri と open-runo の違い(2026-07-11、ユーザー確認済み)

両リポジトリは共通コアを持つが、**スコープが異なる別々のリポジトリ
プロジェクト**であり、統合・一本化すべき対象ではない。

- **共通コア**: WunderGraph Cosmo 有料版の機能(GraphQL Federation・
  VersionlessAPI・SSO/SCIM/RBAC・Persisted Queries・キャッシュ制御・
  細粒度レートリミット等)を、Cosmo自体には依存せず Rust + tokio/hyper で
  自前再実装した OSS 版。これは両リポジトリで共通。
- **poem-cosmo-tauri はさらに範囲が広い**: 共通コアに加えて、Poem(Rust
  Web フレームワーク)と Tauri(デスクトップフロントエンドフレームワーク)
  の**全機能を、AI駆動開発によって一から自作・再現する**ことを目指す
  ——単にAPI形状・体験の互換性を保つだけでなく、両フレームワークの
  機能そのものを自前実装として再現する、という上乗せの目標を持つ。
  open-runo にはこの上乗せ目標はない。
- 両リポジトリは共通コアを持つが**全く違うリポジトリのプロジェクト**であり、
  「ミラー」作業は必ずしも「同一スコープの複製」を意味しない——
  poem-cosmo-tauri 固有の Poem/Tauri 機能再現タスクが open-runo に
  存在理由なく持ち込まれることもあれば、逆に open-runo が独自に先行実装し
  poem-cosmo-tauri へ逆ミラーするケースもある(例:
  `open-runo-feature-flags`、2026-07-11)。新しいタスクを検討する際は、
  `docs/cosmo-parity.md` 4a節のギャップ一覧に加えて、poem-cosmo-tauri
  側では「これは Poem または Tauri の何を再現するか」という軸でも
  評価すること。

**補足・言い換え(2026-07-11、ユーザーによる再定義)**: `open-runo`
(通称 "cosmo")は、(1) Tauri互換のフロントエンド体験、(2) **REST API
不要**(VersionlessAPI/GraphQL Federationで代替し、エンドポイントの
バージョン乱立を根本解決)、(3) **契約不要**(WunderGraph Cosmo 有料版
であれば必要な商用ライセンス契約なしに同等機能をOSSとして提供)、
(4) **独自AI搭載のWeb高速化機能**(自己学習型HTMLキャッシュ予測=
`CachePredictor`によるコールドスタート予測・コスト学習・適応TTL等、
外部LLM/有料契約は一切不要な純Rust統計学習)、の4点を備えたミドルウェア/
フレームワークであり、これが両リポジトリ共通のコア。**poem-cosmo-tauri
は、この open-runo(cosmo)コアを中心に据えつつ、Rust の Poem フレーム
ワークを「連携」するのではなく、**Poemフレームワークとの完全互換を保ち
ながら、Poemフレームワーク自体を一から自作・再現する**ことが本リポジトリ
固有の役割(2026-07-11、ユーザーによる訂正——単なる外部フレームワークとの
連携・統合ではなく、そのフレームワーク自体の完全な自前再実装を指す)。
実装は`poem`パッケージへの直接依存を持たないが、Poemのルーティング/
ハンドラ/ミドルウェア/エクストラクタ等のAPI形状・挙動を余さず再現する
ことを目指す(現状の到達度・残ギャップは`docs/poem-parity.md`が正)。
新機能・改善タスクを検討する際は、この4点(Tauri互換UI・REST API不要・
契約不要・自作AI高速化)と、Poemフレームワークの完全互換再現の網羅性、
の両軸で完成度・利便性・使いやすさ・実用性を継続的に高めることを目標と
する。

## フロントエンド(2026-07-10、方針更新)

- Tauriパッケージには直接依存しない。ただしTauriのデスクトップUI体験・
  `invoke()`的なコマンド呼び出しインターフェースとは互換性を保つ。
- **HTML5/CSS3・TypeScript・Bootstrap・Node.jsのスタックは廃止**。
  Rustをメイン言語としてフロントエンドとバックエンドを統合し、
  **WebAssembly (WASM)** に置き換える(コンパイル対象はRust →
  `wasm32-unknown-unknown`)。DOM操作・`invoke()`相当の呼び出しは
  Rust製WASMモジュール側で行い、TypeScript/Node.jsのビルドチェーンには
  依存しない。https://webassembly.org/ | https://rustwasm.github.io/

## バックエンド・コア

- **Rust**(メイン言語、標準ライブラリ中心): https://www.rust-lang.org/ja/ | https://github.com/rust-lang/rust
- **tokio** + **hyper**(Webフレームワークなしで直接HTTPサーバを自前実装):
  https://tokio.rs/ | https://docs.rs/hyper/latest/hyper/
- Poemパッケージには依存しないが、Poemのルーティング/ハンドラAPI形状とは
  互換性のあるインターフェースを維持しながらtokio/hyper直接実装へ移行する。

## API設計思想(参考・概念のみ)

- **VersionLess API**という考え方を参考にする(WunderGraphのブログ/podcast参照)。
- **WunderGraph Cosmo**: パッケージとしては直接依存させない。GraphQL
  Federation / VersionlessAPI というAPI形状・コンセプトのみ参考にし、
  Rust標準+tokio/hyperで互換性を保ちつつ自前実装する。
  https://github.com/wundergraph/cosmo

## 関連プロジェクト

- **poem-cosmo-tauri**(poem-cosmo-tauriとopen-runoを同時並行開発。実装の
  先行地点。Pure Rust + tokio/hyper直接実装): https://github.com/aon-co-jp/poem-cosmo-tauri
- **open-runo**(poem-cosmo-tauriと同時並行開発。2026-07-10付けで開発再開):
  https://github.com/aon-co-jp/open-runo
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
- **無人自動開発(確認不要・自動デバッグ)のタイミングでは、20〜30分おきの
  スケジュール実行待ちにせず、1パス内でできる限り連続して作業を進める**
  こと(ユーザー指示、2026-07-10)。小さく検証可能な単位(1ハンドラ/1関数
  ごとに `cargo test` → commit → push)を保ちながらも、次の増分に進む前に
  バックグラウンド待機で止まらない。スケジュールされたウェイクアップは
  「セッションが終わっても翌朝まで継続する」ためのフォールバックであり、
  同一パス内で作業を続けられる間は使わない。
