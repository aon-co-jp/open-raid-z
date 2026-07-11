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

## poem-cosmo-tauri の構成・位置付け(2026-07-11、ユーザーによる最終定義)

poem-cosmo-tauri は、以下の3要素をすべて**外部パッケージに依存せず自前で
一から開発・再現**し、それらの連携をスムーズに行うことで、WEBサイト/
WEBアプリ開発を効率的に行えるようにするための**フレームワーク/ミドル
ウェア**である。3要素いずれも「連携」ではなく、そのフレームワーク自体の
完全互換な自前再実装を指す点に注意(2026-07-11、ユーザーによる訂正)。

1. **cosmo部分(= open-runoと共通のコア)**: WunderGraph Cosmo 有料版
   (Launch/Scale/Enterprise)の機能を、Cosmo自体には依存せず Rust +
   tokio/hyper で自前再実装した OSS 版。具体的には (a) Tauri互換の
   フロントエンド体験、(b) **REST API不要**(VersionlessAPI/GraphQL
   Federationで代替しエンドポイントのバージョン乱立を根本解決)、
   (c) **契約不要**(Cosmo有料版であれば必要な商用ライセンス契約なしに
   同等機能をOSSとして提供)、(d) **独自AI搭載のWeb高速化機能**
   (自己学習型HTMLキャッシュ予測=`CachePredictor`によるコールドスタート
   予測・コスト学習・適応TTL等、外部LLM/有料契約は一切不要な純Rust
   統計学習)を含む。open-runo とはこのcosmo部分が共通。
2. **poem部分(= バックエンド)**: Rust の Poem フレームワークの**全機能を
   完全互換で一から自作・再現**したバックエンド。`poem`パッケージへの
   直接依存を持たないが、Poemのルーティング/ハンドラ/ミドルウェア/
   エクストラクタ等のAPI形状・挙動を余さず再現することを目指す
   (現状の到達度・残ギャップは`docs/poem-parity.md`が正)。
3. **tauri部分(= フロントエンド)**: デスクトップフロントエンドフレーム
   ワーク Tauri の**全機能を完全互換で一から自作・再現**したフロント
   エンド(`tauri`パッケージへの直接依存は持たない。現状は Rust→WASM で
   実装、到達度・残ギャップは`docs/tauri-parity.md`が正)。

**この3つ(Tauri再現フロントエンド + open-runo/cosmoコア + Poem再現
バックエンド)がスムーズに連携し合うこと自体が poem-cosmo-tauri の価値**。
フロントエンド開発・バックエンド開発・Web中心的な開発(GraphQL
Federation・VersionlessAPI等)の間の連携を円滑にし、効率よく
WEBサイト/WEBアプリを開発できるようにするためのフレームワーク/
ミドルウェアという位置付け。**open-runo にはこの3要素統合という上乗せ
目標はなく、cosmo部分(共通コア)が中心**。新機能・改善タスクを検討する
際は、この3要素それぞれの完成度(cosmoの4特性・Poem完全再現の網羅性・
Tauri完全再現の網羅性)と、3者の連携の滑らかさ、の両軸で完成度・利便性・
使いやすさ・実用性を継続的に高めることを目標とする。

## open-web-server 拡張要件(2026-07-11、ユーザー指示——目標アーキテクチャ)

`open-web-server` リポジトリで、`poem-cosmo-tauri`(または `open-runo`)・
PostgreSQL・`aruaru-db`・`open-raid-z` を組み合わせ、3Dオンラインゲームの
課金アイテム・金融データ・証券データ等がネットワーク上で紛失しないための
以下を実装する(詳細・進捗は `open-web-server/CLAUDE.md` の同名節が正):
(1) VersionLessAPI(エンドポイント)とGit管理(`aruaru-db`のコミット単位
履歴)のハイブリッドなバージョン管理、(2) `open-raid-z`をディスク冗長化
基盤としてこのデータ永続化層と組み合わせる、(3) **通信層の四重化**
(TCP-IP・UDP-IPに加え、QUIC/MPQUIC・MPTCPまたはSCTPを合わせた4方式、
2026-07-11にネット調査の上で三層三重から改訂)、(4) **DB書き込みの四重化**
(PostgreSQL・aruaru-dbに加え、マルチリージョン同期レプリケーション・
独立監査トランザクションログを合わせた4系統、同じく改訂)。段階的に検証
可能な単位に分割して実装し、各段階を実バイナリ・実ネットワーク通信で
検証すること。詳細・出典は `open-web-server/CLAUDE.md` の同名節を参照。

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
- **各無人開発パスの最後には、必ず以下を実行すること**(ユーザー指示、
  2026-07-11): (1) 世界10ヶ国語のREADME(`README-<言語>.md`、日本語・
  英語・中国語簡体字・韓国語・スペイン語・フランス語・ドイツ語・
  イタリア語・ロシア語・アラビア語)を、そのパスで行った実装内容に
  合わせて更新する、(2) 他プロジェクトへのお引越し可能ファイル
  (`PORTING.md`)を同様に最新の実装内容に合わせて更新する、(3) この
  開発環境ルール(`CLAUDE.md`)のHANDOFF節・現状節を更新して保存する、
  (4) 上記(1)〜(3)を含む全ての変更をコミットしてpushする。この4点を
  終えてから、そのパスを終了する。

## 現状(このリポジトリ固有、2026-07-11時点)

- ルート`README.md`・10ヶ国語版`README-<言語>.md`(日本語・英語・中国語
  簡体字・韓国語・スペイン語・フランス語・ドイツ語・イタリア語・
  ロシア語・アラビア語、姉妹リポジトリと同じ命名規則でルート直下に配置)・
  `PORTING.md`を新規作成した(このリポジトリはこれまでルート
  `README.md`1本のみで、`PORTING.md`は未作成だった。旧`README/`
  フォルダの10言語版(UK/US English・Ukraine・Iran(Persian)を含む異なる
  言語セット)はそのまま残置、新規ファイルが姉妹リポジトリ標準の現行版)。
- 実測ファクト: `open_raid_z_core`/`zfs_accel_hlsl`/
  `open_runo_installer_core`の3クレート構成、`cargo test
  --no-default-features`(WinFsp SDK/dxc/Windows SDK不要のCPU
  フォールバック)で合計163テストpassed・failed 0
  (101 + 32 + 30)。`default`feature(実マウント+GPU高速化)はWindows
  実機+WinFsp SDK+dxcが必要なため今回は未計測。
