# 設計思想・開発方針・開発環境ルール(全リポジトリ共通ヘッダー、2026-07-15追記)

## 0. 設計思想(2026-07-25追記、エコシステム全体の正本)

- **DB非依存・1バイナリ完結・テンプレートエンジン不使用**——重量級
  フレームワーク/ORMに依存せず、Rust標準+tokio/hyperを基本とする
  (詳細は本ファイル各所の技術スタック節を参照)。
- **「型チェック・ビルド成功のみで完了と報告しない」**——実機・実データ
  での検証(数値一致・実HTTPレスポンス本文の確認等)を経てから「動作した」
  と報告する。未実装・スタブ機能は「対応している」という誤ったシグナルを
  出さない(正直な開示を最優先する)。
- **車輪の再発明を避ける**——既存の実績あるOSS実装(クレート・ツール)を
  日英Web検索で調査し、使えるものは自作せず依存する。
  外部AI事業者との有償契約が必要な機能は`open-cuda`+`aruaru-llm`の
  自前SET構成を標準とし、契約不要なAI機能を優先する。
  グラフィックス/GPU抽象化(`open-cuda`)は、Windows/Linux/Mac/Android、
  そして将来的にはINTEL/AMD/nVIDIA各ベンダーの垣根を越えた互換性統合を
  目指す(`open-directx`との並行開発、詳細はopen-cuda/open-directx側
  CLAUDE.md参照)。
- **未着手・見送りの自動判断をしない**——ドキュメントに「未調査」
  「将来検討」と書かれた項目は次に着手すべき実装対象そのものであり、
  確認を求めて手を止めない(詳細は本ファイル内の各運用ルール節を参照)。
- **命名・配置判断を伴う場面は事前確認**——新規リポジトリ作成・既存
  リポジトリへの新規クレート追加等は、着手前にユーザーへ確認する。
- **インストーラーの電源プロファイル選択機能(2026-07-31追記、ユーザー
  指示、エコシステム全体の標準方針)**: `install.sh`/`install.ps1`等の
  インストーラーを持つ全リポジトリは、インストール実行時に以下3つの
  電源プロファイルのいずれかを選択させる(チェックボックス/対話選択):
  1. **省電力(Power-saving)**: CPU使用率・ポーリング間隔を抑えた低負荷
     設定。
  2. **省メモリ(Low-memory)**: メモリ確保量・キャッシュサイズを抑えた
     設定。
  3. **常時電源接続(Always-on)**: 上記の抑制を行わないフル性能設定。
     **このプロファイルを選択した場合のみ**、ハードウェアアクセラレータ
     (NPU/GPU)のサポートを自動検出・自動有効化する(`open-cuda`の
     `GpuDevice`抽象化・ベンダー診断機能を利用、詳細は`open-cuda`の
     CLAUDE.md参照)——省電力/省メモリ選択時はNPU/GPU自動対応を行わない
     (電力・メモリ消費を優先して抑える方針との整合)。
  具体例: `open-redmine`のインストーラーで先行実装予定
  (`open-redmine/CLAUDE.md`参照)。**正直な開示(2026-07-31時点)**:
  この方針はエコシステム全体の標準として記録した段階であり、上記3プロ
  ファイルの実装自体は個々のリポジトリのインストーラーへ順次追加して
  いく必要がある(全リポジトリへの一括実装はスコープが大きいため、
  各リポジトリのCLAUDE.md HANDOFFに次回対応事項として記録し、優先度に
  応じて順次実装する)。実装時は`open-cuda`側のGPU/NPUベンダー検出
  ロジック(既存の`GpuDevice`トレイト・診断機能)を再利用し、車輪の
  再発明を避けること(本節冒頭の「車輪の再発明を避ける」方針に従う)。
- **Androidスマホ・タブレット対応の最優先化(2026-07-31追記、ユーザー
  指示「Androidスマホとタブレット対応は、全てのリポジトリ全ての
  プロジェクトで早急に対応して」)**: GUI/エンドユーザー向けアプリ
  シェルを持つ全リポジトリ(`rs-link-fusion`・`open-easy-web`・
  `open-redmine`・その他将来のプロジェクトを含む)は、Android
  スマホ・タブレット対応を最優先課題として扱う。**正直な開示**:
  「全リポジトリを早急に」という規模は、各リポジトリごとの
  Android NDKクロスビルド検証・APKパッケージング・署名・実機/
  エミュレータでのUI検証が必要であり、1セッションで一括完了できる
  規模ではない。段階的な着手方針として:
  1. 既にAndroid NDKクロスコンパイル自体は実証済みのリポジトリ
     (`open-web-server`・`open-redmine`等、`cargo ndk`でELFバイナリ
     生成まで確認済み——詳細は各リポジトリCLAUDE.md参照)を優先し、
     APKアプリシェル化(フォアグラウンドサービス・電源プロファイル
     連携)へ進める。
  2. まだNDKクロスビルド自体を試していないリポジトリは、まずビルド
     可否の実証から着手する。
  3. 各リポジトリのCLAUDE.md HANDOFFに「Android対応: 現状(NDK
     クロスビルド済み/未確認)・次にすべきこと」を明記し、進捗が
     追跡できるようにする。
- **GUIを持つ全リポジトリに「省機能+省メモリ版に切替」ボタンを設置する
  (2026-07-31追記、ユーザー指示「全てのリポジトリ、全てのプロジェクトの
  GUIに省機能、省メモリ版に切替えるボタンを付けて」)**: `open-easy-web`で
  先行実装したパターン(`open-web-server`由来の`power_profile.rs`——
  省メモリ/省電力/常時電源接続の組み合わせ選択可能な電源プロファイル
  APIに加え、GUI側で「省機能」ボタンを押すと非必須セクションを
  `localStorage`永続化付きでDOM非表示化する)を標準テンプレートとする。
  **正直な開示**: 「全リポジトリ・全プロジェクトへ即座に」という規模は
  1セッションで一括対応できるものではない。段階的な着手方針:
  1. 既にWebフロントエンド(WASM GUI)を持つリポジトリ
     (`open-easy-web`実装済み、`open-redmine`・`rs-link-fusion`等)を
     優先する。バックエンドのみでフロントエンドを持たないリポジトリ
     (`open-web-server`本体等)は対象外(GUIが無いため「GUIに設置」
     という要求自体が適用されない)。
  2. 各リポジトリの実装は、(a) バックエンド側に電源プロファイルAPI
     (`power_profile.rs`、無ければ`open-web-server`/`open-easy-web`の
     実装をそのまま移植、無くても実効果が無いリポジトリ〈バックグラウンド
     ポーリングループを持たないもの〉は省略してよい——`open-redmine`/
     `open-gitea`はこのケース)、(b) フロントエンド側のUI形式は**2026-08-01
     改定(ユーザー指示「省メモリ、常時電源接続などのチェックボックスと
     ボタンにして」)により、排他的な3ボタン方式から以下へ変更**:
     省電力/省メモリ/常時電源接続を**独立したチェックボックス**として
     自由に組み合わせ可能にし(`open-easy-web`の`PowerProfileFlags`と
     同じ設計、「通常」は3つとも未チェック)、「省機能表示に切替」
     「全機能を復元」はボタンのまま据え置く。実効果が無いリポジトリでも
     チェックボックス自体は用意し(将来ポーリング処理を追加した際に
     すぐ使えるUI規約として先行導入)、CLAUDE.mdに「現時点でどの軸に
     実効果があるか」を正直に明記する(`open-redmine`/`open-gitea`の
     実装例参照——GitHubコミット/Wiki一覧の自動取得を省電力・省メモリの
     いずれかで止め、常時電源接続がその抑制を上書きする、という
     1軸だけに実効果を絞った例)。(c) 「省機能」時に非表示にする
     非必須セクションのプロジェクト固有の線引き(各リポジトリで判断し、
     CLAUDE.mdに理由を明記)。
  3. 各リポジトリのCLAUDE.md HANDOFFに実装状況(未着手/実装済み)を
     記録し、進捗を追跡できるようにする。2026-08-01時点の実装済み
     リポジトリ(いずれもチェックボックス方式・本番デプロイ済み):
     `open-easy-web`・`open-redmine`・`open-gitea`。

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
(書き込み側・読み出し側とも実装済み——2026-07-27に`aruaru-db`側で
単一行のみだったcommit_id指定クエリをフルテーブルスキャン〈`WHERE`無し〉
にも対応、詳細は`aruaru-db/CLAUDE.md`の同名HANDOFF参照)、
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
- **日英Web検索の結果、CockroachDB/TiKV等の最先端の実運用システムが
  既に対応済みと判明した技術的ギャップは、「今のところは大丈夫」という
  報告に留めず、確認を求めず自動でそのまま実装に着手すること**
  (ユーザー指示、2026-07-23)。「未着手だから今回は見送る」という判断を
  ユーザーへの確認なしに下してはならない、という既存の運用ルール
  (2026-07-12)の延長——最先端システムが既に解決している設計上のギャップを
  「将来の課題」として先送りする態度そのものが間違っている、という
  ユーザーの明確な指摘に基づく。具体例: aruaru-dbのRaft合意が単一グループ
  のままで将来のスケール限界になり得ると分かった際、CockroachDB/TiKVの
  Multi-Raft(Range単位の独立したRaftグループ)方式へ実際に追従する
  実装に自動着手した(詳細はaruaru-db側CLAUDE.md HANDOFF参照)。
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

### 2026-08-01(続き2) 実バグ発見・修正: Web管理UIも絶対パスfetch罠を踏んでいた

`aruaru-db`側で同種のバグ(`fetch('/api/status')`が`path_prefix`マウント
配下で常にオリジン直下を叩いてしまう)を発見・修正した直後、本リポジトリの
`web/src/main.rs`にも**全く同じバグが実際に存在する**ことを発見した
(リポジトリ横断で似た構造のコードを点検する過程で発覚)。直前エントリの
コメントには「絶対パスでfetchするため相対パス起因の問題は起きない」と
書かれていたが、これは誤り——絶対パスであること自体が
`path_prefix`マウント環境での問題の原因だった(rs-syncで過去に踏んだ
「相対パス+末尾スラッシュ無し」問題とは別種の罠を混同していた)。

**実機確認**: `https://easy-web.tokyo/open-raid-z/`を実ブラウザで開いた
ところ、プール状態が`{"error":"not found"}`と表示されることを確認
(修正前)。`OPEN_RAID_Z_WEB_BASE_PATH`環境変数(既定は空文字列)を追加し、
`aruaru-db`と同じ手法(ページのJSへ`const BASE_PATH`として埋め込み)で
修正。本番デプロイ後、実ブラウザで実際のプール状態
(`{"level":"Z2","total_stripes":4000,...}`)が正しく表示され、
コンソールエラーが無いことを確認済み。

**横展開の点検**: リポジトリ全体を`fetch('/api` / `fetch("/api`等の
パターンで検索し、他に同種のバグが無いか確認した。`aruaru.tokyo/
src/main.rs`にも同じパターンが見つかったが、そちらは`path_prefix`無しで
自身のドメイン(`aruaru.tokyo`)のルートに直接マウントされているため
該当しない(`domains.toml`で確認済み)——実際にバグだったのは
`open-raid-z`/`aruaru-db`の2件のみ。
- 次にすべきこと: 特に緊急の課題は無し。今後、`path_prefix`付きで
  デプロイするWeb UIを新設する際は、必ずこのBASE_PATHパターンを
  最初から組み込むこと(この教訓を`PORTING.md`にも記録)。

### 2026-08-01 積み残しバックログ3件を消化: GPU接続の実配線+実測ベンチマーク+Web管理UIの実VPSデプロイ(ユーザー指示「Aを進めて」への対応)

以前のHANDOFF(下記2026-07-30系列の各エントリ)で繰り返し「次にすべき
こと」として残っていた3項目に対応した。

1. **実バグ発見・修正: GPU実装済みだが誰からも呼ばれていなかった
   (死んだコード)**: `RaidZVdev`/`vdev.rs`にはP-parity(XOR)・Q-parity
   (Reed-Solomon)ともGPU実装・実機検証済みの`with_accelerator`経路が
   既に存在していたが、本リポジトリで唯一の実運用エントリポイントである
   `orzctl`(`create`/`status`/`mount`×2の4サブコマンド)はいずれも素の
   `RaidZVdev::new(...)`しか呼んでおらず、実機でGPUアクセラレータが
   一度も使われていなかった。全4箇所を共通の`build_vdev()`経由に
   一本化し、実際に接続した。
2. **実測ベンチマークを経た設計修正(誠実なプロセス)**: 上記1をまず
   「GPUが検出できれば常に使う」設計で入れた直後、新設
   `examples/raidz2_parity_benchmark.rs`(データ3〜6本+2パリティ、
   128KiBチャンク×200ストライプ、NVMe 4〜8枚構成を模擬)で実測した
   ところ、**この環境ではGPU版がCPU版よりおよそ9〜14倍遅い**
   (CPU 495〜794ms・GPU 6933〜7140ms)ことが判明した——1ストライプ
   ごとに個別のGPUディスパッチ(コマンドバッファ構築・同期待ち)を行う
   現在の実装粒度では、GPU側の固定オーバーヘッドが計算時間そのものを
   上回るため。「実装した・検出できた」だけでGPUを既定にするのは
   実運用での性能退行であり、これを実測せずに「GPU接続完了」と報告
   するのは不誠実と判断し、**既定をCPUへ戻し、`orzctl`に新設した
   `--accel <cpu|gpu>`オプション(既定`cpu`)で明示指定した場合のみ
   GPUを使う実験的機能**という設計に修正した。
3. **付随して発見・修正した実バグ: Linux(`fuse_backend`)ビルドが
   壊れていた**: VPS(Linux)でのデプロイ作業中に
   `cargo build --no-default-features --features fuse_backend,json_status`
   (READMEに明記された非Windows向けビルドコマンド)がコンパイルエラーに
   なることを発見。`src/fuse_mount.rs::errno_from_bridge_error()`が
   `BridgeError::JournalFailed`/`OffsiteBackupFailed`(ディザスタリカバリ
   関連で後から追加された2バリアント)を網羅しておらず、Windows側の
   `mount.rs`には同じ修正が既に入っていた(片方だけ追従漏れ)。通常の
   開発がこのWindows機で行われ`mount.rs`だけがコンパイル対象になるため、
   この回帰はこれまで誰にも検出されていなかった。2行追加して解消。
4. **Web管理UIを実際にVPSへデプロイ(`https://easy-web.tokyo/open-raid-z/`)**:
   `web/`crate(前回2026-07-30続き3で実装、ローカル検証のみで本番未反映
   だった)をVPS上でビルド(`gpu_accel`はビルド時に`dxc`必須のためVPSでは
   無効化——GPUは既定オフになったため実害無し)。ループバックファイル
   5枚(3データ+2パリティ、64MB×5)で実際にZ2プールを作成し
   (`orzctl create`、CPU経路)、systemdサービス`open-raid-z-web.service`
   (port 8110)として常駐化。`open-web-server`の「分身の術」テナント
   登録(`POST /admin/tenants`、`path_prefix=/open-raid-z`)で
   `easy-web.tokyo`へ接続。
5. **検証(実測)**: `cargo test --release`(Windows、既定feature)・
   `cargo test --release --no-default-features --features
   fuse_backend,json_status`(VPS、Linux)ともに全green(回帰無し)。
   実際に`orzctl create`(`--accel`未指定)の標準エラー出力に
   「パリティ計算はCPUで行います」ログが出ること、`--accel gpu`指定時は
   このWindows開発機で実際に`ハードウェアアクセラレータを使用します:
   Gpu`と出ること(実GPU上で実際にディスパッチされていることの直接
   証拠)を確認。本番では`curl https://easy-web.tokyo/open-raid-z/`→
   `200`、`curl https://easy-web.tokyo/open-raid-z/api/status`が実際の
   プール状態(`{"level":"Z2","total_stripes":4000,...,"datasets":
   [{"name":"demo-dataset",...}]}`)を返すことを確認済み。
6. **正直な開示・未着手**: (a) `/open-raid-z/demo`という別テナントは
   今回登録していない——`GET /api/status`自体が元々認証不要(管理者
   トークンが必須なのは`POST /api/create`のみ)であり、`OPEN_RAID_Z_
   READ_ONLY`はプロセス全体に効くフラグのため「本番と別のread-only
   デモ」を同じプロセスから区別して提供する意味が薄いと判断した
   (他リポジトリのdemoが軒並み「本番と同一バックエンドのエイリアス」
   に留まっているのと同じ実情)。(b) VPS上での`gpu_accel`(実際の
   GPU/NPU検出)は`dxc`未導入のため無効化したまま——今回のベンチマーク
   結果からもGPUを既定にする理由が無いため、意図的にこのままとした。
   (c) 複数ドメインでのバイナリ共有・`aruaru-db`側同種UIは引き続き
   未着手。
- 次にすべきこと: 特に緊急の課題は無し。GPU経路をより粗い粒度
  (複数ストライプの一括ディスパッチ等)で再実装すれば結果が変わる
  可能性があるが、現時点ではスコープ外。

### 2026-07-30 構想メモ: NVMe RAID6(4〜8枚)のランダムアクセス低速化対策としてのGPUパリティアクセラレータ(未実装、ロードマップとして記録)

ユーザーから、open-web-server・RPoem・open-raid-z(ZFS互換)・aruaru-db
(ACID互換)による4層4重通信+DATABASE対応(オンライン証券・オンライン
クレジットカード決済等、データ紛失が許されないミッションクリティカル
用途)の文脈で、以下の追加要望が示された。**今回は構想・調査結果の
記録のみで、実装には着手していない**(スコープが大きく、GPU側
[open-directx/open-cuda]との協調設計が必要なため)。

**課題**: NVMe SSD 4〜8枚でRAID6を構成する場合、1回のランダム書き込み
あたりパリティの読み込み・再計算が2回ずつ発生する
(Read-Modify-Write)ため、NVMe本来の性能(数百万IOPS)がボトルネック
化する。

**ユーザーが提示した解決策候補(整理・記録)**:
1. **GPU/専用ASIC搭載の次世代RAID**(例: Graid SupremeRAID方式):
   GPUをパリティ計算のアクセラレータとして使い、CPUボトルネックを
   排除する。本リポジトリの文脈では、**`open-directx`/`open-cuda`の
   ハードウェアアクセラレータ抽象化(`AccelBackend`: Cpu/Gpu/Npu/
   HardwareAccelerator、`open-web-server-wire::accel`で既に型として
   先取り済み)を使い、RAID6のXOR/Reed-Solomonパリティ計算をGPU
   カーネルとして実装する**、という方向性がユーザーの要望に対応する
   最も具体的な実装候補。
2. **ZFS(RAID-Z2)+高速SLOG(ZIL)**: 本リポジトリ自体がZFS互換
   (RAID-Z2/Z3相当)を謳っているため、この方向性は既存アーキテクチャ
   との親和性が高い。Copy-on-Writeによるランダム書き込みの最適化+
   SLOG(高耐久・低レイテンシNVMeを同期書き込み用ログとして前段配置)
   という組み合わせ。
3. **ハードウェアRAIDカードのWrite-Backキャッシュ化**(BBU/フラッシュ
   保護必須)——本リポジトリがソフトウェア実装である以上、直接の
   実装対象ではないが、実運用時の補完策として記録。

**ユーザーが要望する実装範囲(クロスプラットフォーム)**: Windows・
Linux・macOS・iOS/iPadOS・Android(スマホ/タブレット)の各プラット
フォーム、かつNVIDIA/AMD/Intel各GPUベンダーでの互換性。

**現状の技術的土台の棚卸し(このパスで確認、コード変更なし)**:
- `open-directx`の`directx-graphics-vulkan`クレートは実際に
  Vulkan(`ash`)経由でGPU描画パイプラインを実装済み(2026-07-30時点、
  同日のHANDOFF参照)——**Compute Shader経路(XOR/Reed-Solomon演算に
  使うべきパイプライン)は`opencuda-vulkan`側に既にある**とされている
  (`open-cuda`のDXBC/DXIL Compute実機テスト群)。Vulkanはプラット
  フォーム非依存(Windows/Linux/Android/macOS〈MoltenVK経由〉)かつ
  ベンダー非依存(NVIDIA/AMD/Intelいずれも標準対応)であるため、
  「Windows限定のDirectX」ではなく「Vulkan経由でDirectX互換の体験を
  クロスプラットフォームに提供する」という本エコシステムの既存方針
  (`open-directx`の設計思想そのもの)と、今回の要望(GPU RAID6を
  多様なプラットフォームで動かしたい)は方向性が一致している。
  iOS/iPadOSはVulkanネイティブ非対応でMoltenVK経由になる点は
  `open-directx`側の既存の正直な開示と同じ制約を引き継ぐ。
- 本リポジトリ(open-raid-z)側には現時点でRAID6のパリティ計算を
  GPUへオフロードする仕組みは一切無い(CPU実装のみ、要再確認だが
  このパスでは実装箇所の特定調査までは行っていない)。

**次回セッションで着手すべき第一段階(提案、確認なしに着手可)**:
1. `open-cuda`(or `open-directx`)側に、RAID6パリティ計算
   (Galois体上のXOR/Reed-Solomon符号化・復号)専用のCompute
   Shaderカーネルを実装し、CPU参照実装との数値一致を実機で検証する
   (このリポジトリのopen-directx側`indexed_scene_with_depth_and_read_back`
   等、既存の「実GPU上で実際に検証する」開発パターンをそのまま踏襲)。
2. `open-raid-z`側の既存パリティ計算箇所を特定し、上記GPUカーネルを
   `AccelBackend::Gpu`実装として接続する(`Cpu`へのフォールバックは
   既存方針通り安全に維持)。
3. 4〜8枚のNVMe構成を模したベンチマーク(実SSDが無ければRAMディスク/
   ファイルベースの疑似ドライブでも、CPU実装 vs GPU実装のIOPS比較
   自体は意味を持つ)で効果を実測する。
- 次にすべきこと: 上記1から実際に着手する(次回セッション)。

### 2026-07-30 上記「第一段階」の(1)着手・実機検証完了(P-parity/XORのみ)

`open-cuda`側で実装・実機検証を行った(詳細は`open-cuda/CLAUDE.md`の
2026-07-30エントリ参照)。要点:

1. `opencuda-vulkan::real::VulkanDevice`に`raid6_xor_parity`カーネルを
   追加(既存の`vector_add`/`matmul`と同じ`dispatch_spirv`共通経路を
   再利用)。N本のデータディスクを1本の連結バッファとしてバインドする
   設計(可変ディスク本数をシェーダの固定バインディング数で扱うため)。
2. 新規example crate`raid6_xor_parity_vulkan_real`(CPU参照実装・CPU版・
   実Vulkan版の3経路を同一入力で実行しbit-exact一致を検証)を、実機
   (NVIDIA GeForce GT 730)で実際に実行し、3経路すべて一致することを
   確認済み(型チェックのみで完了と報告しない方針を徹底)。
3. **正直な開示・残作業**: P-parity(XOR)のみで、Q-parity
   (Reed-Solomon、GF(2^8))は未実装(実装難度が高く数値検証が困難な
   ため、意図的に別増分として切り出した)。また本リポジトリ
   (`open-raid-z`)側の実パリティ計算経路への統合(上記提案の(2))・
   実ベンチマーク(上記提案の(3))は**まだ未着手**。
- 次にすべきこと: (1) Q-parity(Reed-Solomon)のGPU実装、(2) 本リポジトリの
  実パリティ計算箇所を特定し`AccelBackend::Gpu`実装として接続、
  (3) 4〜8枚のNVMe構成を模したベンチマーク。

### 2026-07-30(続き) 上記(1)Q-parity(Reed-Solomon)のGPU実装も完了

ユーザー指示「Q-parityは必要で重要なので必ずGoogleで日本語と英語で設計
方法と実装方法を検索して調査して開発実装して」に基づき、`open-cuda`側で
Q-parityも実装・実機検証済み(詳細は`open-cuda/CLAUDE.md`の2026-07-30
続きエントリ参照)。Linuxカーネル/H. Peter Anvin論文と同じ標準方式
(生成元`0x02`、既約多項式`0x11D`)を日英Web検索で裏取りした上で実装し、
実機(GT 730)でCPU参照実装とbit-exact一致することを確認した。これで
RAID6のP-parity・Q-parity両方がGPU実装・実機検証済みとなった。

**正直な残作業(変更なし)**: 本リポジトリ(`open-raid-z`)側の実パリティ
計算経路への統合、実ブロックサイズでのベンチマークは依然未着手。
- 次にすべきこと: (1) 本リポジトリの実パリティ計算箇所を特定し
  `AccelBackend::Gpu`実装として接続、(2) 4〜8枚のNVMe構成を模した
  ベンチマーク、(3)(open-cuda側の別項目)Poly1305認証タグのGPU実装。

### 2026-07-30(続き2) Poly1305のGPU実装完了(open-cuda側)+ 安全なアンインストーラーを新設(uninstall.sh/uninstall.ps1)

1. **Poly1305**: `opencuda-directx`側でPoly1305認証タグのGPU実装が完了・
   実機検証済み(詳細は`open-cuda/CLAUDE.md`参照)。これでChaCha20+
   Poly1305の両方がGPU実装・実機検証済みとなった。

2. **アンインストーラー新設(ユーザー指示「別バージョンをインストール
   し直す/アンインストールする際に既存データやHDDのデータへ悪影響を
   与えないように」への対応)**: 従来`install.sh`/`install.ps1`のみで
   アンインストーラーが存在しなかった。新規に[uninstall.sh](uninstall.sh)/
   [uninstall.ps1](uninstall.ps1)を追加した。
   - **安全性の設計**: `orzctl`バイナリはプールのデータ(実ディスク上の
     RAID Z2/Z3構成)とは完全に分離された場所(`/usr/local/bin`、
     `C:\Program Files\open-raid-z`)にのみ存在するため、アンインストール
     スクリプトが削除するのはこのバイナリ(とWindows版はPATHエントリ)
     のみで、プールのデータには一切触れない設計にした——そもそも
     このリポジトリのインストーラー自体がデータディレクトリを持たない
     設計(バイナリ配置のみ)だったため、データ保護という観点では
     本質的に安全だったが、明示的なアンインストーラーが無かったこと
     自体がユーザーの不安要素だったため今回追加した。
   - Linux版は、マウント中と思われるプールが検出された場合に警告+
     確認を挟む(データそのものの削除可否判断には使わない、あくまで
     利便性の注意喚起)。
   - README.mdにアンインストール手順の案内を追加。
3. **正直な開示**: `aruaru-db`(実データディレクトリを持つ)側の
   アンインストーラーも同時に新設したが、詳細は`aruaru-db/CLAUDE.md`
   参照(このリポジトリより安全性設計の比重が大きい——データ
   ディレクトリを意図的に削除しない、という明示的な設計判断が必要
   だったため)。
- 次にすべきこと: (1) open-raid-z本体の実パリティ計算箇所へのGPU接続
  (変更なし)、(2) ベンチマーク(変更なし)、(3) ~~Web管理UIデモ~~
  **着手・実機検証済み(下記2026-07-30続き3エントリ参照)**。

### 2026-07-30(続き3) `orzctl status`サブコマンド新設 + Web管理UI(RPoem)を新規実装・実機検証

ユーザー指示「Rust + RPoem(tokio/hyper直接実装)」でWeb管理UIの技術
スタックを確定。着手前に`orzctl`(`open_raid_z_core`のCLI)の既存
サブコマンドを確認したところ`create`/`mount`/`foreign`のみで、稼働中
プールの状態を機械可読に問い合わせる手段が一切無いことが判明したため、
まずCLI側にその手段を追加してからWeb UIを構築した。

1. **`orzctl status`新設**(`open_runo_zfs_source/open_raid_z_core/src/
   bin/orzctl.rs`): 保存済みプールを開き(マウントはしない)、
   `Pool::usage()`(ストライプ使用状況)・`dataset_names()`/
   `dataset_size()`(データセット一覧)をJSONで出力。JSON組み立ては
   当初手書き文字列だったが、ユーザーから「rust-json RJSONは導入
   してますか?」と問われたのを機に、エコシステム共通のJSON層
   (`RS-JSON`、`aruaru-db`/`open-gitea`等と同じpath依存)の
   `to_string_strict`へ置き換えた(新規`json_status`feature、既定ON、
   無効時は`create`/`mount`のみのビルドを維持するフォールバック付き)。
   実際にloopbackファイル4枚でプール作成→`orzctl status`照会まで実行し、
   実データを反映したJSON(`{"level":"Z2",...,"datasets":[...]}`)が
   返ることを確認済み。
2. **Web管理UI(`open-raid-z/web/`、新規独立クレート)**: RPoem
   (`F:\runo\RPoem\crates\open-runo-poem-compat`)へのpath依存のみ、
   `poem`/`tauri`パッケージへの直接依存は無し。`orzctl`をサブプロセス
   として呼び出し、`status`のJSON出力をそのまま中継する設計
   (プール操作ロジック自体はCLI側に集約、Web層は薄いラッパーに
   徹する)。
   - `GET /`・`GET /demo`: 同じ静的ページ(read-onlyバナーの有無のみ
     差異)。rs-syncで発生した「相対パス+末尾スラッシュ無し」問題を
     教訓に、JS側は常に絶対パス(`/api/...`)でfetchする設計にした
     ——URL構造に依存する曖昧さが構造的に発生しない。
   - `GET /api/status`: `orzctl status`の出力をそのまま中継。
   - `POST /api/create`: 管理者トークン(`OPEN_RAID_Z_ADMIN_TOKEN`
     環境変数、`X-Admin-Token`ヘッダで照合)必須。
   - **read-onlyデモの多層防御(rs-syncの`ReadOnlyGuard`と同じ設計
     思想)**: `OPEN_RAID_Z_READ_ONLY=1`環境変数が設定されている場合、
     `POST /api/create`は**正しい管理者トークンを提示していても**
     常に403で拒否する(UIのフォーム非表示だけに頼らない、サーバー側
     での確実な強制)。
   - **正直なスコープの限界**: `orzctl mount`はフォアグラウンドで
     FUSE/WinFspループをブロックする一発コマンドのため、Webリクエスト
     として呼び出すとハングする——今回は`status`/`create`のみに限定し、
     マウント操作はスコープ外とした(次回以降の課題)。
3. **実機検証(型チェックのみで完了と報告しない方針を徹底)**:
   実際にloopbackファイルでプールを作成し、Webサーバーを起動して
   (a) `GET /api/status`が実プール状態を返す、(b) 管理者トークン無しの
   `POST /api/create`が401、(c) 正しいトークンでの`POST /api/create`が
   実際に新しいプールを作成し`status`にも反映される、(d)
   `OPEN_RAID_Z_READ_ONLY=1`設定時は**正しいトークンでも**403で拒否
   される、(e) `/demo`ページに読み取り専用デモの日本語バナーが含まれる、
   の5点をすべて実際のHTTPリクエストで確認した。
4. **正直な開示・未着手**: (a) VPS(`easy-web.tokyo`)への実デプロイ・
   nginx設定・ドメイン下でのリバースプロキシ配線は今回未実施
   (ローカルでの実機能検証のみ)、(b) 複数ドメインでのバイナリ共有
   (「分身の術」構成)は今回のスコープに含めていない——単一プロセスが
   複数ホスト名を動的に振り分ける仕組み(RPoem側の
   `SharedDispatcher`/`appserver_tenants`)との統合は次回の課題、
   (c) `aruaru-db`側の同種Web管理UIは今回未着手。
- 次にすべきこと: (1) `easy-web.tokyo/open-raid-z`・
  `easy-web.tokyo/open-raid-z/demo`としての実VPSデプロイ(nginx
  reverse proxy設定、`systemd`サービス化)、(2) 複数ドメインでの
  バイナリ共有(RPoemの`SharedDispatcher`経由)、(3) `aruaru-db`側の
  同種Web管理UI新規構築。

### 2026-07-27(続き) チェックポイント項目2〜4を消化

1. **`accdc2c7bcd9e2a60`(お勧めLLMダウンロード機能)は完了済みと確認**:
   `aruaru-llm`の`git status`/`cargo test`を確認したところ、ハードウェア
   検出(`src/hardware.rs`)・`GET /v1/recommend`・`POST /v1/recommend-and-download`・
   `static/index.html`のUIまで実装済みで42件全testがgreenだった。ユーザーからの
   追加指示(「一つ大きなモデルをダウンロードする」/「一つ小さなモデルを
   ダウロードする」ボタン追加)にも対応し、`model_catalog::next_larger`/
   `next_smaller`+`POST /v1/download-larger`/`download-smaller`エンドポイントを
   実装。`cargo test`42件全green(新規2件含む)を確認後、コミット`019d2e2`で
   push済み。
2. **`aruaru-db`の未コミット変更を精査・コミット**: GraphQL
   `cluster_propose` resolverが`RaftWriter`(Raftコンセンサス+
   disaster-backup配線)を迂回し`engine.execute`へ直接書き込んでいた
   ギャップを解消する変更だった(`replicator: Option<Arc<dyn
   ReplicatedWriter>>`を`AdminCtx`へ注入、設定時は`propose_and_wait`
   経由、未設定時のみ`raft_fallback_no_replicator`として明示フォール
   バック)。`cargo build`成功、新規テスト2件(`cluster_propose_tests`)を
   含めビルド・実行して確認後、コミット`c68ed6e`でpush済み。
3. **`open-runo`(旧open-cosmo)側の内部ドキュメントの自称表記確認**:
   `.md`/`.rs`/`.toml`全体を`grep -rn "open-cosmo"`で検索した結果、
   ヒットは0件——別セッションが既に修正済みだったと判明(作業ツリーも
   `git status`でクリーン)。この項目は**解消済み・追加対応不要**。
4. **`open-web-server`は不干渉のまま**: `git status`で電源プロファイル
   (`android/`配下)・`crossrepo_backup.rs`等、別セッションによる未コミット
   の進行中変更を確認したため、深夜自動アップデート機能の実装は今回も
   見送った(前回エントリの判断を継続)。

### 2026-07-27セッション末尾チェックポイント(リミット接近のため記録)

**このセッションで完了したこと**(直前のエントリの詳細も参照):
1. `open-web-server`のTLS証明書永続化恒久修正をVPSへデプロイ完了
   (コミット`e21d871`)。21ドメイン中20ドメインは復旧・恒久化済み。
2. **karu.tokyoのみレート制限待ち**——`open-raid-z/CLAUDE.md`
   (このファイル、直下のエントリ)に記載の1コマンドを
   **2026-07-28 10:12:54 UTC以降に1回だけ**実行して復旧させること。
   それより前には絶対に試さないこと(制限がさらに延びる)。
3. `open-directx`・`open-cuda`・`aruaru-llm`の「お勧めLLMをダウンロード」
   ボタン機能(ハードウェア検出→推奨モデルサイズ判定→自動ダウンロード
   →生成テスト)を実装するバックグラウンドタスクを起動済み
   (agentId `accdc2c7bcd9e2a60`)。**セッション終了時点で完了報告は
   未受領**——次回セッション開始時、`aruaru-llm`の`git status`/
   `git log`で進捗を確認し、未完了なら続きを再開すること。
4. `aruaru-db`に前セッションから未コミットの変更
   (`crates/aruaru-graphql/src/admin_resolvers.rs`等、SET全体の自動
   同期バックアップ関連)が残っている——次回セッション開始時に内容を
   精査し、ビルド・テストが通れば意味のある単位でコミットすること。

**次回セッション開始時に最優先で確認すべきこと**:
1. **karu.tokyoの復旧**(2026-07-28 10:12:54 UTC以降、1回だけ)。
2. `accdc2c7bcd9e2a60`(お勧めLLMダウンロード機能)の完了状況を
   `git status`で確認し、未完了なら再開。
3. `aruaru-db`の未コミット変更の精査・コミット。
4. `open-runo`(旧open-cosmo)側の内部ドキュメントの自称表記確認
   (前々回チェックポイントから継続、まだ未着手)。

### 2026-07-27 karu.tokyo TLS証明書、恒久修正デプロイと引き換えに再度レート制限に抵触
(前回チェックポイント「2026-07-26緊急引き継ぎ」の続き)

**完了**: `open-web-server`のTLS証明書永続化の恒久修正
(`OPEN_WEB_SERVER_TLS_CERT_DIR`、コミット`e21d871`)をVPSへデプロイ済み。
`disaster_email_backup` featureが新たに`open-raid-z`へのpath依存を
追加していたため、VPS側にも`/root/open-raid-z`をclone(浅いclone、
ビルド専用)。`acme,ddns,sftp,upnp,disaster_email_backup` featureを
指定してビルドし直す必要があった(featureを付け忘れた初回ビルドでは
`/admin/tenants/:host/tls/acme`が404になる実バグを踏んだ、原因は
featureゲートの見落とし)。

**21ドメイン中20ドメインは証明書取得・HTTPS復旧・永続化まで完了**
(aon.tokyo・www.aon.tokyo・aon.co.jp・www.aon.co.jp・e-gov.info・
www.e-gov.info・easyweb.tokyo・www.easyweb.tokyo・easy-web.tokyo・
www.easy-web.tokyo・www.karu.tokyo・nasa.tokyo・www.nasa.tokyo・
icpo.tokyo・www.icpo.tokyo・runo.tokyo・www.runo.tokyo・aruaru.tokyo・
fbi.tokyo・www.fbi.tokyo)。

**karu.tokyoのみ現在HTTPS応答不能(`curl`で`000`を確認)**。原因は
このセッション自身の復旧作業(緊急単体取得→ビルド修正のための2回目の
再起動→一括再取得)で短時間に複数回karu.tokyoの証明書取得を試みて
しまい、Let's Encryptの「同一ドメイン集合への証明書発行は168時間に
5件まで」レート制限に到達したこと。**次回取得可能時刻:
2026-07-28 10:12:54 UTC**。この時刻以降、以下のコマンドを1回だけ
実行して復旧させること(解除前に何度も試すと制限がさらに延びるため
絶対に厳禁):
```
curl -X POST http://127.0.0.1:80/admin/tenants/karu.tokyo/tls/acme \
  -H "x-admin-token: <systemdユニットのOPEN_WEB_SERVER_ADMIN_TOKEN>" \
  -H "Content-Type: application/json" \
  -d '{"directory_url":"https://acme-v02.api.letsencrypt.org/directory","contact_email":"norukia.jp@gmail.com"}'
```

**今後の教訓**: 今回の恒久修正(証明書のディスク永続化)により、
**次回以降はプロセス再起動をしても証明書は消えない**——今回発生した
ような「再起動のたびに全ドメイン再取得が必要になり、レート制限を
消費する」という問題自体が今後は起きないはず。ただし今回はその修正を
デプロイする過程で(featureの付け忘れによる想定外の追加再起動を含め)
複数回再起動してしまい、皮肉にも修正のデプロイ自体が新たなレート制限
到達を招いた。次回、同様の恒久修正をデプロイする際は、ビルド前に
必要なfeatureフラグを`Cargo.toml`の`[features]`節で確実に確認して
から`cargo build`することを徹底する。

### 2026-07-24(旧) 緊急引き継ぎ内容は上記で解消済み(参考として下記に残置)



- **2026-07-26 SFTP退避先(`offsite_backup::SftpBackupTarget`)のホスト鍵
  検証をTOFU(Trust On First Use)方式で実装——`check_server_key`が常に
  `Ok(true)`を返すだけだった既知の未検証項目(CLAUDE.mdに以前から正直に
  記録されていた制約)を解消(ユーザー指示: runo.tokyo/open-directx/
  open-cuda/aruaru-llm等7リポジトリの未着手・未完成事項の洗い出し→実装
  継続、SETバックアップ系の実接続配線の一環として着手)**:
  1. **`src/offsite_backup.rs`に`SftpBackupTargetConfig::known_hosts_path:
     Option<PathBuf>`を新設**(`#[serde(default)]`、未設定なら従来どおり
     検証しない後方互換)。`SftpPasswordAuthHandler`に`host_key`
     (`"host:port"`)と`known_hosts_path`を持たせ、`check_server_key`を
     実装: (a) 未設定なら無条件Trust(既存動作)、(b)
     `russh::keys::PublicKey::to_openssh()`でOpenSSH形式へエンコードし、
     独自の簡易known_hostsファイル(`mod known_hosts`、
     `"host:port <openssh鍵>"`を1行ずつ記録するテキスト形式)を検索、
     (c) 未記録なら無条件で信頼して追記(TOFU)、(d) 記録済みかつ一致なら
     許可、(e) 記録済みだが不一致なら`Ok(false)`で接続そのものを拒否
     (中間者攻撃・DNSスプーフィング対策、`tracing::error!`で警告ログ)。
  2. **呼び出し側3箇所を新フィールドに対応**: `open-raid-z`自身の
     `tests/offsite_backup_integration.rs`、`open-easy-web/server/src/
     dist_sync.rs`の`build_manager`/`sftp_target_configs`(こちらは
     `.values()`から`.iter()`へ変更し、分散同期先のUUID(`id`)ごとに
     `journal_dir/dist-sync-known-hosts/<id>.txt`という独立した
     known_hostsファイルを持たせた——複数VPS間で鍵を混同しないため)。
     `aruaru-db`側は`SftpBackupTargetConfig`の直接構築箇所が無いことを
     `grep`で確認済み(影響なし)。
  3. **新規テスト`sftp_host_key_tofu_trusts_first_connection_and_rejects_a_later_mismatched_key`
     を追加**(`tests/offsite_backup_integration.rs`、実インプロセスrussh
     SFTPサーバーへの実接続で検証、モックの呼び出し回数確認に留めない):
     (1) known_hosts未作成の状態から初回接続が成功し、実際にファイルへ
     `"127.0.0.1:<port> ssh-..."`形式で記録されることを`std::fs::read_to_string`
     で確認、(2) 記録済みの鍵と一致する再接続が成功することを確認、
     (3) known_hostsファイルの記録を意図的に無関係のダミー鍵へ書き換えた
     状態で接続を試み、**実際に接続が拒否される(`Err`が返る)**ことを確認。
  4. **検証(実測)**: `open_raid_z_core`側`cargo test --features
     offsite_backup`が**60(lib)+新規1件を含む既存4件(offsite_backup_integration)
     +他の統合テスト群、全green**(実行結果に`FAILED`無し、新規追加分
     `sftp_host_key_tofu_...`/`sftp_backup_target_full_roundtrip_...`共に
     `ok`)。この変更を利用する`open-easy-web/server`側も`cargo test`
     59件全green(既存の`dist_sync`関連テスト含め回帰なし)。
  5. **正直な開示**: (1) known_hostsファイル自体の破損・並行書き込み時の
     排他制御は今回実装していない(単純な追記/全文読み込みのみ、複数
     プロセスからの同時書き込みは非対応)。(2) 実際のDNSスプーフィング・
     中間者攻撃環境での実地検証は行っていない(ユニットテストでの
     人工的な鍵すり替えシミュレーションのみ)。(3) OpenSSH標準の
     `~/.ssh/known_hosts`形式(ホスト名のハッシュ化等)とは異なる独自簡易
     フォーマットであり、既存のOpenSSHツール(`ssh-keygen -R`等)とは
     互換性が無い。
  - 次にすべきこと: (1) 実SMTPサーバー/実Googleドライブアカウントでの
    E2Eディザスタ退避検証、(2) known_hostsファイルへの排他制御
    (ファイルロック)の追加、(3) 複数プロセス/複数VPS運用時の
    known_hosts管理UIの検討(現状はファイル直接編集のみ)。

- **2026-07-26(お引越し前・緊急チェックポイント) VPS上で実際に発生した
  障害と復旧状況——次回セッション開始時に必ず最初に確認すること**:
  1. **`karu.tokyo`は現在HTTPS証明書が無い状態(復旧目安:
     2026-07-27 00:17:48 UTC頃)**。原因: `open-web-server`のTLS証明書は
     `TenantCertResolver`(`crates/open-web-server-gateway/src/tls.rs`)
     内の`RwLock<HashMap<String, Arc<CertifiedKey>>>`に**メモリ上のみ**
     保持され、ディスクへ永続化されていなかった(今回の調査で判明した
     重大な既知バグ)。本セッション中に`open-web-server`を2回再起動した
     結果、全ドメイン(約20件)のTLS証明書がメモリから消失し、
     `runo.tokyo`含む全ドメインが一時的にHTTPS応答不能になった。
     `/admin/tenants/:host/tls/acme`経由で証明書を再取得し19/20ドメインは
     復旧させたが、`karu.tokyo`だけはLet's Encryptの
     「同一ドメイン集合への証明書発行は168時間に5件まで」という実際の
     レート制限(`too many certificates (5) already issued...`)に
     引っかかり、**次回セッション開始時点でまだ復旧していない可能性が
     高い**。まず`curl -s -o /dev/null -w '%{http_code}' https://karu.tokyo/`
     で確認し、まだ000/エラーなら現在時刻がレート制限解除時刻
     (上記UTC時刻)を過ぎているか確認した上で、
     `POST /admin/tenants/karu.tokyo/tls/acme`(`x-admin-token`ヘッダ、
     ボディ`{"directory_url":"https://acme-v02.api.letsencrypt.org/directory","contact_email":"norukia.jp@gmail.com"}`)
     を1回だけ叩いて復旧させること(解除前に何度も試すと制限がさらに
     延びるため厳禁)。
  2. **重要な運用上の教訓(次回以降、絶対に守ること)**: 上記バグが
     修正される(下記3参照、現在バックグラウンドで実装中)までは、
     **`open-web-server`サービスを安易に再起動しないこと**。設定ファイル
     (`domains.toml`/`web_vhosts.toml`)の変更を反映させたい場合でも、
     再起動は全ドメインのTLS証明書を道連れにする実害があることを
     忘れないこと。真に必要な場合のみ、再起動直後に全ドメイン分の
     証明書再取得を1回で済むよう用意してから実行すること(Let's Encrypt
     レート制限に達したドメインが1つでもあると、それだけ長時間の
     ダウンタイムになる)。
  3. **恒久修正が進行中**: `open-web-server`リポジトリに、証明書を
     ディスクへ永続化し起動時に読み戻す修正をバックグラウンドエージェント
     (`OPEN_WEB_SERVER_TLS_CERT_DIR`環境変数、再起動シミュレーションの
     回帰テスト付き)へ依頼済み。次回セッション開始時、
     `open-web-server`側CLAUDE.mdのHANDOFFを確認し、完了していれば
     VPSへデプロイ(この際も、デプロイ後の再起動1回分のみ証明書再取得が
     必要になる点に注意)。
  4. **`runo.tokyo`のTOPページが一時的に`open-web-server`自身の
     紹介ページにすり替わっていたバグを修正済み**:
     `web_vhosts.toml`の`host = "runo.tokyo"`向け`[[webvhost]]`エントリ
     (`docroot = "/root/open-web-server/site"`)が、`domains.toml`の
     `tenant_router`登録(`127.0.0.1:3000`、実際のruno-tokyoアプリ)より
     優先されて表示されてしまっていた——過去に一度修正されたのと同じ
     既知のバグパターンの再発。`web_vhosts.toml`から該当エントリを削除し
     (VPS上のファイルをバックアップの上で編集)、再起動して修正確認済み
     (`https://runo.tokyo/`が正しくruno-tokyoアプリの内容を返すことを
     確認)。**ローカルリポジトリ側(`F:\runo\open-web-server`)の
     `web_vhosts.toml`相当の設定ファイルには反映していない**——VPS上の
     運用設定ファイルのみの変更のため、次回同様の再発防止策を検討する
     場合は、VPS上のこのファイル自体をgit管理下に置く等の恒久対応も
     視野に入れること。
  5. **`https://runo.tokyo/open-redmine`のルーティングを新規追加**:
     `domains.toml`の`path_prefix = "/RS-Red"`エントリを
     `path_prefix = "/open-redmine"`へ変更(バックエンドは同じ
     `127.0.0.1:8100`、open-redmineの実サービス)。動作確認済み。
  6. **ユーザーから受けたが未着手のフォローアップ依頼**:
     (a) `https://runo.tokyo/`のホームページ本文(あきる野市紹介+
     プロダクト紹介)に`open-redmine`へのリンクを追加してほしい
     (runo.tokyoリポジトリの`src/lib.rs`/`src/meta_index.rs`を編集する
     作業、まだ未着手)、
     (b) `open-web-server`のAndroid版(ARM64/x86_64両対応、電源プロファイル
     省メモリ/省電力/通常/常時電源接続の4種)について、「省メモリ版+
     省電力版」のように**複数プロファイルを組み合わせて選択できる
     ようにしてほしい**という要望(現状は4択から1つを選ぶ排他的設計、
     組み合わせ可能にする設計変更が必要、まだ未着手)。

- **2026-07-26(シャットダウン前チェックポイント) RS-Red→open-redmine改名
  +Redmine機能拡充+電源/省メモリプロファイルのライブ切替+open-directx
  デコーダ一般化、複数リポジトリ横断の到達点まとめ**:
  1. **RS-Red→open-redmine 全面改名完了**: GitHub(`aon-co-jp/RS-Red`→
     既存の空プレースホルダ`aon-co-jp/open-redmine`へ内容移設)・
     ローカルドライブ(`F:\runo\RS-Red`→`F:\runo\open-redmine`、Windows
     側の長いパス+実行中プロセスによるファイルロックで難航したが
     `taskkill`で解消)・VPS(ディレクトリ+systemdサービス名を
     `open-redmine`へ統一、`systemctl is-active`で稼働確認済み)の
     3箇所全て完了。旧GitHubリポジトリ`RS-Red`の削除はユーザー確認待ち。
  2. **open-redmine: 実Redmine機能ギャップの一部を実装(コミット
     `3a0711e`)**: 複数トラッカー種別(Bug/Feature/Support/Task)・
     課題の関連(blocks/duplicates/precedes)・作業時間記録を追加。
     66テスト全green(新規9件)。カバレッジ目安を「2〜3割」から
     「3割程度」へ正直に更新(誇張なし)。詳細はopen-redmine側CLAUDE.md。
  3. **省電力/省メモリプロファイルのセッション中ライブ切替
     (open-web-server、コミット`6e08f2b`)**: Android側はポーリング
     ループが起動時に一度だけプロファイルを読んでいた実バグを発見・
     修正し、再起動無しで即座に反映されるよう修正。Windows/Linux
     デスクトップ側は従来ゼロだったプロファイル概念を新設し、
     管理API(`/admin/power-profile`)経由でDDNS等のポーリング間隔を
     ライブ変更できることを実HTTPテストで確認。ハードウェアアクセラ
     プロファイルの切替のみネイティブプロセス再起動が必要という正直な
     制限を明記。
  4. **aruaru-db: ディザスタバックアップの残り3ギャップを解消
     (コミット`3c4eeec`)**: 真の低速SMTPシナリオでの非ブロッキング
     実証・稼働中RaftWriterへの管理API経由の実注入・RaftNode迂回経路の
     棚卸し(1件発見し配線、1件発見したが今回は未着手のまま正直に
     記録——`aruaru-graphql`の`cluster_propose` resolverがRaftWriterを
     経由しない、単一ノードでは実害小さいが複数ノードでは一貫性が
     崩れる真のバグになり得ると明記)。
  5. **open-directx: DXBCデコーダの一般化+DXILワークグループサイズの
     実メタデータ抽出(コミット`214cb5d`)**: 複数演算・複数一時レジスタ
     を扱う汎用パターンを追加(fxcの最適化—コンポーネント再利用・CSE—を
     ハードコード無しで正しく処理できることを確認)。DXILの
     `numthreads`をメタデータから実際に抽出するよう変更(従来の
     ハードコード`(64,1,1)`を解消、回帰防止テスト付き)。全6件の
     実Vulkanハードウェアテスト(NVIDIA GT 730)含め green、リグレッション
     無し。
  - 次にすべきこと: 各リポジトリ側CLAUDE.mdの「次にすべきこと」参照。
    特にopen-redmineのWASM UI対応・aruaru-dbのGraphQL迂回経路配線・
    RS-Red旧GitHubリポジトリの削除確認、が未着手のまま残っている。

- **2026-07-25(セッション末尾チェックポイント) SET全体の自動同期
  バックアップ+DirectX D3D11/D3D12互換層+GPUベンダー統合、複数リポジトリ
  横断の到達点まとめ(このopen-raid-zのCLAUDE.mdが正本、詳細は各リポジトリ
  側CLAUDE.mdのHANDOFF参照)**:
  1. **SET(open-easy-web/open-web-server/open-raid-z/aruaru-db)の
     スタンドアロン・メール・ディザスタバックアップ、4リポジトリ全て完了**:
     `open_raid_z_core::offsite_backup::EmailBackupTarget`(本リポジトリの
     機能)を各リポジトリが再利用する形で、VPS分散同期の設定なしでも
     単体で動くメールバックアップを実装。`aruaru-db`は実際のRaft
     quorum障害時に自動でメール退避が発火するところまで配線完了
     (`RaftWriter`経由、非ブロッキング、未設定時は既存動作を完全維持)。
     `open-easy-web`は実際のサイトファイル書き込みをバックアップ経路へ
     配線する作業が別セッションで進行中(詳細はopen-easy-web側CLAUDE.md)。
  2. **open-directx: DirectX D3D11(DXBC)/D3D12(DXIL)の両方で、実
     シェーダー→実SPIR-V生成→実Vulkanディスパッチ→CPU参照実装との数値
     一致、を実機(NVIDIA GT 730)で検証済み**。D3D11側は加算/乗算/減算
     (境界チェック付き)/除算の4シェーダー、D3D12/DXIL側はLLVM
     bitstream解析(型テーブル・命令列・Call命令の完全disambiguate)を
     経て同じvector_add形状で到達——両者がパリティに達した。汎用的な
     SM5.0/DXIL全体のデコーダではなく、既知の狭いシェーダー形状のみに
     絞った「狭いが実物」アプローチ(詳細はopen-directx側CLAUDE.md)。
     PlayStation対応は法務リスクを理由に引き続き着手保留。
  3. **open-cuda: GPUベンダー統合の実態を監査・拡張**。実機は
     NVIDIA GT 730のみ(ROCm/oneAPIツールチェーン無し)と確認した上で、
     `GpuVendor`にQualcomm Adreno/ARM Mali/Imagination PowerVRの実
     ベンダーIDを追加。Vulkan Compute自体が既に実装レベルでベンダー
     非依存(ディスパッチコードにベンダー分岐が無い)という統合の実態を
     `OmniGPU-Design.md`に文書化。cuBLAS/rocBLAS/oneMKL専用パスは、
     このマシンでは検証手段が無いため引き続き正直にスタブのまま
     (無理に「完成」を主張しない)。
  4. **aruaru-llm: `opencuda-llm`(実GPT-2 124M重み)を実サービスへ統合
     済み**(`POST /v1/generate`で本格的な自己回帰テキスト生成、
     既存の`opencuda-bert`埋め込み分類とは別エンドポイント)。
  - 次にすべきこと: 各リポジトリ側CLAUDE.mdの「次にすべきこと」を参照。
    特にopen-directxはDXBCデコーダの一般化(複数演算・複数一時レジスタ)
    とDXILワークグループサイズの実メタデータ抽出(現状ハードコード)が
    進行中。

- **2026-07-25 切断耐性/オフサイト退避機能(journal.rs/disaster_recovery.rs/
  offsite_backup.rs/accel.rs)の未検証コミットを検証・実バグ2件を修正・
  完了**: 前回セッションが未コミットのまま残していた「HDD読み書き中の
  電源断・USB/SATA/LAN/WiFi切断耐性」機能一式(書き込みWrite-Ahead
  ジャーナル`journal.rs`、再接続時自動復旧`disaster_recovery.rs`、
  Email/Googleドライブ/SFTPへの切断時退避`offsite_backup.rs`、圧縮の
  CPU/GPU/NPU抽象化`accel.rs`、`tests/offsite_backup_integration.rs`)を
  引き継ぎ検証した。
  1. **ビルドエラーの修正**: `src/mount.rs`の`status_from_bridge_error`が
     新設した`BridgeError::JournalFailed`/`BridgeError::OffsiteBackupFailed`
     の2バリアントを網羅しておらず`cargo build --tests`が失敗していた。
     既存の`MountFailed`/`Io`等と同じcatch-all分岐(STATUS_UNEXPECTED_IO_ERROR
     へマップ)に追加して解消。
  2. **実バグ1: SFTPアップロード後の読み取りが空データを返す**——
     `SftpBackupTarget::upload_segment`が`russh_sftp::client::SftpSession
     ::write()`(高レベル便利関数)をそのまま使っていたが、このメソッドは
     内部で`AsyncWriteExt::write_all`のみ呼び、書き込み確認応答(ack)の
     完了待ちも`SSH_FXP_CLOSE`の送信も行わずに返る(`russh-sftp` 2.3.0の
     `src/client/fs/file.rs`を精読して特定——`write_nowait`はoneshotの
     保留キューに積むだけで`poll_write`は即座に`Ready`を返し、
     `poll_flush`/`poll_shutdown`が呼ばれない限りackを待たない設計)。
     テストの`sftp_backup_target_full_roundtrip_via_inprocess_russh_server`
     が「write→reopen for read→read」の直後に空データ(`got_n=0`)を
     受け取っていたのはこれが原因。**修正**: `open_with_flags(CREATE|
     TRUNCATE|WRITE)`+`write_all`+`AsyncWriteExt::shutdown()`(flush＋
     クローズの完了待ち)を明示的に呼ぶ実装へ変更(`src/offsite_backup.rs`
     の`SftpBackupTarget::upload_segment`)。真の原因はテストのフェイク
     SFTPサーバー側ではなく、本体コード側(`offsite_backup.rs`)の実装
     不備だった。
  3. **実バグ2: モックSMTPテストが「500 unrecognized command」で失敗**——
     `email_backup_target_sends_journal_segment_via_mock_smtp`のフェイク
     SMTPサーバーはEHLO応答で`AUTH LOGIN PLAIN`(両方式)を広告しつつ、
     実装は`AUTH LOGIN`のチャレンジ方式しか処理していなかった。`lettre`
     0.11の`DEFAULT_MECHANISMS = [Mechanism::Plain, Mechanism::Login]`
     (`src/transport/smtp/authentication.rs`)により、サーバーがPLAINも
     対応していると広告している場合、クライアントは**PLAINを優先して
     選ぶ**(単一行の`AUTH PLAIN <base64>`コマンドを送る)。フェイク
     サーバーはこのコマンド形式を認識できず「500 unrecognized command」
     を返していた——本体コード(`offsite_backup.rs`のEmailBackupTarget)側
     に問題は無く、**テストのフェイクサーバー実装が広告と実装を
     一致させていなかった**のが原因。EHLO応答を`AUTH LOGIN`のみの広告に
     修正し解消(`tests/offsite_backup_integration.rs`)。
  4. **デバッグ用`eprintln!`の削除**: 前回セッションが原因調査のために
     フェイクSFTPサーバーの`open`/`write`/`read`ハンドラへ残していた
     デバッグ出力を、修正確認後に削除した(本文には残さない方針どおり)。
  5. **検証結果(実際にテスト実行して確認、他リポジトリのCLAUDE.mdにも
     倣い実測値を記録)**: `cargo build --tests`(デフォルトfeature)成功。
     `cargo test --no-default-features`は108テスト全green(lib単体52
     [journal関連追加込み]+統合56)。`cargo test --no-default-features
     --features offsite_backup`は111テスト全green(上記108+
     `offsite_backup_integration.rs`3件——Email/Googleドライブ/SFTPの
     フルラウンドトリップがすべて実際にpassすることを確認)。
  6. **正直な開示(未検証・既知の制約、このリポジトリの既存方針どおり
     隠さず記録)**: 実クラウドアカウント・実SMTPサーバー・実VPSへは
     一度も接続していない——検証は全てローカルの偽サーバー(手製の
     ミニマムSMTPサーバー・`wiremock`・インプロセス`russh`/`russh-sftp`
     サーバー)によるモック結合テストのみ(`tests/offsite_backup_integration.rs`
     冒頭のドキュメントコメントに明記された既存の検証方針どおり)。
     `accel.rs`のGPU/NPU圧縮は2026-0725時点で未実装で常にCPUへ
     フォールバックする設計のまま(日英Web検索でもDirectX/クロス
     ベンダー対応の圧縮GPUカーネルの流用先クレートは見つからなかった
     ため、実装するとすれば独自HLSLカーネルの新規開発が必要——
     今回のスコープ外)。SFTPのホスト鍵検証は`check_server_key`が常に
     `Ok(true)`を返す設計のままで、既知ホスト鍵の永続的な検証は未実装
     (`offsite_backup.rs`内のコメントに明記済み)。`GoogleDriveBackupTarget`の
     OAuth2リフレッシュ実装自体は`wiremock`でHTTPレベルの検証は行ったが、
     実Googleアカウントでの動作確認は行っていない。実際のディスク
     切断・LAN切断シナリオでの`disaster_recovery.rs`自動復旧モードの
     実機検証(VM/実ハードウェアでのケーブル抜線等)も今回は未実施
     ——ユニットテストレベルの検証にとどまる。次回この機能に戻る際は、
     上記の未検証項目(実クラウド接続・実機切断シナリオ・GPU圧縮
     カーネル)から優先度をつけて着手すること。

- **2026-07-23(続き) `open-easy-web`で発見したTOTPテストの実バグと
  ワークスペース構造の罠を、正本として記録(どのリポジトリからでも
  参照できるように)**:
  1. **TOTPテストのブルートフォースによるflaky failure**: 「サーバー側の
     検証関数が受理する6桁コードを0〜999999まで総当たりして探す」
     というテスト手法は、debugビルドでは正解の番号次第で数秒〜
     数十秒かかることがあり、その間にTOTPの時間窓(既定30秒×スキュー
     許容)を超えて間欠的に失敗する。**正しい対処**: TOTP側に「指定
     時刻の正しいコードを直接計算する」関数を`pub`化してテストから
     直接呼び出し、総当たりを一切行わない。`open-easy-web`での実測:
     該当テスト1件が23秒→0.02秒(約1000倍)に短縮、3回連続green
     (修正前は約3回に1回failed)を確認(詳細は`open-easy-web`側
     CLAUDE.md/PORTING.md参照)。TOTP/HOTPを実装する他のリポジトリ
     (2FA機能を持つもの全般)でも、テストに同種のブルートフォース
     ループが無いか点検すること。
  2. **「ルートで`cargo test --workspace`しても実は0件しか検証して
     いない」構造の罠**: 複数クレート・複数ワークスペースに分割された
     Rustプロジェクト(ルートがWASM/フロントエンド用の別`[workspace]`、
     サブディレクトリ`server/`等が独自の別`[workspace]`を持つ構成)では、
     ルートで`cargo test --workspace`を実行しても対象クレートの
     テストが一切実行されないまま「成功」してしまうことがある。
     複数ワークスペースに分かれたリポジトリを扱う際は、実際に何件の
     テストが走ったかを毎回確認し、「0件で成功」を「検証した」と
     混同しないこと。

### 2026-07-25セッション末尾チェックポイント(リミット接近のため記録)

**このセッションで完了したこと**:

1. **`open-directx`**: シェーダー拡張(乗算・境界チェック付き減算、
   コミット`93bf231`)完了・実機検証済み。続けてDXIL実パース+D3D11
   グラフィックスパイプライン(頂点/ピクセルシェーダー)へ着手
   ——**セッション終了時点で完了報告は未受領**、`triangle_ps/vs.hlsl`・
   `vector_add.dxil`・`dump_dxil.rs`まで進捗確認済み(次回`git status`で
   確認)。DXILの次はD3D12パイプラインが自然な流れ(DXBC↔D3D11、
   DXIL↔D3D12のペアリング、ユーザーとの合意事項)。

2. **`open-cuda`**: `opencuda-llm`にGPT-2 124M実重みローダー実装・
   実際の英語生成を確認済み(コミット`d1eca7d`)。続けてcuBLAS実装
   (NVIDIA専用、実機GT730で検証)に着手——CUDA Toolkit 11.4.4+VS
   Build Toolsのインストールを開始、**セッション終了時点で完了報告は
   未受領**(次回`nvcc --version`で導入完了を確認してから続行)。

3. **`aruaru-llm`**: `opencuda-llm`のGPT-2を統合し`POST /v1/generate`
   新設、実HTTPリクエストで実際の生成確認済み(コミット`9b1825c`、
   完了)。

4. **自動同期バックアップシステム**(`open-easy-web`/`open-web-server`/
   `open-raid-z`/`aruaru-db`横断): `open-web-server`側に
   `open-web-server-crossbackup`クレート・`crossrepo_backup.rs`を
   実装中。**セッション終了時点で完了報告は未受領**——次回`git status`
   で進捗確認・再開が必要。既存バックアップ機構(`aruaru-backup`・
   `open-web-server-ledger`のマルチリージョン/監査ログ・`open-raid-z`
   スナップショット)を横断連携させる設計。

5. **リポジトリ改名完了**(前回チェックポイントの継続):
   `open-cosmo`(旧`open-runo`、広範囲)→`open-runo`、`RCosmo`(狭範囲)
   →`open-cosmo`。GitHub description・ローカル・VPS(`/root`)とも更新
   済み。`open-runo`→RPoemへの実装救済調査完了(救済不要と判明、
   RPoem側が既に上位互換)。**`open-runo`(旧open-cosmo)側の内部
   ドキュメントは未着手のまま**(自称が古い可能性)。

6. **Android版(open-web-server)完成**: 実エミュレータで`GET /healthz`
   →`200`実証済み。3電源プロファイル(省電力/通常/常時電源接続)・
   タブレット対応・open-easy-web連携ボタンも実装済み(コミット
   `6fa57ef`/`fed4995`)。物理実機検証・APK署名配布は未実施。

7. **open-easy-web/RS-Redレスポンシブ+英日併記UI**: 完了・push済み
   (コミット`a27cb03`/`2b4ef20`)。

**次回セッション開始時に最優先で確認すべきこと**:
1. `open-directx`(DXIL/D3D11の完了状況)・`open-cuda`(cuBLAS/CUDA
   Toolkit導入状況)・自動同期バックアップの3タスクを`git status`/
   `git log`で確認し、未完了なら再開する。
2. **教訓**: サブエージェントが「別エージェントに委任しました」とだけ
   報告して実際には何も変更していないケースが複数回発生した。再開
   させる際は毎回`git status`で実ファイル変更の有無を確認してから
   判断すること(委任だけで終わっていれば「自分で直接実装するように」
   と明示的に指示し直す)。
3. `open-runo`(旧open-cosmo)側の内部ドキュメントの自称表記確認
   (未着手のまま)。

### 2026-07-24セッション末尾チェックポイント(リミット接近のため記録)

**このセッションで完了したこと**:

1. **リポジトリ改名(ユーザー最終決定、重要・必ず把握すること)**:
   - `aon-co-jp/open-cosmo`(旧`open-runo`、Poem+Tauri+Cosmo有料版+
     WEB高速化の広範な実装)→ **`aon-co-jp/open-runo`へ復元改名**。
   - `aon-co-jp/RCosmo`(Cosmo有料版+WEB高速化中心の狭い実装)→
     **`aon-co-jp/open-cosmo`へ改名**。
   - つまり現在「`open-cosmo`」という名前は**旧`RCosmo`**を指す
     (旧`open-cosmo`ではない、名前が入れ替わっているので注意)。
   - ローカル(`F:\runo\RCosmo`→`F:\runo\open-cosmo`)・VPS
     (`/root/RCosmo`→`/root/open-cosmo`)ともフォルダ名・gitリモート
     URLを更新済み。GitHub description両リポジトリとも更新済み。
   - `open-cosmo`(旧RCosmo)内部ドキュメントの自称「RCosmo」→
     「open-cosmo」統一、完了・push済み(コミット`a066682`)。
   - **`open-runo`(旧open-cosmo)側の内部ドキュメントは今回未着手**
     (自称が「open-cosmo」のままの箇所が残っている可能性、次回対応)。
   - `open-runo`→RPoemへの「Poem/Tauri実装の救済」調査を実施した結果、
     **救済すべき差分は無し**と判明(RPoem側が既に上位互換、
     `open-runo`はRPoemから派生・エクスポートされたスナップショットと
     判断)。コード移植は行わず、調査結果をRPoem側ドキュメントに記録
     のみ(コミット`c6bafea`)。`open-runo`自体は無変更のまま。
   - **リポジトリの完全削除は一切行っていない**(私の安全ルール上
     実行できないため。旧名のリポジトリはリネームで残っている)。

2. **open-web-server Android版**: 3電源プロファイル(省電力/常時電源
   接続/通常、省電力版は文字+アイコン両方で明示)・タブレット向け
   レイアウト(`layout-sw600dp/`)の実装が進行中。Kotlinシェル+
   `cargo ndk`ビルド+gradle assembleDebugでAPK生成は既に成功実績あり。
   **adb unauthorized問題**(ヘッドレスエミュレータでの実機/エミュレータ
   起動確認がブロックされる)が継続課題、対応中。open-easy-webとの
   SET連携導線も指示済み。**セッション終了時点で完了報告は未受領**
   ——次回セッション開始時に`git status`で進捗を確認すること。

3. **open-easy-web/RS-Redのレスポンシブ+英日併記UI**: スマホ縦画面
   自動レイアウト切替(CSSメディアクエリ)・英語+(日本語)併記表示の
   実装に着手済み。**セッション終了時点で完了報告は未受領**——次回
   セッション開始時に`git status`で進捗を確認すること。

4. **open-web-server/open-easy-web**: DDNS無料ドメイン自動化(DuckDNS、
   最大20ドメイン)・組み込みSFTPサーバー・UPnP自動ポート開放・
   CORS対応・構造化アクセスログ(JSON+gzipローテーション)・
   RS-LinkFusionとの連携実機検証(追加コード不要と確認)が完了・
   push済み。

5. **RS-Red**: ガントチャート用フィールド・フィルタAPI・DDNS運用対応・
   StorageBackend抽象化(ローカル/SFTP/Googleドライブ、Store群への
   実配線まで完了)が完了・push済み。

6. **runo.tokyo**: `/runo`プロジェクト紹介ページに全リポジトリの配布
   状況(Windows/Linux DLリンク・Android実態)・GitHub API自動更新機能
   を追加、完了・push済み。

7. **エコシステム全体インストーラー整備**: `RS-Guard`・`RS-Ops`・
   `rs-sync`(release.yml追加)・`RCosmo`(現open-cosmo)・`aruaru-llm`・
   `open-easy-web`に3点セット新規追加、全てCI成功・GitHub Release
   実在確認済み。

**次回セッション開始時に最優先で確認すべきこと**:
1. Android APK完成タスク(`aa73d9a0783b42415`)・open-easy-web/RS-Red
   レスポンシブUIタスク(`a561135a502ec8630`)の完了状況を`git status`/
   `git log`で確認し、未完了なら再開する。
2. `open-runo`(旧open-cosmo)側の内部ドキュメントの自称「open-cosmo」
   表記の要修正確認(今回`open-cosmo`〈旧RCosmo〉側は完了したが
   `open-runo`側は未着手)。
3. VPS上の`mirror-cache`(`sync-repos.sh`)は次回cron実行で自動的に
   新しいリポジトリ名でキャッシュを作り直すはずなので、動作確認のみ
   でよい(手動介入は不要と判断済み)。

### 2026-07-23セッション末尾チェックポイント(リミット接近のため記録)

**このセッションで完了したこと**:
1. `aruaru-db`・`open-raid-z`に3点セット(`install.sh`/`install.ps1`/
   `release.yml`)を追加・push済み。
2. v0.1.0タグを両リポジトリへpushしCIを起動 → **両方とも初回は失敗**、
   原因調査・修正・push済み:
   - `aruaru-db`: `aruaru-core`が`../RS-JSON`へのpath依存を持つが
     CI環境にsiblingリポジトリが無く失敗 → `git clone`でのsibling
     checkoutをrelease.ymlへ追加(コミット`b0cbcaa`)。
   - `open-raid-z`: `open_raid_z_core/Cargo.toml`の`windows`crateが
     全OS共通`[dependencies]`に置かれておりLinux CIでもコンパイル
     対象になり(windows-future 0.2.1の推移的バージョン不整合で
     ビルド不能)、Windows側は既定feature`gpu_accel`がdxc(DirectX
     Shader Compiler)を要求しCIランナーに無く失敗。`windows`/
     `widestring`を`[target.'cfg(target_os = "windows")'.dependencies]`
     へ移動、release.ymlは`--no-default-features --features
     winfsp_backend,foreign_fs`(Windows)/`fuse_backend,foreign_fs`
     (Linux)でgpu_accelを明示除外(コミット`4afbc29`)。
   - **未確認のまま中断**: 上記修正後、タグを再pushしてCIが実際に
     成功するかどうかはまだ確認していない。**次回セッション最初に
     すべきこと**: 両リポジトリで`git tag -d v0.1.0 && git push
     origin :refs/tags/v0.1.0`(失敗した古いタグ削除)→
     再度`git tag v0.1.0 && git push origin v0.1.0`→
     `gh run list`/`gh run view --log-failed`で結果確認→
     成功なら`gh release view v0.1.0`で実在確認、失敗ならログを見て
     再修正、というサイクルを完了させること。
3. RS-Redの完成度向上(バックグラウンドエージェント、完了済み):
   `start_date`/`due_date`/`done_ratio`(ガントチャート用)フィールド追加、
   チケット一覧のstatus/project_idフィルタAPI実装。テスト40件全green、
   コミット`4b7ae2c`push済み。**未着手として次回HANDOFFに記録済み**:
   GUI側のガントチャート/カレンダー描画、`assignee`フィールド、通知機能。
4. `open-web-server`にSFTP簡単接続機能を追加(バックグラウンドエージェント、
   完了済み): `russh`/`russh-sftp`による組み込みSFTPサーバー(`sftp`
   feature、公開鍵認証、パストラバーサル対策)、`igd-next`による
   UPnP自動ポート開放(`upnp` feature、明示opt-in)、
   `GET /admin/sftp/connection-info`ヘルパー。実SFTPクライアントでの
   ループバック往復検証まで実施、コミット`6d7152c`push済み。
   **正直な制限**: UPnPは実ルーターの無い環境のため実機未検証、
   SSHホスト鍵は使い捨てで永続化していない。

**次回セッション開始時に確認すべきこと(優先順)**:
1. 上記2.のCI再検証サイクルの完了(aruaru-db/open-raid-z両方)。
2. エコシステム全体インストーラー整備計画のバックログ(下記節参照)
   から次のリポジトリへ着手。
3. RS-Red・open-web-server(SFTP)双方とも、GUI側の描画・実機UPnP検証
   等の残課題あり(各リポジトリのCLAUDE.md HANDOFF参照)。

### 2026-07-24 CI再検証サイクル完了確認(上記優先1.の決着)

- **open-raid-z**: タグ`v0.1.0`を`68ea6b5`(HEAD)へ削除・再作成し
  push。CI(`gh run view 30056906922 --json conclusion`)は
  `success`。`gh release view v0.1.0 --repo aon-co-jp/open-raid-z`で
  `open-raid-z-linux-x86_64.tar.gz`・`open-raid-z-windows-x86_64.zip`
  の実アセット存在を確認済み。
- **aruaru-db**: タグ再作成後の初回CI(`30056607054`)は再度失敗。
  原因: `aruaru-dist`の`open_raid_z_core`依存は`optional = true`だが、
  Cargoはワークスペース全体のマニフェスト解決時に(feature無効でも)
  このpath依存のCargo.tomlを読もうとするため、`open-raid-z`自体の
  siblingチェックアウトが無いと失敗する(RS-JSONのsibling
  チェックアウトだけでは不十分だった、新たに判明した2つ目の
  sibling依存)。`release.yml`へ`open-raid-z`のsiblingチェックアウト
  手順を追加しコミット`0ac865d`push、タグを再度削除・再作成。
  最終CI結果は`success`、`gh release view v0.1.0 --repo aon-co-jp/
  aruaru-db`で`aruaru-db-linux-x86_64.tar.gz`・
  `aruaru-db-windows-x86_64.zip`の実アセット存在を確認済み。
- **教訓**: `optional = true`のpath依存でも、CIのワークスペース
  マニフェスト解決には依存先の実ファイルが必要(featureのON/OFFでは
  回避できない)。今後同様のpath依存を追加する際は、そのリポジトリ
  自体もCIでsiblingチェックアウトする必要がないか必ず確認すること。
- これで**優先1.(CI再検証サイクル)は両リポジトリとも完了**。
  次回は優先2.(インストーラー整備バックログ)を継続。

### エコシステム全体インストーラー整備計画(2026-07-23、ユーザー指示、原文のまま記録)

> 「open-easy-webより簡単インストール機能付きで、aruaru-dbやopen-raid-z
> などその他の全てのリポジトリを提供したい。全てのプロジェクトは、
> LINUX、Windows、Androidスマホもタブレット版対応のインストーラー付きで
> 提供したい。」

**現状の在庫調査(2026-07-24再監査、前回2026-07-23時点の表は既に古く
なっていたため実ファイル存在確認で更新)** — `install.sh`/
`install.ps1`/`.github/workflows/release.yml`の有無をF:\runo配下の
全Gitリポジトリで再確認:

| 揃っている(3点セット) | 未着手(ライブラリ専用のため対象外と判断) | 未着手(実行可能アプリ、対応要検討) |
|---|---|---|
| RPoem, RS-Blog, RS-EC, RS-Git, RS-LinkFusion, RS-Red, open-web-server, RS-Guard, RS-Ops, rs-sync, aruaru-llm, open-easy-web, **aruaru-db**, **open-raid-z** | RFrontEnd, RS-JSON, RS-SmartTCP, rs-to-readme, runo-scanner | aruaru.tokyo, audiocafe-tokyo-php, audiocafe-tokyo-rust, e-gov.info, karu.tokyo, open-cuda, rs-gitbucket, runo.tokyo, RCosmo(ローカルに現存せず要確認) |

**教訓**: この在庫表は前回(2026-07-23)作成時点でも既に一部古く
(RS-Guard/RS-Ops/rs-syncはその後の別セッションで3点セット完備済み
だった)、「一度作った在庫調査結果」を鵜呑みにせず、次に参照する
セッションは必ず実ファイルの存在を再確認してから着手すること。

**方針(次回以降の実装ロードマップ)**:

1. **配布パターンの統一**: 今夜確立した3点セット
   (`install.sh`〈systemd〉+ `install.ps1`〈Windowsサービス登録〉+
   `.github/workflows/release.yml`〈タグpushでLinux x86_64/aarch64・
   Windowsバイナリ自動ビルド、`softprops/action-gh-release@v2`〉)を
   各リポジトリの実体(バイナリかライブラリか、実行可能アプリか否か)
   に応じて適用する。ライブラリ専用クレート(RS-JSON、rs-to-readme、
   runo-scanner等)はインストーラー対象外(実行可能アプリではない
   ため)——「全リポジトリ」は文字通り全てではなく、**エンドユーザーが
   実行する配布物を持つリポジトリ**が対象という理解で進める。
2. **明示された優先2件(`aruaru-db`・`open-raid-z`)から着手**——
   ユーザーが名指ししたため、他の未着手リポジトリより先に対応する。
   **完了(2026-07-23)**: 両リポジトリとも`install.sh`(systemd)・
   `install.ps1`(Windows)・`.github/workflows/release.yml`(タグpushで
   Linux x86_64/Windows自動ビルド)の3点セットを追加済み。`aruaru-db`は
   `aruaru-server`バイナリを配布対象とした。`open-raid-z`は実体が
   `open_runo_zfs_source/open_raid_z_core`というネストしたcrateのため、
   ワークフロー内で`working-directory`指定・FUSE開発ヘッダ
   (`libfuse3-dev`)の事前インストールを追加した(`orzctl`バイナリを
   配布対象)。**正直な開示**: タグpushによる実CI動作(GitHub Actions上
   でのビルド成功)自体はまだ検証していない——次にv0.1.0等のタグを
   pushした際に確認すること。Android版インストーラーはこの2件では
   まだ未着手(上記の在庫調査時点でのAndroidクロスコンパイル検証パターン
   〈`cargo ndk`〉を横展開する形で次回対応)。
3. **Android対応**: 今夜`open-web-server`・`RS-Red`で実証済みの
   `cargo ndk -t aarch64-linux-android build --release`によるクロス
   コンパイルパターン(NDK 27.1.12297006、rustup target
   `aarch64-linux-android`等は本サンドボックスに導入済み)を横展開。
   各リポジトリで`reqwest`等のTLS依存クレートが`default-features =
   false`無しで`rustls-tls`を追加していないか(→`openssl-sys`が
   cross-compile不能で失敗する、今夜2回踏んだ罠)を都度確認する。
   ただしAPK化(Kotlin/Javaシェル・ProcessBuilder実行ラッパー・
   foreground service・電源プロファイルUI)自体は別途未着手。
4. **`open-easy-web`をインストールハブ化**: 既存の「分身の術」
   (`TenantRegistry`/`SharedDispatcher`、ドメイン毎インストール不要の
   多重化パターン)を流用し、`open-easy-web`側に各リポジトリの
   GitHub Releasesへのダウンロードリンク一覧(ダウンロードハブ)を
   追加する方向で検討——実装はまだ着手していない。
5. **正直なスコープ認識**: リポジトリ数が多く(候補20件超)、
   1セッションで全て完了させるのは非現実的。「未着手だからといって
   確認を求めて手を止めない」という運用ルールに従い、優先2件から
   着手しつつ、残りは本節を継続更新するバックログとして扱う。

### Vulkan バックエンド対応の検討（2026-07-23、将来拡張ロードマップ）

`RS-LinkFusion` の GPU アクセラレーション統合において、`opencuda-directx`（DirectX 12）バックエンドは実装・テスト済みだが、**NVIDIA GT730 のような DirectX 12 非対応の GPU でも Vulkan 1.0 対応であれば動作可能** であることが確認された。  
このため、エコシステム全体として **Vulkan バックエンド（`opencuda-vulkan`）への対応を将来の拡張ロードマップに正式に追加** する。

**経緯**：
- `opencuda-vulkan` は既に `opencuda-core` と共存可能なクレートとして実装済み（Vulkan Compute ベース）
- GT730 は Vulkan 1.0 に対応しているため、`opencuda-vulkan` 経由で GPU 加速が利用できる可能性が高い
- 現状の `opencuda-directx` は Windows + DirectX 12 専用であり、クロスプラットフォーム対応という観点でも Vulkan の方が優位

**今後のタスク**：
1. `RS-LinkFusion` の `AccelBackend` に `Vulkan` バリアントを追加
2. `opencuda-vulkan` の ChaCha20 カーネル（SPIR-V）を実装
3. Vulkan バックエンドと DirectX バックエンドの両方をサポートし、環境に応じて自動選択する仕組みを導入

**優先度**：中（GT730 ユーザーの需要次第で高に変更可能）



- **2026-07-23(続き) open-cudaのDirectXバックエンドにGPU圧縮/暗号化
  カーネル(ChaCha20)を実装、実バグ発見・修正(正本への記録)**:
  `open-cuda`の`opencuda-directx`クレートに、RS-LinkFusion(複数WAN/
  LAN/WiFiボンディング+CPU/GPU/NPUハードウェアアクセラレータ抽象化)
  側の要望に応える形で、matmulカーネル対応とChaCha20圧縮/暗号化
  カーネルをDXIL/HLSLで実装した。実機(NVIDIA GT 730)検証中に、
  **HLSLのcbuffer配列パディングの罠**(`uint key[8]`のようなスカラー
  配列は各要素が16バイト境界へパディングされ、Rust側が
  `SetComputeRoot32BitConstant`で詰めて渡す密なdword列とオフセットが
  ズレる)による実バグを発見——GPU出力が暗号化されず平文のまま返る
  という不具合だった。個別スカラーフィールドへの書き換えで解消し、
  RustCrypto製`chacha20`クレートとの数値一致を実証した(詳細は
  `open-cuda`側CLAUDE.md HANDOFF、コミット`ec6acf1`参照)。
  この成果はRS-LinkFusion側の`accel.rs`(`AccelBackend::Gpu`)の
  実装候補として、次回そちらのセッションで統合を検討する。

- **2026-07-23 2026年最新のDB/通信アーキテクチャ動向を日英Web検索で再調査
  (ユーザー指示——特にSnowflake×CockroachDBハイブリッドの動向)、
  エコシステム全体の設計方針への反映点を記録**:
  > ⚠️ この節はopen-raid-z自体のコード変更ではなく、エコシステム全体
  > (特にaruaru-db)の設計方針としての正本記録。実装はaruaru-db側
  > CLAUDE.mdのHANDOFFを参照。
  1. **HTAP(Hybrid Transactional/Analytical Processing)が2026年の
     核心トレンドと確認**: 「Snowflake×CockroachDBの良いとこ取り」を
     求める設計思想そのものが、業界では既に**HTAP**という確立した
     名前を持つアーキテクチャパターンとして実在することが分かった。
     Snowflakeは2022年に「Unistore」(Hybrid Tables)でOLTP/OLAP統合に
     参入済み、TiDB(TiKV行ストア+TiFlash列ストアをリアルタイム同期)・
     SingleStore・SAP HANA・CockroachDBが代表例
     ([PingCAP: Real-World HTAP](https://www.pingcap.com/blog/real-world-htap-a-look-at-tidb-and-singlestore-and-their-architectures/)、
     [MotherDuck: HTAP](https://motherduck.com/glossary/htap/))。
     **核心設計**: 「行ストア(トランザクション用)と列ストア(分析用)を
     内部に両方持ち、自動的に同期させる」または「両方のパターンに
     十分柔軟なストレージエンジンを使う」の2方式。
  2. **金融分野での実務整合**: CockroachDBのgeo-partitioning(データを
     地理的リージョンへ自動配置、コンプライアンス対応)とSnowflakeの
     統合ガバナンス・分析基盤は「相補的」と位置づけられており
     ([ispirer.com: Best Database for Financial Data 2026](https://www.ispirer.com/blog/best-database-for-financial-data))、
     本エコシステムが目指す「ACID互換+ZFS互換+分散合意+リアルタイム
     分析」という組み合わせ自体は2026年の実務トレンドと整合している
     ことを確認。
  3. **aruaru-db側で発見した具体的ギャップ**: `aruaru-query::olap.rs`は
     既にDataFusionによるOLAP経路(HTAPルーティングの片翼)を実装済み
     だったが、**OLAPクエリのたびに全テーブルを行ストアから毎回
     フル再構築**する設計であり、TiDB/TiFlash方式が実践する
     「行ストアの変更を列ストアへ継続的にインクリメンタル同期する」
     という核心的な性質を持っていなかった。これは「最先端が既に
     対応済みの設計は今すぐ実装する」という2026-07-23運用ルール
     追記の対象そのものであり、aruaru-db側で対応に着手した(詳細は
     aruaru-db側CLAUDE.md HANDOFF参照)。
  4. **今回スコープ外と判断した点**: 真のマルチノード分散HTAP(TiKV/
     TiFlash間のネットワーク越しレプリケーション相当)は、
     aruaru-distのRaft実装自体がまだ単一プロセス内(ネットワーク越し
     複製はopenraft統合待ち)であるため、今回は「単一プロセス内での
     行→列インクリメンタル同期」という現実的なスコープに留めた。
  5. **(続き、同日中に再設計・実装完了)** 上記3のギャップ対応は最初
     テーブル単位粒度で実装したが、ユーザーから「その限界に対する
     再設計方法を日英で検索して開発に活かして」と指示を受け、
     TiFlashの実設計(「Delta Tree」——B+木とLSM木のハイブリッド、
     新規行はまず行ストア形式のデルタ層へ、後で列ストアのベース層へ
     バッチマージされる)を調査
     ([TiFlash Overview](https://docs.pingcap.com/tidb/stable/tiflash-overview/))。
     SQL Serverの列ストアインデックスも同様の「デルタストア」方式を
     持つことを確認、業界横断で確立した設計と裏付けを取った上で、
     行単位のpk追跡+`arrow::compute::filter_record_batch`によるベース
     からの除去+変更行だけの再取得+`concat_batches`結合、という
     真の行単位インクリメンタル同期へ再設計・実装完了(aruaru-db側
     `cargo test -p aruaru-query`42件全green、詳細はaruaru-db側
     CLAUDE.md HANDOFF参照)。**「日英検索で最先端が既に対応済みと
     わかった設計ギャップは自動実装する」という2026-07-23運用ルールを
     実際に2段階(まずテーブル単位→ユーザー指摘で行単位へ再設計)で
     体現した実例**として記録する。

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

### Apache/Tomcat互換性の目標(ユーザー指示、2026-07-23、原文のまま記録)

> RPoemのPoemとの互換性の向上や、open-web-serverのApacheとNginxなどと
> 互換性でApacheの様にWEBサーバーとして間違いなく動作するようにしたい。
>
> open-web-serverをJAVAのApacheの様に使える様にしたい。
>
> そして
>
> 連携でApacheのTOMCATの様にRPoemをJavaからでもRustとPoemやRPoemからでも
> Rubyからでも、Ruby on Railsからでも、PHPやPHPとLARABELからでもPython
> からでも、PythonとFastAPIからなど汎用性を持たせ互換性を高めたいです。

**現状の到達点(2026-07-23調査済み)**: `open-web-server`の`app_proxy`/
`tenant_router`は既にプレーンHTTPで転送する設計のため、**言語を問わず
HTTPで応答する任意のアプリケーションサーバーを同じ仕組みで指せる**
(2026-07-14実装、`open-web-server`側CLAUDE.md HANDOFF明記)。Java
(Spring Boot等)・Ruby on Rails(`rails server`)・PHP/Laravel
(`php artisan serve`/php-fpm)・Python/FastAPI(`uvicorn`)いずれも、
単体のHTTPサーバーとして起動し`POST /admin/tenants`へ登録するだけで
Apache+Tomcat的な連携が動く。

**残る具体的なギャップ**:
1. RPoem側`open-runo-appserver::tenant_bridge`(型非依存の橋渡し関数、
   実装・テスト済み)と`open-web-server`側`TenantRegistry`の実際の
   クロスリポジトリ配線——設計上は`open-easy-web`(ドメイン登録UI)が
   両方の管理API(`POST /admin/tenants`・`POST /admin/appserver-tenants`)
   を呼ぶ形で完成させる想定だが、この配線自体はまだ未接続。
2. PHP-FPM/FastCGIのような本番グレードの直結経路が`open-web-server`に
   無く、現状は開発用`php -S`のみ(Ruby/Python/Java側もFPM相当の
   常駐プロセス管理は無く、都度`rails server`等を手動起動する前提)。
3. RPoem(Poem本体)側のパリティは`docs/poem-parity.md`によれば
   ほぼ完成しているが、gRPC対応範囲拡大・brotli圧縮・poem-openapi相当の
   マクロ自動生成が残課題(詳細はRPoem側`docs/poem-parity.md`参照)。

次回この方針に着手する際は、上記3点を優先順位付きで進めること
(1が最も価値が高い——3つ目のリポジトリ`open-easy-web`をまたぐ作業)。
