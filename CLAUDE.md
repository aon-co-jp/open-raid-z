# 開発方針・開発環境ルール(全リポジトリ共通ヘッダー、2026-07-15追記)

## 1. 比較的新しい言語・フレームワークの参照資料一覧

Rust自体は歴史があるが、本エコシステムが採用する **Poem** のような
比較的新しい・情報量がまだ少なめのWebフレームワークは、Python+FastAPIの
ような広く普及した組み合わせと比べ、AIモデルの学習データ・公開されている
実装例/Q&A/ブログ記事の絶対量が少ない傾向がある。そのため、AI駆動開発
(Claude等)がこれらを扱う際、実装の勘違い・API名の記憶違い・古いバージョン
のAPIでの実装(本プロジェクトで実際に複数回発生した既知の失敗パターン)に
よる**手戻り・いたちごっこ**が起きやすい。

対策として、AIが作業を始める際は、以下から**そのタスクに必要な部分だけ**を
先に参照してから実装に着手すること(全部読む必要はない。関連しそうな1〜2件を
拾い読みする程度で十分)。これにより歩留まりが上がり、AI駆動開発の手戻りが
減ることが期待される。

| 技術 | 公式ドキュメント | GitHub | 補足・ブログ等 |
|---|---|---|---|
| Rust言語本体 | https://doc.rust-lang.org/book/ | https://github.com/rust-lang/rust | https://blog.rust-lang.org/ |
| Poem(Webフレームワーク) | https://docs.rs/poem/latest/poem/ | https://github.com/poem-web/poem | https://crates.io/crates/poem |
| Tokio(非同期ランタイム) | https://tokio.rs/tokio/tutorial | https://github.com/tokio-rs/tokio | https://tokio.rs/blog |
| async-graphql | https://async-graphql.github.io/async-graphql/en/index.html | https://github.com/async-graphql/async-graphql | https://crates.io/crates/async-graphql |
| Tauri | https://tauri.app/ | https://github.com/tauri-apps/tauri | https://tauri.app/blog/ |
| wasm-bindgen / web-sys | https://rustwasm.github.io/wasm-bindgen/ | https://github.com/rustwasm/wasm-bindgen | https://rustwasm.github.io/docs/book/ |
| SurrealDB | https://surrealdb.com/docs | https://github.com/surrealdb/surrealdb | https://surrealdb.com/blog |
| sqlx | https://docs.rs/sqlx/latest/sqlx/ | https://github.com/launchbadge/sqlx | |
| WinFsp | https://winfsp.dev/ | https://github.com/winfsp/winfsp | |
| DirectX 12 / DirectML | https://learn.microsoft.com/en-us/windows/win32/direct3d12/directx-12-programming-guide | https://github.com/microsoft/DirectML | https://devblogs.microsoft.com/directx/ |
| WebAssembly(wasm32全般) | https://webassembly.org/ | https://github.com/WebAssembly | https://rustwasm.github.io/docs/book/ |

> ⚠️ **重要な注意(正直な開示)**: このURL一覧は、Web検索ツールを持たない
> セッションで学習データに基づき記載したものであり、**実在性・現在の
> 有効性・記載内容の正確性を検証していない**。特にAI(Claude含む)が
> このリストを鵜呑みにして実装や回答の根拠にすることは避け、
> **開発者自身が実際にアクセスして確認する**か、Web検索が使える
> セッションで一次情報を再確認してから利用すること。リンク切れ・
> リダイレクト・バージョン変更(特にAPIの破壊的変更)の可能性を
> 常に考慮する。新しい技術を追加する場合はこの表に追記していくこと。

## 2. AI駆動開発ツールに関する所感(2026-07-15、ユーザー所感として記録)

2026-07-15時点、ChatGPT等の汎用AIチャットは小規模なWebアプリ程度までは
開発できるものの、システムがある程度複雑・大規模になると出戻りが大きくなり、
一度に扱えるプログラムサイズにもすぐ限界が来る傾向がある。

Claude Code / Claude Desktopは、ローカルドライブを直接指定してファイルの
読み書きができ、GitHubリポジトリの読み出し(本プロジェクトのような
複数リポジトリにまたがるエコシステム)にも対応できるため、本プロジェクトの
ような規模のAI駆動開発には適していると考えられる。新しくAI駆動開発環境を
セットアップする際の選択肢として推奨する。

---

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

## open-web-server 拡張要件(2026-07-13、要約を統合・整理)

`open-web-server` は、3Dオンラインゲームのアイテム課金やクレジットカード
決済のような金融データを扱う、24時間365日ノンストップ運用の
ミッションクリティカルな Web サーバー。**4層防御通信による高セキュリティ
と高速性の両立**、および**ZFS互換(`open-raid-z`)とACID互換
(PostgreSQL)のハイブリッド技術**を核として、`poem-cosmo-tauri`
(または `open-runo`)・PostgreSQL・`aruaru-db`・`open-raid-z` と連携する
多層防御アーキテクチャにより、ネットワーク瞬断・プロセス再起動・
リトライが起きても「二重課金」も「データ消失」も起こさない設計を
実現する(詳細・進捗は `open-web-server/CLAUDE.md` の同名節が正)。
目標アーキテクチャは以下4項目: (1) VersionLessAPI(エンドポイント)と
Git管理(`aruaru-db`のコミット単位履歴)のハイブリッドなバージョン管理
(書き込み側は実装済み、読み出し側=commit_id指定クエリは未着手)、
(2) `open-raid-z`をディスク冗長化基盤としてこのデータ永続化層と組み合わせ、
Raftコミット確定と連動したZFS互換スナップショット連携(実装済み)、
(3) **通信層の四重化**(TCP-IP・UDP-IPに加え、QUIC・MPTCP/SCTPを合わせた
4方式——2026-07-13時点でQUICは`quinn`ベースで実装済み、MPTCP/SCTPは
Windows開発環境にネイティブ実装が無いため`aggligator`によるユーザー空間
代替で実装済み[本物のカーネル実装ではない点を明記・再調査中])、
(4) **DB書き込みの四重化**(PostgreSQL・aruaru-db・マルチリージョン同期
レプリケーション・独立監査トランザクションログの4系統、全て実装済み・
PostgreSQLのみ実接続検証待ち)。詳細・出典は `open-web-server/CLAUDE.md`
の同名節を参照。

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

### パフォーマンス・並行処理方針(2026-07-13、ユーザー指示)

システム全体として、4層4重の通信・DB冗長化によるハイセキュリティを
保ちつつ、ハイパースレッディング/マルチコア/マルチスレッドを活かした
高速性を両立させる。**非同期(tokio、マルチスレッドランタイム)を基本**
とし、必要な場面(CPU負荷の高い計算・厳密な順序保証が必要な処理等)での
み同期処理を用いる。着眼点: (1) `#[tokio::main]`のランタイムflavorが
current_threadに固定されていないか、(2) async関数内でのブロッキング
I/O・CPU負荷処理は`tokio::task::spawn_blocking`へ退避、(3) CPU律速な
処理は`rayon`等でのデータ並列化を検討、(4) セキュリティクリティカルな
ホットパスの排他ロックがボトルネックになっていないか、を確認する。

## API設計思想(参考・概念のみ)

- **VersionLess API**という考え方を参考にする(WunderGraphのブログ/podcast参照)。
- **WunderGraph Cosmo**: パッケージとしては直接依存させない。GraphQL
  Federation / VersionlessAPI というAPI形状・コンセプトのみ参考にし、
  Rust標準+tokio/hyperで互換性を保ちつつ自前実装する。
  https://github.com/wundergraph/cosmo

## 契約不要の独自AI(open-cuda × aruaru-llm SET、2026-07-18追記)

エコシステム内のどのプロジェクトであれ、**外部AI事業者との有償契約・
APIキー(OpenAI等)を必要としない、自前完結のAI機能**が必要になった場合は、
**`open-cuda` + `aruaru-llm` のSET構成を標準として使うこと**
(ユーザー指示、2026-07-18)。

- **`open-cuda`**(https://github.com/aon-co-jp/open-cuda): クロスベンダー
  GPUランタイム。CPUバックエンド(rayon)・実Vulkanのvector_add/matmulまで
  実装済み。**2026-07-19更新**: LLM推論に不可欠なGEMM/Attention
  (`opencuda-blas`クレート)のうちCPU経路は実装済みになった——
  `sgemm`の`GemmPath::CpuNaive`(alpha/beta付き`C=alpha*A·B+beta*C`、
  `opencuda_core::GpuDevice::launch_kernel`経由の実カーネル)と、
  `scaled_dot_product_attention`(QKᵀ・softmax・P·Vを実計算する素朴な
  非タイル化attention。**真のFlash Attention(タイル化+オンライン
  softmax)ではない**ため誠実に別名にした、`flash_attention`は
  引き続きスタブ)。単体テスト7件で検証済み。**残る未実装**: GPU
  ベンダー別経路(cuBLAS/rocBLAS/oneMKL/Vulkan汎用)、INT4/INT8量子化
  (`quantize_int4`)、真のFlash Attention(タイル化)——これらが次の
  増分。
- **`aruaru-llm`**(https://github.com/aon-co-jp/aruaru-llm): エコシステム
  共通の「AIチャットコマース」応答HTTPサービス。`open-cuda`の
  `opencuda-core`/`opencuda-cpu`をpath依存し、リクエストごとに実際に
  `GpuDevice::launch_kernel`を呼び出す(bag-of-wordsベクトルの要素積
  カーネル)。**正直な開示**: v0.1.0時点では本物のニューラル推論ではなく、
  固定語彙へのbag-of-wordsドット積による単純なルールベース意図分類。
  `engine`フィールドで実装方式を常に正直に返す設計。

**適用方針**: 新規/既存プロジェクトで「AIによる判定・応答・分類」機能が
必要になった場合、まず外部LLM API(OpenAI等、契約・費用・データ送信先の
懸念を伴う)に頼るのではなく、この自前SET構成で実現できないか検討する
ことを既定とする。実現できない場合(高度な自然言語理解が必須等)は、
その理由をドキュメントに明記した上で外部API利用を検討してよい
(例: `audiocafe-tokyo-rust`のcron自動更新のうち技術ランキング/AI学習
コメント処理は、既存PHP実装がOpenAI API依存のため今回は移植対象外と
した実例がある——将来的に`aruaru-llm`側の能力が向上すれば移行を検討)。

## 「分身の術」構成の対象拡大(2026-07-18追記、ユーザー指示)

`open-web-server`が採用している「分身の術」(1つの共有バックエンド
インスタンスに、ドメインごとの個別インストール無しで複数テナントを
動的登録する設計、`open-easy-web/server/src/appserver_registration.rs`
参照)を、以下のリポジトリにも適用する:

- **`open-cuda`**・**`aruaru-llm`**・**`RPoem`**(poem-cosmo-tauri)・
  **`RCosmo`**(open-runo)・**`open-raid-z`**・**`aruaru-db`**

**要件**:
1. **マルチCPU・マルチコア・マルチスレッドの非同期処理対応**:
   `tokio`の`#[tokio::main]`は既定のmulti_threadフレーバーを使う
   (`current_thread`への固定を避ける)。CPU負荷の高い処理は
   `rayon`(`opencuda-cpu`が既に採用)や`tokio::task::spawn_blocking`を
   活用し、単一スレッドがボトルネックにならないようにする。
2. **ドメイン(テナント)ごとの個別インストール不要**: 各サービスは
   1つの共有インスタンスとして起動し、`POST /admin/tenants`
   (`aruaru-llm`で実装した`src/tenants.rs`の`TenantRegistry`パターンを
   踏襲、`x-admin-token`ヘッダによる簡易認証込み)で動的にテナント
   (ドメイン)を登録・削除できるようにする。プロセス再起動は不要。
3. **管理は`open-easy-web`側で行う**: 個々のサービスが管理UIを
   別々に持つのではなく、`open-easy-web`(第二のKUSANAGI、易操作ツール)
   の管理画面から、各共有サービスの`/admin/tenants`系APIを呼び出して
   テナント登録・削除を行う(`open-web-server`/`poem-cosmo-tauri`向けの
   既存`appserver_registration.rs`と同じ設計思想を、`open-cuda`/
   `aruaru-llm`/`RPoem`/`RCosmo`/`open-raid-z`/`aruaru-db`向けにも
   拡張する)。

**現状の実装状況(2026-07-18、調査・実装完了)**:
- **`aruaru-llm`**: `src/tenants.rs`(`TenantRegistry`、
  `POST /admin/tenants`・`GET /admin/tenants`・
  `DELETE /admin/tenants/:host`)実装済み。`cargo build`/`cargo test`
  (10件全green)、および実バイナリでの一連のHTTPフロー
  (登録→一覧→tenant付きchat→削除→一覧)を実際に検証済み。
- **`RPoem`・`RCosmo`・`open-web-server`**: 調査の結果、**既にこの
  「分身の術」パターンが実装済み**であることが判明(`RPoem`/`RCosmo`は
  `crates/open-runo-gateway/src/appserver_tenants.rs`+
  `open-runo-appserver/src/tenant_bridge.rs`、`open-web-server`は
  `crates/open-web-server-gateway/src/tenant_router.rs`+
  `handlers/tenants.rs`)——追加実装は不要と判断。
- **`open-cuda`・`open-raid-z`**: HTTPサービスではなくライブラリ
  (GPUランタイム/ストレージ)のため、「ドメインごとの個別インストール」
  という概念自体が当てはまらない。path依存として複数プロジェクトから
  共有される時点で要件を自然に満たしており、追加実装は不要と判断。
- **`aruaru-db`**: 既存の`aruaru-server`(pgwire)自体が「1インスタンスを
  複数クライアントアプリが接続して共有する」設計であり、HTTPの
  `/admin/tenants`的な仕組みを別途持つよりSQLデータベース/スキーマ単位の
  マルチテナント性を活かす方が自然なため、追加実装は見送り。
- **`open-easy-web`側の管理統合**: `appserver_registration.rs`の
  `AppServerKind`に`AruaruLlm`variantを追加し
  `register_aruaru_llm()`を実装済み(`x-admin-token`ヘッダ認証、
  `POST /admin/tenants`呼び出し)。`cargo test`50件全green
  (新規1件含む)。WASM側(`src/profiles.rs`)の選択肢UIへの反映は
  未着手(次回以降)。

## 関連プロジェクト

- **RS-Ops**(旧`RS-AI-DevOps`、2026-07-22リネーム。エコシステム全体マップ
  自動生成+AIエージェント向けコンテキストファイル(CLAUDE.md/.cursorrules/
  AGENTS.md)生成+複数Git/課題管理サービス連携。GitHub/RS-Chiketto/GitLab/
  Bitbucket/標準Redmine実装済み・実HTTP検証済み(Gitbucketは公開デモが無く
  未検証)。優先度の星1〜5評価、AIツール個別対応(Claude/Claude Code Desktop/
  Claude(ブラウザ)/Cursor/ChatGPT/Gemini/DeepSeek/Grok)、OTPログイン+
  Viewer/Editor/Adminのチーム権限管理、16言語UIまで実装。
  `https://runo.tokyo/RS-Ops`で稼働中):
  https://github.com/aon-co-jp/RS-Ops
- **RS-Guard**(2026-07-22新設。サプライチェーン/ウイルス/スパイウェアの
  静的スキャナ。既知悪意パッケージ名ブロックリスト・疑わしいスクリプト
  パターン・EICAR等マルウェアシグネチャ・スパイウェア挙動(無断の情報収集/
  持ち出し/常駐・自動巡回)を深刻度付きで検出+ClamAV委譲。既存アンチウイルスを
  置き換えず併用。AI二次判定は`aruaru-llm`の`/v1/classify-security`
  (open-cuda埋め込み)を「分身の術」で共有呼び出し。runo.tokyo/RS-Guardが
  紹介・ダウンロード、easy-web.tokyo/RS-Guardがログイン後の実運用画面
  (open-easy-web統合予定)):
  https://github.com/aon-co-jp/RS-Guard
- **poem-cosmo-tauri**(poem-cosmo-tauriとopen-runoを同時並行開発。実装の
  先行地点。Pure Rust + tokio/hyper直接実装): https://github.com/aon-co-jp/RPoem
- **open-runo**(poem-cosmo-tauriと同時並行開発。2026-07-10付けで開発再開):
  https://github.com/aon-co-jp/open-runo
- **open-web-server**: https://github.com/aon-co-jp/open-web-server
- **aruaru-db**: https://github.com/aon-co-jp/aruaru-db
- **open-easy-web**(第二のKUSANAGI、ドメイン/サブドメイン簡単登録+HTTPS
  自動監視/発行/更新の易操作ツール。高速化機能は含まない、2026-07-13に
  aruaru-webから分離): https://github.com/aon-co-jp/open-easy-web
- **aruaru-web**(2026-07-13廃止。役割はopen-easyweb(易操作)と
  open-runo/poem-cosmo-tauri(高速化)へ分割継承済み): https://github.com/aon-co-jp/aruaru-web
- **open-cuda**(GPUランタイム、`aruaru-llm`とSET構成): https://github.com/aon-co-jp/open-cuda
- **aruaru-llm**(契約不要の独自AIチャットコマース応答サービス、`open-cuda`とSET構成。
  2026-07-22、`POST /v1/classify-security`を追加——コード片をマルウェア/
  スパイウェア/常駐・自動巡回/正常へ埋め込みコサイン類似度で分類し
  RS-Guardへ二次判定を提供): https://github.com/aon-co-jp/aruaru-llm
- **e-gov.info**(デジタルガバメント×オンライン貿易プラットフォーム、サンプル・デモ段階): https://github.com/aon-co-jp/e-gov
- **open-raid-z**(このリポジトリ): https://github.com/aon-co-jp/open-raid-z
- **rs-to-readme**: https://github.com/aon-co-jp/rs-to-readme
- **RS-Git**(旧RGit、2026-07-22リネーム。Gitea/GitBucket相当、自己ホスト型Git forge。OTPログイン・
  アクセス制御・容量ベースの自動判定まで実装済み、WASM UIも着手済み):
  https://github.com/aon-co-jp/RS-Git
- **RJSON**(`rust-json`クレート、寛容/厳密JSONパース+依存ゼロの`light`
  モジュール、RS-GitのWASMフロントエンドが利用): https://github.com/aon-co-jp/RJSON
- **RS-Chiketto**(Redmine相当、v0.1.0チケットCRUD+OTP認証まで実装済み):
  https://github.com/aon-co-jp/RS-Chiketto
- **RS-Blog**(WordPress相当、PHPプラグイン互換レイヤも目指す、器のみ):
  https://github.com/aon-co-jp/RS-Blog
- **RS-EC**(EC-CUBE相当、実決済連携〈Stripe等〉も目指す、器のみ):
  https://github.com/aon-co-jp/RS-EC

<!-- AUTO-GENERATED ECOSYSTEM MAP START (runo-scanner --update-ecosystem-map) -->
- **RBootstrap**([RFrontEnd](https://github.com/aon-co-jp/RFrontEnd)傘下、Bootst…): https://github.com/aon-co-jp/RBootstrap
- **RCSS**(作業ドライブは`F:\open-runo`。この節は[`open-raid-z`](https://github.com…): https://github.com/aon-co-jp/RCSS
- **RCosmo**(「配信エンジン(vhost)」に`open-web-server`を選択肢として追加したが、): https://github.com/aon-co-jp/RCosmo
- **RGraphQL**(`RGraphQL`は、GraphQLのRust版を、既存のGraphQL実装(`async-graphql`/): https://github.com/aon-co-jp/RGraphQL
- **RHTML**(作業ドライブは`F:\open-runo`。この節は[`open-raid-z`](https://github.com…): https://github.com/aon-co-jp/RTHML
- **RNode.js**(Node.js のコア概念を、既存の Node.js 実装コードを一切流用せず Rust で): https://github.com/aon-co-jp/RNode.js
- **RReact**(作業ドライブは`F:\open-runo`。この節は[`open-raid-z`](https://github.com…): https://github.com/aon-co-jp/RReact
- **RS-JSON**(`Rust-JSON`は、以前`open-runo`/`poem-cosmo-tauri`内のクレート): https://github.com/aon-co-jp/RS-JSON
- **RTypeScript**(作業ドライブは`F:\open-runo`。この節は[`open-raid-z`](https://github.com…): https://github.com/aon-co-jp/RTypeScript
- **aon.co.jp**(`aon.co.jp`のTOPページ。[`aon-tokyo`](https://github.com/aon-co-j…): https://github.com/aon-co-jp/aon-co-jp
- **aon.tokyo**(`aon.tokyo` / `aon.co.jp`(同一内容・同一バイナリで両ドメインを配信)のTOPページ。): https://github.com/aon-co-jp/aon-tokyo
- **aruaru.tokyo**(`aruaru.tokyo`のTOPページ。2026-07-15、それまでPHPで実装していたものをRust+[Poem…): https://github.com/aon-co-jp/aruaru.tokyo
- **audiocafe-tokyo-rust**(`audiocafe.tokyo`の既存PHPモノリス([`audiocafe-tokyo`](https://gith…): https://github.com/aon-co-jp/audiocafe-tokyo-rust
- **karu.tokyo**(`karu.tokyo`のTOPページ。軽井沢・あきる野市・東京を含む日本の観光と): https://github.com/aon-co-jp/karu-tokyo
- **rs-sync**(VPS上の既存`/root/sync-repos.sh`(cron、aon-co-jp組織の全リポジトリを): https://github.com/aon-co-jp/RS-Sync
- **runo.tokyo**(`runo.tokyo`のTOPページ。東京都西部(あきる野市・旧五日市・桧原村・): https://github.com/aon-co-jp/runo.tokyo
<!-- AUTO-GENERATED ECOSYSTEM MAP END -->

### 同時並行開発の対象(2026-07-21、ユーザー指示)

上記のうち`RS-Chiketto`・`RS-Blog`・`RS-EC`(1つずつ順番に着手、現在は
`RS-Chiketto`から着手中)・`open-raid-z`・`aruaru-db`・`open-cuda`・
`aruaru-llm`・`open-web-server`・`open-cosmo`・`RPoem`、および
Python製AIライブラリのRust移植ハイブリッド/トライブリッド版
(マーケティング調査1〜6位、vLLM/Transformers/NumPy/PyTorch互換/
scikit-learn/Whisper相当、Rustを基本とし必要なら`RPoem`も併用)は、
**同時に開発を進め、エコシステム全体の完成度を高めていく**方針。
各プロジェクトの現況・詳細は、そのリポジトリ自身の`CLAUDE.md`の
HANDOFF節を参照すること(**どのリポジトリから読んでも、この節を
起点に他プロジェクトへ辿れるようにしてある**)。

## 運用ルール

- **開発中はこの`CLAUDE.md`を、コード変更のコミット/pushと必ず一緒に
  push する**(内容を更新した場合はもちろん、変更が無い場合も他の変更と
  一緒にコミット対象へ含めておくこと)。
- 実装で迷った場合や、API仕様の詳細確認が必要な場合は、学習データからの
  推測より公式ドキュメント(上記URL)を優先して参照する。
- 作業ドライブ(現在`F:\open-runo`)が変わった場合は、この節を更新し、
  CHAT_HANDOFF.md にも変更の経緯を記録すること。
- **ローカル作業ドライブ(`F:\open-runo`)上の各リポジトリは、常にリモート
  (GitHub)の最新コミットに追従させておくこと**(`git fetch`/`git pull`を
  こまめに実行する。ローカルにのみ存在する未コミット変更がある場合は、
  上書き前に必ず内容を確認し、必要なら `git stash` で退避してから最新化
  する)。
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
- **ドキュメント(parity調査・ギャップ一覧・HANDOFF等)に「未調査」「未着手」
  「意図的に先送り」「将来検討」等と記録されている項目を見つけた場合、
  それは承認待ちの提案ではなく、次に着手すべき実装対象そのものである**
  (ユーザー指示、2026-07-12)。「ご指示はありますか?」のように追加の
  確認を求めて手を止めず、確認不要でそのまま実装を進めること。
  優先度・実現可能性の判断(例: ブラウザ実行という設計上の制約で
  技術的に不可能な項目は代替案を検討する)は自分で行ってよいが、
  「未着手だから今回は見送る」という判断そのものをユーザーへの
  確認なしに下してはならない——見送る場合も、まず着手を試み、
  真に不可能/著しく非現実的と判明した場合のみ、その理由をドキュメントに
  明記した上で次の項目に進む。
- **技術選定・仕様確認で迷った場合、必要に応じて日本語と英語の両方で
  Google検索し、Claude(自分自身)の知識・推論も動員し、GitHubでも
  調査すること**(ユーザー指示、2026-07-13)。
  学習データからの推測だけに頼らず、実在するクレート・ライブラリの
  現状(バージョン・メンテナンス状況・プラットフォーム対応)や、
  最新の実務知見(2026年時点のベストプラクティス等)を実際に検索して
  裏付けを取ってから実装判断を下す。日本語のみ・英語のみでは見つからない
  情報が言語を変えると見つかることがあるため、両言語での検索を基本とする。
- **よほど確認が必要な場面(重大な破壊的操作・仕様の根本方針転換等)を
  除き、確認を求めて手を止めないこと**(ユーザー指示、2026-07-13)。
  技術選定や実装方法で分からないこと・迷うことがあれば、まず上記の通り
  日本語・英語両方でのGoogle検索・GitHub調査を行い、それでも判断が
  つかない場合は自分の工学的判断で最も妥当な選択をして実装を進める。
  「〜については確認が必要です」と言って作業を止め、ユーザーの回答を
  待つことを既定の振る舞いにしない。
- **バックグラウンド実行(ビルド・テスト・サブエージェント)を「見失わない」
  ための定期確認と、無人での自動再実行**(ユーザー指示、2026-07-18)。
  背景: 実際に発生した事象として、(a) サブエージェントを並列起動した際、
  完了通知が届く前にタスク管理システム側のタスクIDが失効し
  `No task found` となった(実作業自体は正常に完了し `git status`/`git
  diff` で裏取りできた——**タスク管理メタデータの消失と実際の作業結果は
  別物**)、(b) サブエージェントが最終応答として実装内容の要約ではなく
  「これから通知を待ちます」のような独り言的なテキストのみを返した
  (これも実際にはファイル変更が完了していた)、(c) 長時間ビルドが
  タイムアウトで打ち切られ `error: could not compile` 相当のログが出たが、
  実際にはコンパイルエラーではなく単に時間切れだった(タイムアウトを
  伸ばして再実行したら成功した)。これらはいずれも「本当に失敗/消失した」
  のではなく「見かけ上そう見えただけ」だったが、区別せずに放置すると
  本物の失敗を見逃す・止まっている作業に気づけないリスクがある。
  対応方針:
  1. バックグラウンドで実行中の処理(ビルド・テスト・並列サブエージェント
     等)がある間は、放置せず**一定間隔で状態を能動的に確認する**
     (タスク一覧の確認、生きている場合は`running`であることの確認)。
     ただし完了通知が来る処理を無意味に頻繁にポーリングしない
     (通知の仕組みで拾えるものは通知を待つ)——「見失っていないか」を
     時々確認する頻度で十分。
  2. タスク管理システムの応答(`No task found`・要領を得ない完了報告文・
     タイムアウトによるエラー風ログ等)を**鵜呑みにしない**。実際に何が
     起きたかは必ず一次情報で裏取りする: 対象リポジトリの`git status`/
     `git diff`(変更が実在するか)、ビルド/テストログの実際の中身
     (本物のコンパイルエラーか、タイムアウトによる強制終了(exit code
     124/143等)かを区別する)、生成物ファイルの実在確認。
  3. 裏取りの結果、**作業が実際に失われている・未完了・本物のエラーで
     失敗している**と判明した場合は、確認を求めて手を止めず、
     そのまま自動的に(無人で)再実行・修正する(上記の「確認を求めて
     手を止めないこと」と同じ扱い)。タイムアウトが原因なら、より長い
     タイムアウトで再実行する、または完了を待つ設計(バックグラウンド
     実行+完了通知待ち)に切り替える。
  4. 裏取りの結果、**作業自体は実際には完了しており通知/タスクIDだけが
     欠落していた**場合は、二重実行で無駄なリソースを使わないよう、
     その旨を記録した上でそのまま先に進む。
  5. 定期的な状態確認・再実行の判断はユーザーへの確認を求めず自分で
     行ってよい(既存の「確認を求めて手を止めない」方針の一部として扱う)。
- **コンテキストウインドウ・5時間利用制限・その他のセッション中断が
  発生し、その後リミットが解除されて新しいセッションが開始された場合、
  「続けてよろしいですか」等の確認を挟まず、毎回自動的に前回セッションの
  続きの作業を再開すること**(ユーザー指示、2026-07-18)。具体的には:
  1. セッション開始時、各リポジトリの`git status`/`git log`と、この
     `CLAUDE.md`(および各プロジェクトのCLAUDE.md)のHANDOFF節・
     「次にすべきこと」記載を確認し、未完了・未pushの作業が無いかを
     まず裏取りする(タスク管理メタデータを鵜呑みにしない既存方針と
     同じ姿勢で、実際のgit状態を確認する)。
  2. 未完了作業が見つかった場合、ユーザーへの確認を求めず、そのまま
     自動的に検証(build/test)→修正→コミット→pushまで完了させる。
  3. 完了している場合は、各CLAUDE.mdの「次にすべきこと」「未着手・
     未完成」に記載された次の項目へ確認なしに着手する(既存の
     「未着手だからといって確認を求めて手を止めない」方針の延長)。
  4. 「続けてよろしければそのまま自動開発を継続します」のような、
     続行そのものを尋ねる確認は今後一切行わない(ユーザー指示、
     2026-07-18)。作業内容の要約・進捗報告はしてよいが、それは
     承認を求めるものではなく完了報告として書く。
  5. こまめにコミット・pushしておくことで、次回セッションが「どこから
     再開すべきか」を迷わず`git log`/CLAUDE.mdから機械的に判断できる
     ようにしておく(区切りがついた時点で都度コミット・pushする既存
     方針との組み合わせ)。
- **WEB/UIを持つ機能を実装した後は、ビルド成功・`cargo test`・
  curlでのステータスコード確認だけで「完了」と報告せず、実際に画面が
  正しく表示される(白画面・レンダリング崩れ・コンソールエラーが
  無い)ところまで確認すること**(ユーザー指示、2026-07-19)。背景:
  「開発後に画面が真っ白になる」といった、HTTPステータスやビルド成功
  だけでは検知できない不具合が実際に起こり得る。対応方針:
  1. ブラウザ操作が可能な環境では、実際にページを開いて表示内容
     (見出し・本文・想定した要素の存在)とコンソールエラーの有無を
     確認する(`preview_start`+`read_page`/`get_page_text`/
     `read_console_messages`等、利用可能なブラウザツールを使う)。
  2. ブラウザ操作ができない環境(バックグラウンドサブエージェント等)
     では、少なくとも`curl`等でHTMLボディの中身を取得し、期待される
     文字列(見出し・特定のテキスト)が実際に含まれているかを
     `grep`等で確認する——ステータスコード200だけを見て「動作確認済み」
     としない(空のbody・エラーページも200を返すことがあるため)。
  3. 確認の結果、白画面・エラー・期待した内容の欠落等の不具合が
     見つかった場合は、確認を求めず自動的に原因調査・修正・再確認まで
     行う(このファイルの「無人での自動再実行」節と同じ扱い)。
  4. **本番ドメインが未取得・DNS未設定なだけの状態**(例:
     `e-gov.info`のようにまだサンプル・デモ段階でドメインが実在しない
     プロジェクト)は、上記の「白画面バグ」とは別物であり、混同しない
     こと——`localhost`/開発用ポートでの動作確認で代替できる場合は
     それで十分とし、無関係なDNS登録作業を勝手に行わない。
- **本番インフラの実行操作(nginx設定reload、systemdサービス再起動、
  段階的カットオーバー等)は、技術的な検証(設定構文チェック・内容/
  見た目の一致確認)が済んでいれば、都度「実行してよいですか」と
  確認を求めず、そのまま実行すること**(ユーザー指示、2026-07-19)。
  対象は「破壊的で取り返しがつかない操作」(データの永久削除、
  force push等)ではなく、設定reloadやサービス再起動のような
  再度戻せる/元の設定ファイルをバックアップ済みの操作に限る——
  そうした操作でも実行前に確認を求めていた従来の慎重さは、この
  エコシステム内の作業に関しては不要と明示的に指示された。
  設定変更前のバックアップ取得(`cp`でのタイムスタンプ付き複製等)は
  引き続き行うこと(「元に戻せる」を実際に担保するため)。
- **「エコシステム全体に関わる依頼」(例: プロジェクトシリーズ一覧・
  横断的なドキュメント整備・全リポジトリ共通のルール変更・複数
  リポジトリにまたがる機能追加など)を受けた場合、依頼者がリポジトリを
  1つずつ個別に指定しなくても、関連する全リポジトリを自動的に洗い出して
  横断的に調査・変更すること**(ユーザー指示、2026-07-20)。具体的には:
  1. 依頼の内容が特定の1リポジトリに閉じるものか、複数リポジトリに
     またがる「エコシステム全体」的な性質のものかをまず判断する。
     後者と判断した場合、`F:\open-runo`直下の全ディレクトリを列挙し、
     各ディレクトリが実際にGitリポジトリ(`.git`が存在し、GitHub上の
     `aon-co-jp` organizationへの remote を持つもの)かどうかを機械的に
     確認した上で、依頼内容に関係する対象リポジトリを特定する
     (「関係する」の判断基準: 依頼内容のキーワード・機能が実際に
     そのリポジトリのREADME.md/CLAUDE.mdやコードに存在するか、または
     依頼内容が明示的に「全プロジェクト」「エコシステム全体」等を
     指しているか)。
  2. 対象リポジトリを特定したら、依頼者に「どのリポジトリを対象にするか」
     を都度確認する質問はせず、特定した範囲でそのまま横断的に
     調査・実装・検証・commit・pushまで進める(既存の「確認を求めて
     手を止めないこと」方針の一部として扱う)。
  3. ただし、対象範囲の特定自体が本質的に曖昧で、依頼内容だけからは
     どのリポジトリ群を指すのか判断がつかない場合(例: 新規リポジトリの
     作成が必要かどうか、公開範囲や命名が依頼内容と食い違う場合等)は、
     「エコシステム全体だから確認不要」の対象外とし、通常どおり
     依頼者に確認する(この節は「個別リポジトリ名の指定を省略してよい」
     という意味であり、「対象範囲の曖昧さそのものを確認なしで独断で
     決めてよい」という意味ではない)。
  4. 横断作業の結果は、各リポジトリのCLAUDE.md HANDOFF節にそれぞれ
     記録し、最後にどのリポジトリを対象にした/しなかったかを依頼者への
     完了報告にまとめて明記する(暗黙のうちに一部リポジトリを除外した
     まま「完了」と報告しない)。

- **バックグラウンド並行開発エージェントの停滞自動検知・自動再開
  (ユーザー指示、2026-07-21)**: 複数リポジトリで並行してバックグラウンド
  エージェントを走らせている間は、単に完了通知を待つだけでなく、
  定期的に(`/loop`のような自己ペース監視、既定は数分〜20分間隔、
  ユーザー指示があればその間隔に従う)各対象リポジトリの`git log
  --oneline -3`と`git status --short`を確認し、**直近複数回のチェックで
  コミット・作業ツリーの差分が全く変化していない場合は「停滞」と
  みなす**。実際にこのエコシステムで、エージェントが実装は行うものの
  「別のエージェントを起動しました」「完了を待っています」という
  自己言及的な報告だけを繰り返し、実際には何もコミットしないまま
  ループする事例が複数回発生したため、この検知は実際に有効だと確認済み。
  1. 停滞を検知したら、確認を求めずSendMessageで該当エージェントへ
     「これ以上何かを待つ・別エージェントに委任するのではなく、
     自分自身のBash/Read/Write/Editツールで直接ビルド・テスト・
     `git add`/`commit`/`push`まで完了させよ」と明示的に再指示する。
  2. **エージェントがユーザー操作によって停止(`stopped by the
     user`)している場合の扱い(2026-07-21、ユーザー指示により訂正)**:
     この停止は、依頼者本人が明示的にチャットで「このタスクは中止して」
     と述べたのでない限り、**誤操作(意図しないクリック・UI操作等)に
     よるものである可能性が高いと判断し、確認を求めず自動的に同じ
     指示内容で新しいエージェントを起動し直す**(未コミットの変更は
     ディスク上に残っているため、そこから作業を再開させる指示を含める)。
     以前は「二度と再起動しない」という慎重すぎる方針だったが、
     ユーザーからの明確な訂正を受けて上記の通り変更した。ただし、
     直前の会話で依頼者が該当タスクの中止を明言している場合は、
     従来通り再起動しない。
  3. 監視間隔はユーザー指示があれば都度変更する(例: 「5分に変更して」
     と言われたら、そのつどスケジュールを組み直す)。監視対象が
     全て完了またはユーザーにより停止済みになった時点で、監視ループ
     自体も終了する(いたずらに空回りさせない)。

## HANDOFF(直近の自動巡回ログ)

- **2026-07-20 (4) ドキュメント整合性チェック(監査、`CHAT_HANDOFF.md`/
  `MIGRATION.md`/`PORTING.md`/`README.md`)**: 実装状況(このファイルの
  HANDOFF節)とこれら4ファイルの記述を突き合わせ、3件の齟齬を修正。
  (a) `README.md`/`PORTING.md`のテスト件数バッジ・本文が2026-07-11時点の
  「163テスト」のまま古かったため、2026-07-20実測(166テスト
  [104+32+30]、`foreign_fs`込みでWindows112・Linux(WSL2)115)へ更新。
  (b) `MIGRATION.md`の移行方式表が「ext4はOS標準の`mount`のみ、`orzctl`は
  関与しない」という古い記述のままだったため、2026-07-20実装済みの
  `orzctl foreign --format ext4`(読み取り専用)を反映。
  (c) `CHAT_HANDOFF.md`追記32の「次に着手すべきこと」item 3
  (initramfs/switch_root実験が未着手)が、実際には同ファイル内の
  追記24で既に実機成功済みという矛盾を発見、追記33として訂正・
  現状の残タスクを整理して追記。コード変更なし、ドキュメントのみ。

- **2026-07-20 (3) CI恒常失敗の修正(存在しないfeature名+rustfmt未整備)**:
  GitHub Actions CIが作成当初(c9dac59)から一度もgreenになっていなかった
  ことを発見。原因は2つ: (a) `ci.yml`が存在しないfeature名
  `foreign_fs_fat,foreign_fs_exfat`を指定していた(正: `foreign_fs`)、
  (b) `cargo fmt --check`がrustfmt設定なしで走り、コードの実スタイルと
  既定整形が衝突していた。対応: ci.ymlのfeature名修正、
  `open_raid_z_core/rustfmt.toml`新設(max_width=120・
  use_small_heuristics="Max")の上で`cargo fmt`を全面適用、
  `cargo clippy --all-targets -- -D warnings`で検出された4件
  (collapsible_if・manual is_multiple_of・useless vec!・
  large_enum_variant→全バリアントBox化)も修正。fmt/clippy/テスト
  (Windows 112・WSL2 Linux 115)全green確認済み。

- **2026-07-20 (2) ext2/ext4読み取りブリッジ実装 + chunk_size=65536破損バグのWSL2実FUSE検証**:
  1. **ext2/ext4読み取り対応(MULTIPLATFORM_ROADMAP.md目標②の未着手項目)**:
     純Rustの`ext4-view` 0.9.3(`std` feature必須——既定のno_stdでは
     `Ext4::load_from_path`が存在せずコンパイルエラーになる点に注意)を
     ラップした`ForeignExt4Volume`(読み取り専用)を`foreign_fs.rs`へ追加。
     `orzctl foreign --format ext4 ls/cat/mount`対応(`put`は明示エラー、
     FUSEマウントは`MountOption::RO`)。`foreign_fuse_mount.rs`の
     `ForeignVolume`へ`Ext4`バリアントと`is_read_only()`を追加。
     テスト: 実`mkfs.ext4`(e2fsprogs 1.47、WSL2 Ubuntu 26.04、root不要の
     `debugfs -w`でファイル投入)製の512KiBフィクスチャ
     `tests/fixtures/ext4_small.img`を使う統合テスト`tests/foreign_ext4.rs`
     (8件)を新規作成し、**Windows(112テスト)・Linux/WSL2
     (fuse_backend+foreign_fs、115テスト)の両方で全green**。
     さらにWSL2実FUSEマウントE2E(ls/cat/put拒否/mount/カーネルRO強制)も
     実機確認済み。`zfs_accel_hlsl`の死コード
     (`#[cfg(not(feature="gpu"))] mod imp`、dead_code警告の原因)も削除
     (32テストgreen維持)。
  2. **chunk_size=65536書き込み破損バグ(2026-07-18 HANDOFFの継続)**:
     このWindows機の**WSL2 Ubuntu(/dev/fuse実在)でfuse_backendビルドの
     実FUSEマウントによる再現を初めて実施**。Z2・4ディスク(64-128MiB
     ループバックイメージ)・chunk_size=65536で、cp(4MiB/2.7MiB/
     131071/131072/131073/20MiB)・上書き(縮小/拡大)・dd bs=131072・
     ddによる順序入れ替え書き込みの全9ケースで、アンマウント→再マウント後も
     **全てbyte-exact一致、破損は一切再現せず**。また「メタデータ溢れが
     データストライプを汚した」仮説はgit履歴検証(12bb343^のsave()は
     溢れ時にCapacityExceededを返す設計で上書きはしない)により棄却。
     結論: 現行コードは実FUSE(WSL2カーネル)では健全。当時のVirtualBox VM
     報告は、その後の複数修正(unaligned書き込み系の改良等)で解消済みか、
     VM環境固有の要因の可能性が高い。残タスクは「元のVirtualBox VM
     (`open-raid-z-linux-boot`)での最終確認」のみ(任意・低優先へ格下げ)。

- **2026-07-20 運用ルール追記: エコシステム全体に関わる依頼の自動横断対応**:
  ユーザー指示により、「エコシステム全体に関わる依頼」を受けた際に
  リポジトリを1つずつ個別指定しなくても関連リポジトリを自動的に
  洗い出して横断的に対応する運用ルールを追記(本ファイル上部
  「無人自動開発の運用ルール」節、2026-07-20付け新規項目)。
  ドキュメント追記のみ、コード変更なし。

- **2026-07-15 コードヘルス監査 — audit only, no changes**:
  `open_runo_zfs_source`配下の3クレート(`open_raid_z_core`・
  `zfs_accel_hlsl`・`open_runo_installer_core`)を`--no-default-features`
  (WinFsp SDK/dxc/Windows SDK不要のCPUフォールバック)でそれぞれ個別に
  `cargo build`/`cargo test`し、全てビルド成功・合計108テストgreen
  (46+32+30)を確認。警告はdead_code(未使用関数)・命名規則
  (`BusTypeSata`等、Windows API由来の定数名でclippy naming lintに
  引っかかるが実害なしのスタイル指摘)のみで、いずれも軽微なため修正は
  見送った(この監査は破壊的リファクタを行わない方針のため)。`git
  status`はクリーン、修正すべき壊れたビルド・失敗テスト・小規模な欠落は
  見つからなかったため、コード変更は行っていない。デフォルトfeature
  (実マウント+GPU高速化)はWindows実機+WinFsp SDK+dxcが必要なため
  今回も未計測(既存の制約どおり)。

- **2026-07-18 chunk_size=65536書き込み破損バグの調査・メタデータ容量バグの現状確認**:
  `CHAT_HANDOFF.md`追記21/24で報告された2件の実バグを調査。(1)
  メタデータ容量上限バグは追記30で既に根本修正済み(`superblock_stripe_count`
  による動的予約、README「容量無制限」記述も既に削除済み)であることを
  `pool.rs`・README.mdで確認、追加対応不要。(2)
  chunk_size=65536・RAID-Z2・4ディスクでのストライプ境界書き込み破損疑いは、
  `write_unaligned`/`write_unaligned_growing`/`align_range`/`Pool::write`/
  `Pool::read`/`vdev.rs`の`write_stripe`/`read_stripe`/`block_device.rs`を
  精読したが論理上のオフバイワンは発見できず。`tests/unaligned_io.rs`に
  実際の条件(chunk_size=65536・Z2 4ディスク・131072バイトFUSEバッファ相当の
  ストリーミング書き込み・末尾が境界に揃わないサイズ)を再現する回帰テスト
  `streaming_writes_with_fuse_sized_buffer_are_byte_exact_across_stripe_boundaries`
  を追加したが、Pool API直呼び出しでは再現せず(byte-exactで成功)。
  よって原因はPool/vdev層のロジックではなく、実FUSEマウント(Linuxカーネルの
  writebackページキャッシュの発行順序・並行ディスパッチ等)固有の要因である
  疑いが強い。このWindows専用サンドボックスには実FUSEマウント環境が無いため
  実機再現・特定は次回、Linux VM(`open-raid-z-linux-boot`)上で`cp`による
  ストリーミング書き込みを行いながら`strace`等でFUSE write要求の実際の
  offset/size列を記録し、`tests/unaligned_io.rs`側の再現テストの入力パターン
  (offset順序・サイズ)をそれに合わせて調整することを推奨。
  `cargo test --no-default-features`(`open_raid_z_core`)は新規テスト込みで
  全102テストgreen(既存回帰なし)。

## 現状(このリポジトリ固有、2026-07-11時点)

- ルート`README.md`・10ヶ国語版`README-<言語>.md`(日本語・英語・中国語
  簡体字・韓国語・スペイン語・フランス語・ドイツ語・イタリア語・
  ロシア語・アラビア語、姉妹リポジトリと同じ命名規則でルート直下に配置)・
  `PORTING.md`を新規作成した(このリポジトリはこれまでルート
  `README.md`1本のみで、`PORTING.md`は未作成だった。旧`README/`
  フォルダの10言語版(UK/US English・Ukraine・Iran(Persian)を含む異なる
  言語セット)はそのまま残置、新規ファイルが姉妹リポジトリ標準の現行版)。
- 実測ファクト(2026-07-20更新): `open_raid_z_core`/`zfs_accel_hlsl`/
  `open_runo_installer_core`の3クレート構成、`cargo test
  --no-default-features`(WinFsp SDK/dxc/Windows SDK不要のCPU
  フォールバック)で合計166テストpassed・failed 0
  (104 + 32 + 30)。`--features foreign_fs`を加えるとext4統合テスト8件が
  加わり`open_raid_z_core`は112(Windows実測)。Linux(WSL2)の
  `--features fuse_backend,foreign_fs`では115。`default`feature
  (実マウント+GPU高速化)はWindows実機+WinFsp SDK+dxcが必要なため
  今回は未計測。
- **2026-07-13 (aruaru-db側から`open_raid_z_core`をpath依存する新規利用者
  が追加)**: `aruaru-db`(`crates/aruaru-dist`)が、`open-web-server/
  CLAUDE.md`拡張要件(2)「次回新規開発予定」(aruaru-dbコミット×ZFS風
  スナップショット連携)の第一段実装として、本クレートを
  `default-features = false`(WinFsp/dxc/Windows SDK不要のCPUフォール
  バックのみ、`open_raid_z` featureで任意有効化)でpath依存するように
  なった。`Pool::create_snapshot`をRaft commit完了フックから呼び出し、
  実RAID-Z2プール(6台の`FileBackedDevice`)上での実スナップショット
  作成をaruaru-db側の統合テストで検証済み。詳細はaruaru-db側`CLAUDE.md`
  HANDOFF節を参照。本クレート自体のAPI・実装に変更は無い(利用側の
  新規結線のみ)。

## アプリケーションサーバー層の役割(open-runo / poem-cosmo-tauri、2026-07-16追記)

「配信エンジン(vhost)」に`open-web-server`を選択肢として追加したが、
open-web-serverがApache＋Nginxのハイブリッド仕様のWebサーバーとして
まだ機能していない間は、Tomcatのような互換レイヤーとして機能するのは
`open-runo`または`poem-cosmo-tauri`である。

これらは`open-raid-z`とVersionlessAPIによって、バージョンレス運用と
バージョン管理・Git管理を両立しながら、ACID互換性とZFS互換性に対応した
`aruaru-db`と、PostgreSQLとのDUAL DATABASE構成による「4層4重」の
最新鋭の通信システムを構築し、仕様変更が容易なデータベース設計により、
3DオンラインゲームAI課金アイテム、オンライン金融、オンライン証券、
オンラインクレジットカード決済など、ネット上で紛失してはならない
ミッションクリティカルな用途向けに、24時間365日ノンストップの
サーバー対応WEBサイト開発を全面的にバックアップするフレームワーク・
ミドルウェアとして機能することを目指す。
