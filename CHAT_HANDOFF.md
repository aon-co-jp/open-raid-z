# OpenRuno / open-raid-z — Claude Desktop 引き継ぎ資料

最終更新: このチャットセッションの最後の状態を反映

---

## 最重要: 正となる場所

**このプロジェクトの正式なソースは、もうzipファイルではなくGitHubです。**

```
リポジトリ: https://github.com/aon-co-jp/open-raid-z
ブランチ:   feature/raid-z2-z3-scaffolding
最新コミット: 7417362 "Add snapshots and clones, completing the ZFS feature-parity checklist"
```

ローカル(`E:\open-runo\open-raid-z`)は上記と完全同期済み(`git status`で
`nothing to commit, working tree clean`確認済み)。

Claude Desktopでは、**zipを再展開するのではなく、このリポジトリをclone**
するところから始めてください。

---

## Claude Desktopでの再開手順

### 1. リポジトリをclone(まだの場合)

```powershell
cd E:\open-runo   # または任意の親ディレクトリ
git clone https://github.com/aon-co-jp/open-raid-z.git
cd open-raid-z
git checkout feature/raid-z2-z3-scaffolding
```

既に `E:\open-runo\open-raid-z` にある場合はclone不要、そのまま使えます。

### 2. Claude Desktopにワークスペースフォルダを指定

親ディレクトリ `E:\open-runo\` を1つ指定すれば、配下の `open-raid-z` を含む
OpenRuno系プロジェクト全部にアクセスできます(open-aruaru, OpenRedmine,
OpenWordPress等も同じ親配下に置いている場合)。

- 方式A: Claude Desktop設定内のフォルダピッカーで `E:\open-runo\` を選択
- 方式B: `claude_desktop_config.json` に以下を追記

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "E:\\open-runo"]
    }
  }
}
```

### 3. Claude Desktopでの最初のメッセージ例

```
E:\open-runo\open-raid-z の CHAT_HANDOFF.md を読んで経緯を把握してください。
現在のブランチは feature/raid-z2-z3-scaffolding、最新コミットは 7417362 です。
続けて openruno-installer (Tauri) の実装を進めたいです。
```

---

## プロジェクト概要(経緯サマリ)

Windows版NTFS互換・全RAID対応・NPU/GPUハードウェアアクセラレータ対応の
ZFSファイルシステム導入システムの開発。「DirectXのプラグイン」という
当初案は技術的に成立しないため、以下の3コンポーネント構成に整理した。

| # | コンポーネント | 役割 | 技術スタック |
|---|---|---|---|
| ① | openzfs-winfsp-bridge | WinFsp経由でZFSのI/Oをフックし、NTFS ACLセマンティクスをエミュレーション | Rust + WinFsp(winfsp-rs) + windows-rs |
| ② | zfs-accel-hlsl | チェックサム/RAID-Zパリティ/圧縮をNPU/GPUへオフロード(DirectML経由) | Rust + DirectX 12 Compute + DirectML(HLSL) |
| ③ | openruno-installer | ハードウェア検出・ドライバ登録・zpool初期化のGUIインストーラー | Tauri |

### GitHub側で実装が進んでいる内容(このチャット外で追加されたもの)

ローカルとGitHubの同期時点で、①②には以下が既に実装済みだったことを確認:

- `pool.rs` / `vdev.rs`: ZFSスタイルのストレージプール、複数データセット管理
- `checksum.rs`: チェックサム + 自己修復(self-healing)
- `block_device.rs`, `id_mapping.rs`: ブロックデバイス抽象化、UID/GIDマッピング
- Copy-on-Write(CoW)セマンティクス実装
- スナップショット/クローン機能(ZFS feature-parityチェックリスト達成)
- RAID-Z2/Z3の障害復旧(resilver)統合テスト
- `galois.rs`, `gf_matrix.rs`, `raidz23_parity.rs`: Reed-Solomon(GF(2^8))によるRAID-Z2/Z3パリティ計算
- `raidz2_parity.hlsl`: RAID-Z2用HLSLシェーダ追加
- テスト一式: `checksum_self_healing.rs`, `copy_on_write.rs`, `pool_management.rs`,
  `raidz_failure_recovery.rs`, `snapshots_and_clones.rs`

このチャットで作成したのはRAID-Z1(単一XORパリティ)のCPU参照実装と
ACL変換の骨格まで。GitHub側はそれよりかなり先行している状態。

### ③ openruno-installer(Tauri)の状態

`package.json`, `tauri.conf.json`, `src-tauri/`, アイコン一式など
Tauriプロジェクトの雛形は既に存在(GitHub側)。中身の実装(ハードウェア
検出UI、zpool初期化ウィザード等)は要確認・要継続。

---

## ユーザーのプロジェクト方針(参考)

- PureRust + Poem + 独自AI予測判断 を基本方針とする「OpenRuno」構想
- Tauriでデスクトップアプリの高速化・サーバー負荷軽減を志向
- 関連プロジェクト群: open-aruaru(iLumi)、open-e-gov、OpenDirectX、
  OpenCuda、OpenLLM(aruaru-llm)、OpenCosmo、OpenRedmine、OpenWordPress
- 参考実装: WunderGraph Cosmo(Go製、OpenRunoではPure Rust化する方針)

---

## 次のステップ候補

1. `openruno-installer`の実装内容を確認し、未完成部分を洗い出す
2. 実機Windows環境でのビルド検証(WinFsp依存、DirectML依存の実クレート名確認)
3. `feature/raid-z2-z3-scaffolding` → `main` へのPull Request作成
4. NTFS ACL⇔ZFS ACLのUID/GIDマッピングの実運用設計(AD/SAM連携)

---

*この資料はGitHub同期完了時点(コミット7417362)の状態を記録したものです。
以降の変更はGitHub上のコミット履歴を正としてください。*

---

## 追記: 実用性向上セッション(完成度・ビルド健全性の改善)

このセッションでは新機能追加ではなく、「ビルドできる」「テストできる」という
基礎の底上げと、既知の制約(README「現状の制約」節)のうち1点の解消を行った。

### 1. ビルドの致命的バグを修正(最優先で対応)

READMEは`cargo test --no-default-features`でWinFsp無しでもテストできると
謳っていたが、実際には**常に失敗していた**:

- `openzfs-winfsp-bridge/Cargo.toml`の`[build-dependencies]`で`winfsp`が
  `optional`になっておらず、`--no-default-features`を付けても常に`winfsp`
  クレート(→WinFsp SDKヘッダ)を要求していた。
- `zfs-accel-hlsl/build.rs`が、featureに関わらず常に`dxc`
  (DirectX Shader Compiler)の存在を要求してpanicしていた
  (`zfs-accel-hlsl`は`openzfs-winfsp-bridge`の非optional依存だったため、
  ブリッジ側だけ`--no-default-features`を付けても無意味だった)。

**対応**: `zfs-accel-hlsl`に`gpu` feature(既定ON)を新設し、D3D12/DirectML
呼び出し(`device.rs`の実装部分、`compute.rs`全体)とシェーダ事前コンパイル
(`build.rs`)をこのfeature配下に隔離。`openzfs-winfsp-bridge`側にも
`gpu-accel` feature(既定ON、`zfs-accel-hlsl/gpu`に配線)を新設し、
`default-features = false`で依存するよう変更。

**効果(実機で検証済み)**: Ubuntu上にRustをaptで導入しただけの素の環境で、

```bash
cd openzfs-winfsp-bridge
cargo test --no-default-features
```

が実際に成功し、RAID0/1/5/6/10/Z2/Z3・チェックサム自己修復・CoW・
スナップショット/クローン・resilverを含む**全65テストがWindows/WinFsp/dxc
無しでパス**することを確認した(`zfs-accel-hlsl`単体でも20テスト全パス)。
これによりCI(GitHub Actions等)や非Windows開発機でのロジック検証が
初めて現実的になった。

### 2. WinFspマウントの複数データセット対応(README「現状の制約」の解消)

従来`mount.rs`はルート直下に固定ファイル`\pool.dat`が1つだけ存在する
設計だった。`Pool`自体は元々複数データセットを管理できる設計
(`dataset_names()`が存在)だったため、`mount.rs`を拡張し、
**プール内の全データセットをそれぞれ`\<データセット名>`というファイルとして
ルート直下に公開する**ように変更した(`PoolFileSystem::new`は
データセット名を1つ受け取らなくなり、`mount_pool(pool, mount_point)`と
シグネチャが変わった点に注意)。

- Windowsのファイル名として不正な文字(`\ / : * ? " < > |`)を含む
  データセット名は一覧・オープンどちらからも除外される
  (`mount.rs`の`INVALID_NAME_CHARS`)。
- ディレクトリ階層・create/delete/rename・任意オフセット書き込みは
  引き続き未対応(次の課題として残る)。

**⚠️ 未検証(重要)**: この変更は`winfsp-backend` feature配下
(`#[cfg(feature = "winfsp-backend")]`)のコードであり、実際にコンパイルする
には`winfsp`クレート(→edition2024が必要)とWinFsp SDKが必要。
このセッションの作業環境(Ubuntu, apt版Rust 1.75)ではcargoが古く
`edition2024`を解釈できず、`cargo check --features winfsp-backend`が
`winfsp`クレートのダウンロード段階で失敗するため、**この変更を含む
`mount.rs`は一度もコンパイルできていない**。型の整合性(借用・
match ergonomics・`&[char]`への`str::contains`等)は手動レビューと
最小の再現コードでの確認は行ったが、実際のwinfsp-rs APIとの整合は
Windows実機(Rust 1.85以降、WinFsp SDKインストール済み)で
`cargo test --features winfsp-backend,gpu-accel`を実行して確認すること。
特に`tests/winfsp_mount.rs`は`mount_pool(pool, "tank", MOUNT_POINT)`から
`mount_pool(pool, MOUNT_POINT)`(データセット名引数を削除)へ、
アクセスパスも`\pool.dat`から`\tank`へ追随済みだが、これも同様に未実行。

### 3. `fs_ops.rs`について(要確認事項の切り分け)

`fs_ops.rs`は`mount.rs`とは独立した、別設計時代の骨組み(`ZfsBackend`
トレイト・`DatasetHandle`)であり、`mount.rs`のいかなる型・関数とも
参照し合っていないことを確認した。したがって今回の`mount.rs`の変更に
追随修正すべき箇所は無い。ただし2つの設計(`fs_ops.rs`の
`ZfsBackend`抽象化 と `mount.rs`の`PoolFileSystem`直結実装)が並存して
おり、どちらを正とするか(あるいは統合するか)は未整理のまま。

### 4. 最小Rustバージョンについて

`winfsp` 0.13系が`edition2024`を要求するため、既定feature
(`winfsp-backend`有効)でのビルドには**Rust 1.85以降**が必要
(edition2024が安定化されたバージョン)。古いツールチェインでは
「featureを無効にしたはずなのにビルドできない」というたぐいの
問題ではなく、そもそも`Cargo.toml`のマニフェスト解析時点で失敗する
ため分かりにくいエラーになる点に注意(READMEへの追記candidate)。

### 次のステップ候補(更新版)

1. Windows実機(Rust 1.85+, WinFsp SDK, dxc導入済み)で
   `cargo test --features winfsp-backend,gpu-accel` を実行し、
   本セッションの`mount.rs`変更を実際に検証する。
2. GitHub Actionsに`ubuntu-latest`向けCI
   (`cargo test --no-default-features`、今回の修正で実現可能になった)を
   追加し、リグレッション(今回発見したようなビルド不能バグ)を
   継続的に検知できるようにする。
3. `mount.rs`の任意オフセットread-modify-writeバッファリング層、
   ディレクトリ階層・create/delete/renameへの対応。
4. `fs_ops.rs`と`mount.rs`、どちらの設計を正とするか整理する。
5. (元からの課題)`openruno-installer`の実装確認、
   `feature/raid-z2-z3-scaffolding` → `main`へのPR作成、
   NTFS ACL⇔ZFS ACLのAD/SAM連携の実運用設計。

---

## 追記2: 任意オフセットread-modify-write層の追加(README「現状の制約」のもう1点を解消)

前回セッションで洗い出した3つの実用性課題のうち、**「チャンク境界に揃った
読み書きしかできない(任意オフセット不可)」**を解消した。これは`Pool`層
(純粋なRust/CPUロジックで、Windows/WinFsp/dxc無しで完全にテスト可能)への
追加なので、今回も土台のロジックを壊さずに実機無しで検証できている。

### 変更内容

- `pool.rs`に`Pool::read_unaligned` / `Pool::write_unaligned`を追加。
  要求範囲を含む最小のストライプ境界範囲を計算し、既存の
  `Pool::read`/`Pool::write`(ストライプ境界前提・CoW実装)へ委譲する
  read-modify-write層。バイト単位の任意オフセット・任意長の読み書きを
  提供し、書き込みは対象範囲を丸ごと読み出してから対象部分だけ書き換えて
  書き戻すため、境界からはみ出す未変更バイトは保持される。
- `mount.rs`の`read`/`write`トレイト実装を、`Pool::read`/`Pool::write`
  (ストライプ境界必須)から`Pool::read_unaligned`/`Pool::write_unaligned`
  へ配線し直した。これにより(実機で検証でき次第)WinFspマウント経由でも
  任意オフセットの読み書きができるようになる想定。
- `tests/unaligned_io.rs`を新規追加(5テスト、全てパス確認済み):
  単一ストライプ内の非境界書き込み/読み出しの往復、複数ストライプに
  またがる非境界書き込み、書き込み範囲の前後バイトが変化しないこと
  (read-modify-writeの正しさ)、長さ0の呼び出し、割当容量超過時に
  エラーになり既存データが無傷なままであること、をそれぞれ検証。
- READMEの「主な機能」「現状の制約」「ビルド・テスト」節を、新しいfeature
  構成(`gpu-accel`)と今回の変更に合わせて更新。

### 検証状況

`cargo test --no-default-features`で**全75テスト(既存70+新規5)が
Windows/WinFsp/dxc無しでパス**することを確認済み
(`openzfs-winfsp-bridge`側)。`zfs-accel-hlsl`単体は変更無し(20テスト
引き続きパス)。

**⚠️ `mount.rs`の変更(read/writeの配線変更)自体は、前回同様
`winfsp-backend` feature配下であり、この作業環境ではコンパイルを
一度も試せていない**。ロジック自体は単純な呼び出し先の差し替え
(`pool.read(...)` → `pool.read_unaligned(...)`など、シグネチャは同一)
であり、`pool.rs`側は完全にテスト済みなのでリスクは小さいと判断しているが、
Windows実機での`cargo test --features winfsp-backend,gpu-accel`実行時に
念のため確認すること。

### 残る実用性課題(次回優先度の参考)

1. **ディレクトリ階層・create/delete/rename未対応**(フラットな名前空間のまま)。
   これは`mount.rs`(WinFsp API)に踏み込む変更が必須で、実機無しでは
   検証できない領域。次にWindows実機での検証機会が来たタイミングで
   着手するのが良い。
2. Windows実機(Rust 1.85+, WinFsp SDK, dxc導入済み)での
   `cargo test --features winfsp-backend,gpu-accel`実行(累積2回分の
   `mount.rs`変更をまとめて検証できる)。
3. GitHub Actionsへの`ubuntu-latest`向けCI追加(`cargo test
   --no-default-features`、既に実現可能)。
4. `openruno-installer`の実装確認、`feature/raid-z2-z3-scaffolding` →
   `main`へのPR作成、NTFS ACL⇔ZFS ACLのAD/SAM連携の実運用設計。

---

## 追記3: `Pool::scrub`を実際に呼べるようにした(scrubの「到達不能」バグを解消)

前回セッションで洗い出した3課題のうち残っていた「GPU加速部分・Windows
マウント部分は未検証」は今回も据え置き(実機無しでは検証しない方針の通り)。
代わりに、コードを読み直す中で見つけた**「`scrub`がPool経由では一切
呼び出せない」という抜け**を解消した。これも`pool.rs`/`vdev.rs`/`raid10.rs`
という純粋なRust/CPUロジック層への変更なので、実機無しで完全にテスト可能。

### 見つかった問題

`RaidZVdev::scrub`(チェックサム不一致=サイレント破損の一括検知・修復、
ZFSの`zpool scrub`相当)は既に実装・テスト済みだったが、`Pool`が`vdev`
フィールドを非公開で保持しているため、**`Pool`しか持たない呼び出し側
(`mount.rs`など、実際の利用シーンそのもの)からは`scrub`を一切呼び出せない**
という抜けがあった。また`Raid10Vdev`側には`scrub`自体が存在しなかった
(ミラーグループ横断のスキャンができない)。

### 対応

- `raid10.rs`: `Raid10Vdev::scrub(total_stripes)`を新規実装。グローバル
  ストライプ数を各ミラーグループの担当数(ラウンドロビン配置により、
  割り切れない場合は若い番号のグループが1つ多く担当)へ正しく変換して
  各グループの`RaidZVdev::scrub`へ委譲する。
- `vdev.rs`: `Vdev`トレイトに`scrub`を追加し、`RaidZVdev`・`Raid10Vdev`
  両方の`impl Vdev`で委譲するよう配線(シグネチャが完全に一致していたため
  トレイトへの統一は容易だった)。
- `pool.rs`: `Pool::scrub()`(`self.vdev.scrub(self.total_stripes)`に委譲)
  と`Pool::vdev_mut()`(内部vdevへの可変参照を返すエスケープハッチ)を追加。
  `resilver`は`RaidZVdev`(対象ディスク1個のインデックス)と`Raid10Vdev`
  (ミラーグループ+グループ内インデックスの2階層)とでシグネチャが
  異なるため`Vdev`トレイトへは統一していない。ディスク交換のような
  vdev固有の低頻度操作は`Pool::vdev_mut()`経由で呼ぶ設計とした。

### 追加テスト(全てパス確認済み)

- `src/raid10.rs`(単体テスト2件): 単純なscrub、およびグループ数で割り切れない
  グローバルストライプ数での余り分配ロジックの正しさ(意図的に「余り分」の
  ストライプを破損させ、スキャン漏れが無いことを検証)。
- `tests/pool_scrub.rs`(新規、3件): `Pool::scrub`がRAID-Z(Z2)バックエンドと
  RAID10バックエンドの両方で機能すること、`Pool::vdev_mut`経由でRAID10固有の
  `resilver`を呼び出せること。

### 検証状況

`cargo test --no-default-features`で**全75テストがパス**
(`openzfs-winfsp-bridge`側。内訳は前回70+今回追加5)。`zfs-accel-hlsl`は
変更無し(20テスト引き続きパス)。今回の変更は`winfsp-backend`/`gpu-accel`
どちらのfeatureにも依存しない純粋なコアロジックのみなので、
前回・前々回のような「実機でしか確認できない」リスクは無い。

### 残る実用性課題(更新版)

1. ディレクトリ階層・create/delete/rename ― `mount.rs`必須、実機待ち
2. Windows実機での`cargo test --features winfsp-backend,gpu-accel`
   (これまでの`mount.rs`変更3件分をまとめて検証)
3. CI(GitHub Actions、ubuntu-latestで`--no-default-features`)追加
4. `resilver`を`Vdev`トレイトへ統一するかどうかの設計判断
   (RAID-Z系とRAID10とでディスク指定の階層が異なるため、統一するなら
   「ディスクロケータ」のような共通の抽象化が必要になる)
5. installer実装確認・PR作成・AD/SAM連携

---

## 追記4: partition.rsのSend/Syncバグ修正、エラー種別の整理、重要な発見

前回に引き続き「実装されているのに繋がっていない」系のバグを重点的に
洗い出した。今回は2件の実質的な修正と、今後の開発方針に関わる**重要な
発見**が1件ある。

### 1. `partition.rs`: パーティション分割 × WinFspマウントが原理的に両立不能だったバグ

`PartitionedDevice`が内部で`Rc<RefCell<D>>`を使っていたが、`Rc`は
`Send`/`Sync`のどちらも満たさない。一方`mount.rs::mount_pool`は
`V: Vdev + Send + Sync`を要求する(WinFspが任意のワーカースレッドから
呼び出すため)。つまり**README記載の2つの目玉機能「ディスクの
パーティション分割・使い回し」と「実際のWinFspマウント」が、
組み合わせた瞬間に成立しなくなる**という設計上の欠陥があった
(`dual_role_disk.rs`はパーティション分割単体、`winfsp_mount.rs`は
マウント単体しかテストしておらず、両方を組み合わせるテストが
存在しなかったため見過ごされていた)。

`Arc<Mutex<D>>`へ変更して解消。再発防止として、`mount_pool`と同じ境界
条件(`V: Vdev + Send + Sync`)を独立に検証するコンパイル時regression
テストを追加した(`src/partition.rs`の
`partitioned_device_backed_vdev_satisfies_mount_pools_send_sync_bound`)。

### 2. `error.rs`/`pool.rs`/`vdev.rs`/`raid10.rs`: エラー種別が実質使われていなかった

`BridgeError`には`PoolNotFound`・`AclTranslationFailed`等の意味のある
variantが定義されていたが、`pool.rs`/`vdev.rs`/`raid10.rs`の実装は
**全てのエラーを`BridgeError::Io(std::io::Error::other(..))`という
汎用I/Oエラーに潰していた**ため、呼び出し側は「データセットが無いのか」
「容量不足なのか」「単なるI/Oエラーなのか」を一切区別できなかった。

新規に`DatasetNotFound`・`SnapshotNotFound`・`AlreadyExists`・
`CapacityExceeded`・`InvalidConfig`・`Unrecoverable`を追加し、各エラー
発生箇所を意味の合うvariantへ置き換えた。`tests/error_semantics.rs`
(新規、6テスト)で、呼び出し側が実際に`matches!`でエラー種別を
判別できることを検証済み。

### 3. 【重要な発見】`windows`クレートはWindows以外のターゲットでは中身が完全に空になる

これまでのセッションで「`windows`クレートは`--no-default-features`でも
コンパイルが通っていた」ことを、`mount.rs`のリスクが「WinFsp SDK/dxcが
無いだけ」であるかのように誤って捉えていたが、実際には**もっと根本的な
制約**があることが分かった:

`windows`クレート(v0.58)自体のソース(`lib.rs`)は`#![cfg(windows)]`で
丸ごとガードされており、**コンパイルターゲットが実際にWindowsでない限り、
`windows::Win32::*`以下のあらゆる型・定数が一切存在しない**(クレート自体は
「空の殻」としてコンパイルは通るが、中身は空)。実際に検証したところ、
`windows::Win32::Foundation::STATUS_DATA_ERROR`等をLinux上で`use`しようと
すると`error[E0433]: could not find Win32 in windows`になった。

これまで`--no-default-features`のテストが通っていたのは、`windows::Win32::*`
を実際に参照するコード(`mount.rs`全体、`device.rs`の`gpu` feature配下の
`imp`モジュール、`compute.rs`)が全てfeatureゲートで無効化されていたため、
たまたま一度も踏んでいなかっただけだった(`acl_emulation.rs`の
`sid_placeholder`フィールドにあった「windows::Win32::Security::SIDに
置換予定」はコメントのみで実コードではなかった)。

**この意味するところ**: `mount.rs`・`device.rs`のGPU実装・`compute.rs`は、
WinFsp SDKやdxcが仮に用意できたとしても、**Windows実機(またはWindows
ターゲットへのクロスコンパイル環境)以外では原理的に一切コンパイル
チェックできない**。今回追加した`mount.rs::status_from_bridge_error`の
NTSTATUSマッピング拡張(`BridgeError`の新variantを`STATUS_OBJECT_NAME_
COLLISION`・`STATUS_DISK_FULL`・`STATUS_INVALID_PARAMETER`・
`STATUS_DATA_ERROR`・`STATUS_NOT_IMPLEMENTED`へ対応させた)も、
定数名・値は一般に知られたNTSTATUSコードとして記載したが、
**windows-rsが実際にこれらの識別子をこの通りに公開しているかは
Windows実機でのビルドでしか確認できない**。

### 検証状況

`cargo test --no-default-features`で**全82テスト**
(前回75+`partition.rs`のregression 1+`error_semantics.rs` 6)がパス。
`mount.rs`関連の変更(status_from_bridge_errorのNTSTATUSマッピング拡張)は
上記の理由により今後も実機でしか検証できない。

### 残る実用性課題(更新版)

1. ディレクトリ階層・create/delete/rename ― `mount.rs`必須、実機待ち
2. Windows実機での`cargo test --features winfsp-backend,gpu-accel`
   (これまでの`mount.rs`変更4件分をまとめて検証)
3. CI(GitHub Actions)追加。**Linux runnerでは`--no-default-features`のみ
   有効**であることに注意(`windows`クレートの制約上、`gpu-accel`/
   `winfsp-backend`を含むテストはWindows runnerでしか実行できない)。
4. `resilver`を`Vdev`トレイトへ統一するかどうかの設計判断
5. installer実装確認・PR作成・AD/SAM連携
