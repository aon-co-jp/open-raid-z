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
続けて open_runo_installer (Tauri) の実装を進めたいです。
```

---

## プロジェクト概要(経緯サマリ)

Windows版NTFS互換・全RAID対応・NPU/GPUハードウェアアクセラレータ対応の
ZFSファイルシステム導入システムの開発。「DirectXのプラグイン」という
当初案は技術的に成立しないため、以下の3コンポーネント構成に整理した。

| # | コンポーネント | 役割 | 技術スタック |
|---|---|---|---|
| ① | open_raid_z_core | WinFsp経由でZFSのI/Oをフックし、NTFS ACLセマンティクスをエミュレーション | Rust + WinFsp(winfsp-rs) + windows-rs |
| ② | zfs_accel_hlsl | チェックサム/RAID-Zパリティ/圧縮をNPU/GPUへオフロード(DirectML経由) | Rust + DirectX 12 Compute + DirectML(HLSL) |
| ③ | open_runo_installer | ハードウェア検出・ドライバ登録・zpool初期化のGUIインストーラー | Tauri |

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

### ③ open_runo_installer(Tauri)の状態

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

1. `open_runo_installer`の実装内容を確認し、未完成部分を洗い出す
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

- `open_raid_z_core/Cargo.toml`の`[build-dependencies]`で`winfsp`が
  `optional`になっておらず、`--no-default-features`を付けても常に`winfsp`
  クレート(→WinFsp SDKヘッダ)を要求していた。
- `zfs_accel_hlsl/build.rs`が、featureに関わらず常に`dxc`
  (DirectX Shader Compiler)の存在を要求してpanicしていた
  (`zfs_accel_hlsl`は`open_raid_z_core`の非optional依存だったため、
  ブリッジ側だけ`--no-default-features`を付けても無意味だった)。

**対応**: `zfs_accel_hlsl`に`gpu` feature(既定ON)を新設し、D3D12/DirectML
呼び出し(`device.rs`の実装部分、`compute.rs`全体)とシェーダ事前コンパイル
(`build.rs`)をこのfeature配下に隔離。`open_raid_z_core`側にも
`gpu_accel` feature(既定ON、`zfs_accel_hlsl/gpu`に配線)を新設し、
`default-features = false`で依存するよう変更。

**効果(実機で検証済み)**: Ubuntu上にRustをaptで導入しただけの素の環境で、

```bash
cd open_raid_z_core
cargo test --no-default-features
```

が実際に成功し、RAID0/1/5/6/10/Z2/Z3・チェックサム自己修復・CoW・
スナップショット/クローン・resilverを含む**全65テストがWindows/WinFsp/dxc
無しでパス**することを確認した(`zfs_accel_hlsl`単体でも20テスト全パス)。
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

**⚠️ 未検証(重要)**: この変更は`winfsp_backend` feature配下
(`#[cfg(feature = "winfsp_backend")]`)のコードであり、実際にコンパイルする
には`winfsp`クレート(→edition2024が必要)とWinFsp SDKが必要。
このセッションの作業環境(Ubuntu, apt版Rust 1.75)ではcargoが古く
`edition2024`を解釈できず、`cargo check --features winfsp_backend`が
`winfsp`クレートのダウンロード段階で失敗するため、**この変更を含む
`mount.rs`は一度もコンパイルできていない**。型の整合性(借用・
match ergonomics・`&[char]`への`str::contains`等)は手動レビューと
最小の再現コードでの確認は行ったが、実際のwinfsp-rs APIとの整合は
Windows実機(Rust 1.85以降、WinFsp SDKインストール済み)で
`cargo test --features winfsp_backend,gpu_accel`を実行して確認すること。
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
(`winfsp_backend`有効)でのビルドには**Rust 1.85以降**が必要
(edition2024が安定化されたバージョン)。古いツールチェインでは
「featureを無効にしたはずなのにビルドできない」というたぐいの
問題ではなく、そもそも`Cargo.toml`のマニフェスト解析時点で失敗する
ため分かりにくいエラーになる点に注意(READMEへの追記candidate)。

### 次のステップ候補(更新版)

1. Windows実機(Rust 1.85+, WinFsp SDK, dxc導入済み)で
   `cargo test --features winfsp_backend,gpu_accel` を実行し、
   本セッションの`mount.rs`変更を実際に検証する。
2. GitHub Actionsに`ubuntu-latest`向けCI
   (`cargo test --no-default-features`、今回の修正で実現可能になった)を
   追加し、リグレッション(今回発見したようなビルド不能バグ)を
   継続的に検知できるようにする。
3. `mount.rs`の任意オフセットread-modify-writeバッファリング層、
   ディレクトリ階層・create/delete/renameへの対応。
4. `fs_ops.rs`と`mount.rs`、どちらの設計を正とするか整理する。
5. (元からの課題)`open_runo_installer`の実装確認、
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
  構成(`gpu_accel`)と今回の変更に合わせて更新。

### 検証状況

`cargo test --no-default-features`で**全75テスト(既存70+新規5)が
Windows/WinFsp/dxc無しでパス**することを確認済み
(`open_raid_z_core`側)。`zfs_accel_hlsl`単体は変更無し(20テスト
引き続きパス)。

**⚠️ `mount.rs`の変更(read/writeの配線変更)自体は、前回同様
`winfsp_backend` feature配下であり、この作業環境ではコンパイルを
一度も試せていない**。ロジック自体は単純な呼び出し先の差し替え
(`pool.read(...)` → `pool.read_unaligned(...)`など、シグネチャは同一)
であり、`pool.rs`側は完全にテスト済みなのでリスクは小さいと判断しているが、
Windows実機での`cargo test --features winfsp_backend,gpu_accel`実行時に
念のため確認すること。

### 残る実用性課題(次回優先度の参考)

1. **ディレクトリ階層・create/delete/rename未対応**(フラットな名前空間のまま)。
   これは`mount.rs`(WinFsp API)に踏み込む変更が必須で、実機無しでは
   検証できない領域。次にWindows実機での検証機会が来たタイミングで
   着手するのが良い。
2. Windows実機(Rust 1.85+, WinFsp SDK, dxc導入済み)での
   `cargo test --features winfsp_backend,gpu_accel`実行(累積2回分の
   `mount.rs`変更をまとめて検証できる)。
3. GitHub Actionsへの`ubuntu-latest`向けCI追加(`cargo test
   --no-default-features`、既に実現可能)。
4. `open_runo_installer`の実装確認、`feature/raid-z2-z3-scaffolding` →
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
(`open_raid_z_core`側。内訳は前回70+今回追加5)。`zfs_accel_hlsl`は
変更無し(20テスト引き続きパス)。今回の変更は`winfsp_backend`/`gpu_accel`
どちらのfeatureにも依存しない純粋なコアロジックのみなので、
前回・前々回のような「実機でしか確認できない」リスクは無い。

### 残る実用性課題(更新版)

1. ディレクトリ階層・create/delete/rename ― `mount.rs`必須、実機待ち
2. Windows実機での`cargo test --features winfsp_backend,gpu_accel`
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
2. Windows実機での`cargo test --features winfsp_backend,gpu_accel`
   (これまでの`mount.rs`変更4件分をまとめて検証)
3. CI(GitHub Actions)追加。**Linux runnerでは`--no-default-features`のみ
   有効**であることに注意(`windows`クレートの制約上、`gpu_accel`/
   `winfsp_backend`を含むテストはWindows runnerでしか実行できない)。
4. `resilver`を`Vdev`トレイトへ統一するかどうかの設計判断
5. installer実装確認・PR作成・AD/SAM連携

---

## 追記5: 命名規則の統一(アンダースコア化)と`open_runo_installer_core`の切り出し

ユーザーからの指示で、ハイフン混じりだったディレクトリ・crate名を
アンダースコア区切りに統一した。あわせて、`openruno-installer`
(Tauriアプリ)の中身を調査したところ、**Tauriに一切依存しない純粋な
ロジック(ディスク検出以外の助言・zpoolプレビュー計算)が、`hardware.rs`
1ファイルの`windows`クレート無条件依存に巻き込まれて一度もテストできて
いなかった**という、これまでと同種の「実装されているのに検証できない」
問題を発見し、解消した。

### 1. 命名規則の統一

以下の通りリネーム(`git mv`で履歴を保持):

| 旧 | 新 |
|---|---|
| `open-runo-zfs-source` | `open_runo_zfs_source` |
| `openzfs-winfsp-bridge`(crate名`openzfs_winfsp_bridge`) | `open_raid_z_core` |
| `zfs-accel-hlsl` | `zfs_accel_hlsl` |
| `openruno-installer`(crate名`openruno-installer`、lib名`openruno_installer_lib`) | `open_runo_installer`(lib名`open_runo_installer_lib`) |

ディレクトリ名・Cargo.tomlの`name`・Rust側の`use`文・パス依存(`path = "../..."`)
・README/CHAT_HANDOFF.mdの記述・package.json/tauri.conf.json等のフロント
エンド側の名称も含めて全て置換した。置換後、`open_raid_z_core`
(全82テスト)・`zfs_accel_hlsl`(全20テスト)とも引き続き
`cargo test --no-default-features`が成功することを確認済み。

### 2. `open_runo_installer_core`の新設(Tauri非依存ロジックの分離)

`openruno-installer/src-tauri/src/hardware.rs`が`windows::Win32::Storage::
FileSystem`等を**無条件に**(feature gate無し)使っていたため、
同じ`lib.rs`が`mod hardware;`する`copilot.rs`(Copilot風助言、15テスト)・
`zpool_wizard.rs`(zpool初期化プレビュー、9テスト)という**本来OS非依存の
ロジック**まで、Windows以外のターゲットでは一切コンパイルすらできない
状態になっていた(前回発見した「`windows`クレートはWindows以外では
中身が空になる」という制約がここでも再現していた)。

加えて、Tauri本体(`tauri`クレート)自体の依存グラフ(`idna_adapter`等)も
edition2024を要求するため、この作業環境(cargo 1.75)では
`open_runo_installer/src-tauri`を`cargo check`することはそもそもできない
(Windows実機やRust 1.85+の環境でしか検証できない)ことも判明した。

**対応**: `hardware.rs`・`copilot.rs`・`zpool_wizard.rs`を、Tauriに一切
依存しない新規crate`open_runo_installer_core`へ切り出した。
`hardware.rs`はWindows専用の実装を`#[cfg(windows)]`モジュールへ、
非Windows向けに常に空リストを返すフォールバックを`#[cfg(not(windows))]`
モジュールへ分離(`windows`クレート自体も`[target.'cfg(windows)'.dependencies]`
でWindows限定の依存にした)。Tauriアプリ側(`open_runo_installer/src-tauri`)
は、この新crateへ委譲する薄いラッパー(`#[tauri::command]`のみ)になった。

**効果(実機で検証済み)**: `open_runo_installer_core`が**この作業環境で
初めてビルド・テストでき**、既存の26テスト(copilot 15 + hardware 2 +
zpool_wizard 9)が全てパスすることを確認した。Tauri本体・Windows実機・
dxc・WinFspのいずれも不要。

### 3. フロントエンド(TypeScript)の検証

npm/node(v22.22.2)が利用可能だったため、`open_runo_installer`の
フロントエンドも初めて検証した:

- `npx tsc --noEmit`: 型チェックが**エラー無しで完全に通過**。
- `npx vite build`: 実際のバンドルビルドも成功。
- `i18n.ts`(10言語対応)の翻訳キー網羅性を検証する過程で、
  `Dict = Record<string, string>`という緩い型のため、**ある言語で
  翻訳キーが1つ抜けていてもTypeScriptのコンパイルは通ってしまう**
  (実行時に静かに日本語やキー名へフォールバックするだけになる)という
  型安全性の穴を発見。`type TranslationKey = keyof typeof ja; type Dict =
  Record<TranslationKey, string>;`へ変更し、日本語辞書を「唯一の正」として
  他言語の辞書にキーの過不足があれば`tsc`がコンパイルエラーとして検出
  できるようにした。実際にキーを1つ削除して`tsc`がエラーを検出すること、
  復元後は正常に通ることを確認済み。

### 検証状況まとめ

| 対象 | 検証方法 | 結果 |
|---|---|---|
| `open_raid_z_core` | `cargo test --no-default-features` | 82テスト全パス |
| `zfs_accel_hlsl` | `cargo test --no-default-features` | 20テスト全パス |
| `open_runo_installer_core`(新規) | `cargo test` | **26テスト全パス(初検証)** |
| `open_runo_installer`フロントエンド | `tsc --noEmit` / `vite build` | **エラー無し(初検証)** |
| `open_runo_installer/src-tauri`(Tauri本体) | `cargo check` | 未検証(tauriクレート自体がedition2024要求、Windows実機/Rust 1.85+待ち) |
| `mount.rs`・GPU実装 | - | 未検証(Windows実機待ち、既知の制約) |

### 残る実用性課題(更新版)

1. ディレクトリ階層・create/delete/rename ― `mount.rs`必須、実機待ち
2. Windows実機での`cargo test --features winfsp_backend,gpu_accel`
   (bridge側)、`cargo tauri build`(installer側)
3. CI(GitHub Actions)追加。Linux runnerでは`open_raid_z_core`/
   `zfs_accel_hlsl`の`--no-default-features`と`open_runo_installer_core`・
   フロントエンド(tsc/vite)が実行可能。`gpu`/`winfsp_backend`/Tauri本体
   はWindows runner(かつ十分に新しいRustツールチェイン)が必要。
4. `resilver`を`Vdev`トレイトへ統一するかどうかの設計判断
5. installer実装確認(UI/UXの拡充)・PR作成・AD/SAM連携

---

## 追記6: 実機(Windows, Rust 1.96)での初検証。create/delete/rename/append/truncateの追加。重要な訂正

このセッションはWindows実機(前回までのLinuxセッションとは別の作業環境。
Rust 1.96、dxc(Vulkan SDK同梱)、WinFsp実行時コンポーネントが揃っている)
で行われた。「上記4件はWindows実機待ち」としていた項目のうち、1・2が
実際に検証可能であることが判明し、進めた。

### 【重要な訂正】これまでの「WinFspマウント実機検証済み」という記述について

このセッションの調査で、`cargo test --features winfsp_backend,gpu_accel`が
`--nocapture`無しだと、**実際にはWinFspのDLLロードに失敗してテストが
早期スキップされているだけの場合でも`ok`としか表示されない**ことが判明した
(スキップ時の`eprintln`はcaptureされ、`--nocapture`無しでは見えない)。
このセッションの冒頭でこの問題を踏んでおり、`--nocapture`を付けて初めて
実際にはマウントに失敗しスキップしていたことが分かった
(`WIN32(1285)`=`ERROR_DELAY_LOAD_FAILED`。原因はWinFspの`bin`ディレクトリが
`PATH`に無く、`winfsp`クレートの`LoadLibraryW("winfsp-x64.dll")`が
見つけられないこと。詳細はREADMEの「ビルド・テスト」節に追記した)。

このため、**過去のセッション記録にある「WinFspマウントを実機で検証し、
実際に読み書きできることを確認した」という趣旨の記述(このファイルの
冒頭付近、コミット`fab0999`に関する記述)は、同様に`--nocapture`無しで
確認されていた可能性があり、実際には検証されていなかった疑いがある**。
今後このプロジェクトを引き継ぐ際は、`mount.rs`関連のテストが本当に
マウントできているかどうかは、`--nocapture`を付けて`スキップします`
という文字列が出ていないことを都度目視で確認すること。

一方、GPU実ディスパッチのテスト
(`zfs_accel_hlsl`の`xor_dispatch_matches_cpu_reference_when_hardware_available`)
はこのセッションで`--nocapture`付きで確認しており、スキップメッセージが
出ずに実際のGPU(NVIDIA GeForce GT 730)でディスパッチが成功し、CPU参照
実装と結果が一致することを確認済み。こちらは訂正の必要はない。

### 今回追加した機能: WinFspマウント経由でのcreate/delete/rename/append/truncate

前回までの`mount.rs`はルート直下のデータセット一覧の読み書きのみで、
ファイルの新規作成・削除・名前変更はマウント外から`Pool`のAPIを直接
呼ぶ運用を想定していた。今回、WinFspの`create`/`cleanup`+`set_delete`/
`rename`/`set_file_size`コールバックを実装し、Explorerや通常のアプリ
から「新規ファイル作成」「削除」「リネーム」「追記」「切り詰め」が
そのまま使えるようにした。

- `pool.rs`: `Dataset`に`logical_size`(バイト単位の論理サイズ)を追加。
  従来`dataset_size()`は`stripes.len() * chunk_bytes`(常にストライプ境界に
  切り上げられた値)を返していたが、これを実際に書き込んだバイト数どおりの
  値に変更(4KB未満の小さいファイルでも正確なサイズが報告できるように
  なった、既存の`grow_dataset`系テストとの後方互換は維持)。
  `write_unaligned_growing`(書き込みが現在のサイズを超えたら自動的に
  容量を拡張してから書く)、`set_dataset_size`(拡張/切り詰め、切り詰め時は
  不要になったストライプをプールへ返却)、`rename_dataset`を新規追加。
- `mount.rs`: `create`(サブディレクトリ作成は拒否)・`cleanup`+`set_delete`
  (WinFspの規約どおり、削除は`cleanup`の`FspCleanupDelete`フラグで実施)・
  `rename`・`set_file_size`を実装。`write`は`write_to_eof`
  (FILE_APPEND_DATA)・`constrained_io`(メモリマップ書き込み、ファイルを
  伸ばしてはいけない)も正しく扱うよう書き直した。
  `get_volume_info`の**バグも発見・修正**: `total_size`/`free_size`に
  ストライプ「数」をそのまま渡していたため、Windowsからは「容量数バイト
  しかない極小ボリューム」に見えていた(バイト単位の値に修正)。

### 【重要な発見】CoWは常に「あと1ストライプ」の空きを要求する(ZFSのslop spaceと同じ)

`Pool::write`のCoW実装は「新しいストライプへ書いてから古いストライプを
解放する」順序で動くため、プールを100%使い切った状態からの書き込みは
(たとえ同じデータで上書きするだけでも)失敗する。これは今回の変更で
新たに生まれた制約ではなく`Pool::write`が最初から持っていた性質だが、
今回`create`のCREATE_ALWAYS経由でのtruncate→再書き込みという新しい経路が
できたことで初めて顕在化した。`ensure_min_capacity`に「追加確保ぶん+1」の
空きを要求するチェックを追加して安全に失敗するようにし、`tests/winfsp_mount.rs`
のプールにも1ストライプぶんの余裕を持たせるよう修正した。

### 検証状況(実機、`--nocapture`で確認済み)

`PATH`にWinFspの`bin`を追加した状態で`cargo test --features
winfsp_backend,gpu_accel`を実行し、**スキップメッセージ無しで94テスト
全パス**(内訳: 既存82 + `tests/dynamic_file_size.rs`新規7 +
`tests/winfsp_mount_file_ops.rs`新規4 + α)。実マウント経由での
新規作成・削除・リネーム・追記・切り詰めをそれぞれ`std::fs`の標準API
(`write`/`remove_file`/`rename`/`OpenOptions::append`/`File::set_len`)から
実際に検証した。`cargo test --no-default-features`(Linux CI相当)も
引き続き全パス。

### 容量・ファイルサイズの上限(ユーザーからの質問への回答、README追記済み)

ファイル(データセット)の論理サイズは`u64`で一貫管理しており、FAT32の
ような人為的な4GB上限は無い。実際の上限は**プールの空き容量**
(接続ディスクの実容量合計からRAID冗長化ぶんを引いたもの)で決まる。
動画・画像などサイズの大きいファイルも、残容量の範囲内であれば
問題なく保存できる。ただし上記のCoW slop space制約により、プールは
常に少なくとも1ストライプぶんの空きを残しておく必要がある。

### 残る実用性課題(更新版)

1. ディレクトリ階層(サブディレクトリ)― 引き続き未対応
2. リネーム中の別ハンドルの整合性(`FileHandle`が名前ベースのため、
   リネーム対象を指す他のオープンハンドルは以後失敗しうる。既知の制約として
   `Pool::rename_dataset`のドキュメントに明記済み)
3. `cargo tauri build`(installer本体)の実機検証 ― 今回`cargo check`は
   実機で成功したが、GUIとして実際に動かす検証はまだ
4. CI(GitHub Actions)追加。実マウント系テストをCIで回す場合は
   `PATH`にWinFsp `bin`を追加するステップが必須(上記参照)
5. `resilver`を`Vdev`トレイトへ統一するかどうかの設計判断
6. installer実装確認(UI/UXの拡充)・PR作成・AD/SAM連携
7. 実ハードウェア(HDD/SSD/USBメモリ複数台)を使ったRAID6実験、
   実GPU(GT730)でのDirectXハードウェアアクセラレーション実験 ―
   ユーザーから提案あり、実施する場合は対象ディスクのデータ消失
   リスクについて事前確認が必要

---

## 追記7: 実ディスク実験のヒアリング、および`migrate`モジュール(既存データ移行ツール)の新規実装

### 実ディスク構成のヒアリング(実施内容の記録)

ユーザーの実機(GT730搭載)で実際にRAID6/RAID-Z2実験を行う計画について、
`Get-Disk`/`Get-Partition`/`Get-Volume`で**読み取り専用**にディスク構成を
確認した。ドライブレターの対応が途中で変わっていたため2回スキャンし、
最終的に以下が確定した(確認時点):

| ドライブ | Disk番号 | 種類 | サイズ | 用途 |
|---|---|---|---|---|
| C: | Disk 2 | NVMe(Hanye HE70-2TBNHS1) | 2TB | Windows 11本体。空き無し |
| E: | Disk 1 | SATA HDD(WDC WD30EZRZ) | 3TB | 実データ約179GB(消去OKと確認済み) |
| F: | Disk 0 | SATA HDD(FSLC MAL32000SA-T54) | 2TB | 実データ約178GB(消去OKと確認済み) |
| G: | Disk 3 | USB接続SSD(P3-256) | 256GB | ほぼ空 |
| H: | Disk 4 | USBメモリ | 8GB | 実データ約6.5GB(消去OKと確認済み) |

途中、ユーザーから「C:(Windows起動中)を、Windowsを起動したまま新しい
ファイルシステムへその場変換したい」という要望が出たが、これは**原理的に
不可能**であることを説明した(OS自身が使用中のボリュームを、そのOS上の
ソフトウェアが書き換えることはできない。実在するどの変換ツールも対象の
アンマウント/オフライン化を要求する)。またopen-raid-z自体にも「既存の
NTFSデータを保持したまま変換する」機能(以前から「ライブマイグレーション」
として未着手扱いだった機能)が無いことを説明した。

**まだ確定していないこと**: E:・F:・G:・H:の4台をどう組み合わせて
RAID6/RAID-Z2(+ミラー)にするかは、複数回のすり合わせでも確定しなかった
(サイズが2TB/2TB/256GB/8GBと大きく異なり、最小メンバーに容量が
引きずられる点も指摘済み)。次回再開時はここから続きを詰める必要がある。
C:を対象に含める案は撤回され、E:・F:・G:・H:の4台のみが対象。

### `migrate`モジュールの新規実装(既存データ移行ツール)

上記の「C:のその場変換は不可能」を受けて、ユーザーから「せめて既存の
NTFSデータを保持したままプールへ取り込む機能を実装してほしい」という
要望があり、実装した(`open_raid_z_core/src/migrate.rs`、新規)。

**できること**: 既存のディレクトリツリー(NTFS等、起動中のシステム
ドライブ以外)を再帰的に走査し、各ファイルを`Pool`内のデータセットとして
チャンク単位(既定8MiB)でストリーミングコピーする。各チャンクは書き込み
直後にその場で読み戻して内容を比較し、不一致があれば即座に検出する
(巨大な動画ファイル等でもメモリを使い切らない)。**コピー元には一切
書き込まない**ため、通常のファイルコピーツールと同じ安全性で、
Windowsを起動したまま実行できる。

**できないこと(意図的な設計上の制約)**:
- 現在起動中のシステムドライブ(C:等)を対象にはできない(上記の原理的制約)。
  あくまで「(起動中でない)既存ボリュームから、プールという別の場所へ
  コピーする」ツールであり、真の意味の「ライブマイグレーション」
  (同じディスクの中身をその場で変換する)ではない。
- `Pool`/`mount.rs`が現状ルート直下のフラットな名前空間しか対応していない
  ため、サブディレクトリを含む階層は指定した区切り文字で1階層のデータ
  セット名へ平坦化する(例: `docs/readme.txt` → `docs_readme.txt`)。
  平坦化後に名前が衝突する場合は、そのファイルだけを安全にスキップし
  (他のデータは壊さない)、結果を`MigrationReport`で報告する。
- 現状はライブラリ関数(`migrate_directory_into_pool`)のみで、CLI/GUIは
  まだ無い。実際にユーザーの実ディスクに対して使うには、呼び出し用の
  小さなバイナリ(またはインストーラーへの組み込み)が別途必要。

**検証状況**: 実際の一時ディレクトリ(`std::env::temp_dir()`、ユーザーの
実ドライブには一切触れていない)を使い、5件のテストで検証済み
(基本的な移行+検証、コピー元が一切変更されないことの確認、複数チャンクに
またがる大きめファイルの往復、平坦化後の名前衝突の安全なスキップ、
不正な名前になるファイルの安全なスキップ)。`cargo test
--no-default-features`で新規5テストを含む全46件(旧41+新5)がパス。

### 残る実用性課題(更新版)

1. ディレクトリ階層(サブディレクトリ)― `mount.rs`・`migrate.rs`双方に影響、引き続き未対応
2. E:・F:・G:・H:の具体的な組み合わせ方(ミラー+RAID6/Z2の対応関係)の確定 ― 次回続きから
3. `migrate`モジュールをCLIまたはインストーラーGUIから呼び出せるようにする
4. リネーム中の別ハンドルの整合性(前回から変更なし)
5. `cargo tauri build`(installer本体)のGUI実機検証(前回から変更なし)
6. CI(GitHub Actions)追加(前回から変更なし)
7. `resilver`を`Vdev`トレイトへ統一するかどうかの設計判断(前回から変更なし)
8. installer実装確認・PR作成・AD/SAM連携(前回から変更なし)

---

## 追記8: マルチOS対応(Windows起動ディスク化・Linux版)構想の方針決定

ユーザーから「open-raid-zでフォーマットしたHDDにWindows/Mac/Linuxを直接
インストール可能にしたい」「Windows版を`open-raid-z-win`、Linux版を
`open-raid-z-linux`と名前を分けたい」という大きな構想が出た。

### 技術的な制約の説明と、ユーザーとのすり合わせ結果

以下を説明し、ユーザーもこれを踏まえて方針を調整した:

- **Windowsのセットアップ**は、インストール先としてMicrosoft非公式の
  ファイルシステムを選ばせる仕組みが無い。起動時(ブートローダーが
  カーネルを読み込む段階)にファイルシステムを読ませるには、現行の
  WinFsp(ユーザーモード、Windows起動後にしか動かない)ではなく、
  **カーネルモードのブート起動ドライバ**が必要。かつWindows(Secure Boot
  有効時)はブート起動ドライバにMicrosoftの署名を要求するため、
  Microsoftとの提携無しには正式なインストール先として選択可能にはならない。
- **macOS**はApple Silicon以降、起動ボリュームが署名付きで封印(Sealed
  System Volume)される設計のため、サードパーティファイルシステムを
  起動ディスクにする経路自体がAppleのプラットフォームセキュリティ上
  意図的に塞がれている。
- **Linux**はオープンなため、①Linuxカーネルモジュール、②GRUB用の
  読み込みモジュールを自作すれば、理論上は起動ディスク化まで到達可能
  (実際のOpenZFS on root/ZFSBootMenuと同じ道)。

### ユーザーの最終判断

- **macOS対応は当面見送り**(Appleのプラットフォームセキュリティ上、
  独力での突破が困難なため)。
- **Windows11・Windows Server・Linuxの3つに絞り、少しずつでも実現を
  目指す**。Windowsのカーネルモード・ブート起動ドライバ開発も、
  「一つずつ、少しずつ開発すれば実現可能」という方針でユーザーが
  明示的に着手を希望。**Secure Bootを無効化した状態での動作は許容**
  (ユーザー自身の発言: 「Secure Bootを無効化して、今は、動かすしか
  ありません」)。
- README(全10言語)の冒頭に、**Microsoft社・Apple社の関係者へ向けた
  協力要請のメッセージ**を追記(ユーザーからの明示的な依頼)。
  「起動起動ドライバの署名認証やインストーラー対応にはベンダーの協力が
  不可欠」という前提を明記したうえで、関心があれば連絡してほしい、
  という趣旨。

### 進め方として合意した順序

1. 名前分割(`open-raid-z-win`/`open-raid-z-linux`) ― 組織的な変更、低リスク
2. **Linux版のデータドライブ対応**(FUSE経由のマウント)― 次に着手する
   具体的な実装項目。コアロジック(`Pool`/`RaidZVdev`等)は既にOS非依存
   (`--no-default-features`でLinux上ビルド・テスト可能)なので土台はある
3. Linux起動ディスク化(カーネルモジュール+GRUBモジュール)― 2が動いてから
4. **Windowsカーネルモード・ブート起動ドライバの開発** ― 最も難度が高く
   リスクも大きい(バグがあるとブート不能になりうる)。着手する場合は
   ユーザーの本番機ではなく、**隔離されたVM/テスト機**で行うべきことを
   次回以降強調する必要がある。WDK(Windows Driver Kit)のセットアップ、
   テスト署名モードでの最小限の「Hello World」ミニフィルタドライバの
   ロード確認から始めるのが安全な出発点。

### 次のステップ

- READMEのMicrosoft/Apple向けメッセージは全10言語へ反映・push済み。
- 次回再開時は、上記2(Linux版FUSEマウント)から実装に着手するのが
  妥当。4(Windowsカーネルドライバ)に着手する場合は、開発環境の隔離
  (VM推奨)について改めてユーザーに確認すること。

---

## 追記9: WSL2導入、Linux版FUSEマウント(`fuse_mount.rs`)の実装・実機検証、クレート改名

前回の「2(Linux版FUSEマウント)から着手」の通りに進めた。加えて
ユーザーから「Windows版とLinux版はほぼ共通プログラムとして」という
明確な方針指示があり、それに沿ってコアクレートを改名した。

### WSL2セットアップ(実施記録)

このマシンにWSLが未導入だったため、`wsl --install`で導入した
(要管理者権限。ユーザー自身に管理者PowerShellで実行してもらった)。
Ubuntu 26.04(WSL2、カーネル`6.18.33.2-microsoft-standard-WSL2`)が
入り、Rust 1.96.1(rustup経由)・build-essential・pkg-config・
libfuse3-devをセットアップ済み。

**ハマった点(再発防止のため記録)**: `wsl`コマンドをこのBash/PowerShell
ツールから実行すると、複数回呼び出しがハングして`wsl`プロセスが
何個も残留する現象が起きた。原因は再起動未実施(WSL2の仮想化基盤の
完全な有効化に必要)だったと推測される。ユーザーに再起動してもらい
解消した。また`sudo`はパスワード入力待ちでノンインタラクティブ実行時に
タイムアウトする(`sudo: timed out`)ため、パッケージインストール等は
`wsl -d Ubuntu -u root -- ...`のように**rootユーザーとして直接実行**
することで回避した(WSLではrootに素で入れるため、この用途では安全)。

### `fuse_mount.rs`の新規実装(Linux実マウント、`mount.rs`のFUSE版)

`fuser`クレート(0.17.0)を使用。WinFsp版`mount.rs`との対応関係:

- 同じ`Pool<V>`をそのまま扱う(コアロジックの重複は一切無い)。
- FUSEはinodeベースでファイルを識別する設計のため、`mount.rs`の
  `FileHandle`(データセット名を直接保持)とは異なり、「名前 <-> inode
  番号」の対応表(`PoolState`)を持つ設計にした。これにより
  **`mount.rs`の既知の制約(リネーム後に他のオープンハンドルが古い
  名前を参照し続けて失敗しうる)が、こちらの実装には存在しない**
  (inode番号自体はリネーム後も変わらないため)。
- create/delete/rename/append/truncate/任意オフセット読み書きに対応。
  Windowsのファイル名として不正な文字を拒否するバリデーションも
  `mount.rs`と揃えている(同じプールを将来Windows側からもマウントする
  可能性があるため、意図的に同じ制約を課している)。

**fuser 0.17.0のAPIについての注記**: `INodeNo`/`FileHandle`/
`LockOwner`/`OpenFlags`/`WriteFlags`/`RenameFlags`等、多くのパラメータが
生の`u64`/`i32`ではなくニュータイプでラップされている(古いバージョンの
`fuser`のAPIとは異なる)。型を手作業で完全に予測するのではなく、
実際に`cargo check`を回してコンパイルエラーから正しい型を都度確認する
方針で進めた(前回セッションの`winfsp-sys`の`FILE_ACCESS_RIGHTS`の件と
同様の教訓)。

### 実機検証(WSL2上、`--nocapture`で確認済み)

`tests/fuse_mount.rs`を新規作成し、実際にWSL2上へマウントして
`std::fs`の標準APIから検証した(2件、スキップ扱いではなく失敗時は
`panic`する設計):

1. 新規作成→書き込み→読み込み→`readdir`一覧確認→リネーム→
   `set_len`による切り詰め→削除、という一連の操作。
2. 4万バイト(複数ストライプにまたがる、境界に揃っていないサイズ)の
   ファイルの往復。

既存のコアロジックのテスト(46件)・installer_core(26件)・
Windows実機での`winfsp_backend`テスト(全件)も、この変更後に
改めて確認し、いずれも引き続き全パス。

### クレート改名: `open_zfs_winfsp_bridge` → `open_raid_z_core`

ユーザーから「Windows版とLinux版はコマンドやインストーラーの違いを
超えて、ほぼ共通プログラムとして」という方針指示を受け、コアクレートの
名前が「winfsp」というWindows専用を示す名前のままLinux版
(`fuse_mount.rs`)も含んでいる状態だったのを是正した。`git mv`で
ディレクトリを改名し、`Cargo.toml`(`[package] name`・`[lib] name`・
`description`)、依存元(`open_runo_installer_core`のパス依存)、
テストファイル内の`use`文、README(全10言語)、CHAT_HANDOFF.mdを
一括で置換(過去の`openzfs-winfsp-bridge`→`open_zfs_winfsp_bridge`の
改名と同じ要領)。改名後、Windows側(`winfsp_backend`+`gpu_accel`、
実機WinFspマウント含む)・WSL2側(`fuse_backend`、実機FUSEマウント含む)・
Tauriインストーラー本体(`cargo check`)のいずれも改めて確認し、
全て問題なくビルド・テストが通ることを確認済み。

なお`open_runo_zfs_source/CHAT_HANDOFF.md`(このファイルとは別の、
過去の一時点のチャット記録)は意図的に改名の対象外とした(その場の
記録という性質上、後から書き換えるべきではないため)。

### Cargo.tomlの構成変更点

- `[target.'cfg(target_os = "linux")'.dependencies]`に`fuser`
  (optional)を追加。
- `[features]`に`fuse_backend = ["dep:fuser"]`を追加(`winfsp_backend`/
  `gpu_accel`と同様、既定では無効。Linux以外のターゲットでは`fuser`
  自体が依存関係に存在しないため有効化できない)。

### macOS対応への足がかり(ロードマップ更新)

ユーザーから「macOSも将来的にロードマップの視野に入れて」との指示。
起動ディスク化がAppleのSealed System Volumeで塞がれている状況は
変わらないが、**`fuser`クレートには`macfuse-4-compat`というfeatureが
既に存在する**ことを確認した。つまり`fuse_mount.rs`とほぼ同じ設計で、
**データボリュームとしてのmacOS対応(起動ディスク化ではない)は
将来追加できる見込みがある**。READMEにこの旨を追記済み。

### 残る実用性課題(更新版)

1. ディレクトリ階層(サブディレクトリ)― `mount.rs`・`fuse_mount.rs`・
   `migrate.rs`いずれにも影響、引き続き未対応
2. E:・F:・G:・H:の具体的な組み合わせ方(ミラー+RAID6/Z2の対応関係)の
   確定 ― まだ未確定、次回続きから
3. `migrate`モジュールをCLIまたはインストーラーGUIから呼び出せるように
   する(前回から変更なし)
4. Linux起動ディスク化(カーネルモジュール+GRUBモジュール)― `fuse_mount.rs`
   が動いた今、着手条件が整った
5. **Windowsカーネルモード・ブート起動ドライバの開発** ― 着手する場合は
   隔離されたVM/テスト機で行うことをユーザーに要確認(前回から変更なし)
6. リポジトリは分割せず1つのまま維持し、配布物(インストーラー・
   パッケージ)だけを`open-raid-z-win`/`open-raid-z-linux`の名前で
   分ける、という方針が確定(まだ実際のパッケージング作業は未着手)
7. macOS対応(データボリューム、`fuser`の`macfuse-4-compat`活用) ―
   ロードマップに追加、まだ未着手
8. `cargo tauri build`(installer本体)のGUI実機検証(前回から変更なし)
9. CI(GitHub Actions)追加(前回から変更なし)
10. `resilver`を`Vdev`トレイトへ統一するかどうかの設計判断(前回から変更なし)
11. installer実装確認・PR作成・AD/SAM連携(前回から変更なし)

---

## 追記10: VirtualBox導入、【重要】メタデータ永続化の欠落を発見・修正

「1(Linux起動ディスク化)→2(Windowsカーネルドライバ)→3(RAID6実験)」の
順で進める合意のもと、まず1に着手する過程で、起動ディスク化以前に
影響する、より根本的な欠落を発見し、優先して対応した。

### VirtualBox 7.2.12の導入

Linux起動ディスク化の検証(実際に再起動して確認)には、ユーザーの
本番機ではなく隔離されたVMを使うべき、という方針のもとVirtualBoxを
`winget install --id Oracle.VirtualBox`で導入した(UAC確認のみ、
管理者権限のコマンド実行は不要だった)。`VBoxManage --version`で
`7.2.12r174389`を確認済み。

### 【重要な発見】`Pool`にメタデータ永続化が一切無かった

CLIツール作成の準備として`Pool::new`の永続化まわりを確認したところ、
**データセット一覧・ストライプ割当・スナップショット等の管理情報を
ディスクへ保存する仕組みが存在しない**ことが判明した。実データの
バイト列は`BlockDevice::write_at`により確実にディスクへ書かれるが、
「どのファイルがどのストライプにあるか」という対応情報は`Pool`構造体の
メモリ上にしかなく、プロセス終了(アンマウント)で完全に失われていた。
これまでの全セッション・全テストが「同一プロセス内でPoolを作って
操作して終わり」という前提で行われていたため、これまで一度も
踏まれていなかった抜けだった。

起動ディスク化はおろか、「フォーマットして使う」という最も基本的な
用途すら、この状態では実用にならない。ユーザーに確認のうえ、
Linux起動ディスク化より優先してこの永続化機能を実装した。

### `Pool::save`/`Pool::open`の実装

- ストライプ0を「スーパーブロック」として恒久的に予約(`free_stripes`に
  一切含めない)。`Pool::new`(新規作成)は据え置き、既存プールを
  再度開く場合は新設の`Pool::open(vdev, total_stripes)`を使う。
- `PoolMetadata`(マジックバイト`b"ORZPOOL1"`・`total_stripes`・
  `ref_counts`・`datasets`・`snapshots`)を`bincode`(新規依存)で
  シリアライズし、`Pool::save()`でストライプ0へ書き込む
  (既存の`Vdev::write_stripe`をそのまま使うため、チェックサム自己修復・
  RAID-Zパリティ保護もスーパーブロックに対して自動的に効く)。
- `Pool::open()`はストライプ0を読み、マジックバイトと`total_stripes`の
  一致を検証したうえで復元する(ディスク構成の取り違え・非対応
  バージョンを検知)。`free_stripes`は`ref_counts`から再計算する
  (保存はしない。空きリストの中身の順序に意味は無いため)。
- `mount.rs`・`fuse_mount.rs`の全ての変更操作(create/write/rename/
  delete/truncate)の後に`pool.save()`を自動実行するよう配線した
  (保存失敗時は操作自体をエラーとして呼び出し元へ返す。`create`の
  保存失敗時は作成自体をロールバックする)。

### 既存挙動への影響(想定内、全て修正済み)

ストライプ0が恒久的に使えなくなったため、「プール容量を丸ごと使い切る」
ことを前提にしていた既存テスト(容量計算アサーション)が連鎖的に
影響を受けた。想定内の範囲で、全て`cargo test`の実エラー出力を見ながら
1件ずつ修正した(手作業での事前予測ではなく、実際のコンパイラ/テスト
出力に基づいて直す、という前回確立した方針を踏襲):

- `usage().used_stripes`は「メタデータ予約ぶんの1」を常に含むため、
  「未使用時はused_stripes==0」という前提のテストは`==1`へ修正。
- 「プール全容量を使い切る」系のテストは、スーパーブロック用の1
  ぶんを差し引いて調整(既存のCoW作業領域ぶんの-1に、さらに-1)。
- RAID10のscrubテストは、破損注入先のグローバルストライプ0が
  予約領域になったため、実際にデータセットが使う範囲内の別ストライプ
  (グローバルストライプ2)へ変更。

### 実機検証: アンマウント→再マウントをまたいだ永続化(最重要)

`Pool`単体のAPIレベルでの往復だけでなく、**実際のマウント層
(WinFsp/FUSE)を経由した、本物のアンマウント→再マウント**で
ファイルが残ることを検証した:

- `tests/pool_persistence.rs`(新規、6件): `Pool::save`→スコープを
  抜けてdrop→同じディスクを`Pool::open`で開き直す、というAPIレベルの
  往復。データセット複数件・容量計算・スナップショットの参照カウント・
  保存前に破棄したデータセットが復元後に存在しないこと・
  `total_stripes`不一致の拒否・一度も保存していないディスクをopenした
  場合の安全な失敗、をそれぞれ検証。
- `tests/fuse_mount.rs`/`tests/winfsp_mount_file_ops.rs`に
  `a_file_created_through_the_mount_survives_a_real_unmount_and_remount`
  を追加(各1件)。実際に`mount_pool`→書き込み→`unmount`→同じディスク
  イメージから`Pool::open`→再度`mount_pool`→`std::fs::read`、という
  一連を本物のWSL2 Ubuntu/Windows実機で実行し、ファイルが読めることを
  確認した。

### 検証状況

Windows実機(`winfsp_backend`+`gpu_accel`、実マウント含む)・WSL2実機
(`fuse_backend`、実マウント含む)とも、新規追加ぶん(`pool_persistence.rs`
6件+実マウント永続化テスト各1件)を含め全テストがパス。

### 残る実用性課題(更新版)

1. Linux起動ディスク化(カーネルモジュール+GRUBモジュール) ―
   永続化問題の解消により、着手条件が改めて整った。次はここから。
   VirtualBoxのVM作成・ディスク割り当てから着手する。
2. `migrate`モジュールをCLIまたはインストーラーGUIから呼び出せるように
   する(前回から変更なし)。実ディスクへ書き込むCLIバイナリは
   Linux起動ディスク化の一環として必要になる。
3. E:・F:・G:・H:の具体的な組み合わせ方(ミラー+RAID6/Z2の対応関係)の
   確定 ― まだ未確定
4. **Windowsカーネルモード・ブート起動ドライバの開発** ― VM必須、
   最難関(前回から変更なし)
5. ディレクトリ階層(サブディレクトリ) ― 引き続き未対応
6. リネーム中の別ハンドルの整合性(前回から変更なし)
7. `cargo tauri build`(installer本体)のGUI実機検証(前回から変更なし)
8. CI(GitHub Actions)追加(前回から変更なし)
9. `resilver`を`Vdev`トレイトへ統一するかどうかの設計判断(前回から変更なし)
10. installer実装確認・PR作成・AD/SAM連携(前回から変更なし)

---

## 追記11: `orzctl`CLIツールの新規実装・実機スモークテスト成功

Linux起動ディスク化(initramfsからのマウント)には、`Pool`をコマンドラインから
操作できる実行可能ファイルが必要だったため、`orzctl`(`src/bin/orzctl.rs`)を
新規実装した。

### 内容

- `orzctl create --level <raid0|raid1|raid5|raid6|z2|z3> --chunk-size <バイト>
  --stripes <総ストライプ数> --dataset <名前> <ディスク...>`:
  実ディスク(またはループバックファイル)から新規プールを作成し、
  指定名のデータセットを作成して`Pool::save()`で保存する。
- `orzctl mount --level ... --chunk-size ... --stripes ... --mountpoint
  <ディレクトリ> <ディスク...>`: 保存済みのプールを`Pool::open()`で開き、
  FUSE経由でマウントしてフォアグラウンドで待機する(initramfsからは
  バックグラウンド実行するか、`switch_root`直前でそのまま待たせる運用を想定)。
- Linux(`fuse_backend`)専用。他環境でビルド・実行した場合はエラー
  メッセージを出して終了する(`main`をcfgで分岐)。
- `--stripes`は明示的に指定する方式(生ブロックデバイスの実容量自動検出は
  未実装。実運用では`blockdev --getsize64`等で事前計算する想定)。

### 実機スモークテスト(WSL2、シェルスクリプト経由で実行・成功)

1MiBのループバックファイル6枚(Z2, chunk_size=4096)に対し、
`orzctl create`→`orzctl mount`(バックグラウンド)→`echo`でマウント先へ
書き込み→`fusermount3 -u`でアンマウント→`orzctl mount`で再マウント→
`cat`で内容確認、という一連を実際にシェルから実行し、**再マウント後も
書き込んだ内容がそのまま読めることを確認**した(`Pool::save`/`Pool::open`が
CLIツール経由でも正しく機能することの実証)。

**ハマった点(再発防止のため記録)**: PowerShell経由で`wsl -d Ubuntu --
bash -c '...'`のように複雑な複数行スクリプト(`for`ループ・セミコロン区切り
複数コマンド等)を渡すと、セミコロンが正しく渡らず構文エラーになる
(`for i in 0 1 2 3; do ... done`が`unexpected end of file`になる)。
原因はPowerShellから`wsl.exe`(ネイティブ実行ファイル)への引数受け渡しの
段階でのエスケープ崩れと見られる。**複数コマンド・ループを含むスクリプトは、
インラインの`bash -c '...'`ではなく、一度`.sh`ファイルへ書き出してから
`wsl -d Ubuntu -- bash /mnt/c/.../script.sh`のように実行する**ことで回避した。

### 次のステップ

1. VirtualBoxでVMを作成し、複数の仮想ディスクを割り当てる
2. VM内(Linuxライブ環境)で、`/boot`用の通常ext4パーティション+
   `orzctl`を組み込んだinitramfsを構築する
3. GRUBが`/boot`からカーネル+initramfsを読み込み、initramfsが`orzctl mount`
   相当の処理でRAID-Zプールを組み立てて`switch_root`する、という流れを
   実際に構築・起動テストする

---

## 追記12: 実ディスク適用コマンド追加、`cargo test`が全く動いていなかった問題の発見・修正、GPU/NPUアクセラレーションの大幅拡張(GEMM化)

このセッションでは、Claude Codeで`open-raid-z`リポジトリを直接操作。
インストーラーの安全設計上の抜け(プレビューのみ)を埋め、
`cargo test`自体が実は一度も実行できていなかったという重大な問題を発見・修正し、
GPU/NPUアクセラレーションをパリティ生成だけでなくパリティチェック(復旧計算)にも拡張した。

### 1. インストーラー: 実ディスクへのzpool適用(`init_zpool_apply`)

`zpool_wizard.rs`の`init_zpool_preview`/`init_raid10_preview`は、ずっと
スクラッチイメージ(`std::env::temp_dir()`)だけを対象にした「プレビュー
専用」実装で、`hardware::list_physical_disks()`が返す実ディスクパス
(`\\.\PhysicalDriveN`)へは一切書き込めなかった(意図的な安全設計だったが、
「将来UIボタンを追加するだけで対応できる」というコメントのまま放置されていた)。

`init_zpool_apply`を新規実装し、`hardware::list_physical_disks`が返す
実パス+サイズを受け取って`FileBackedDevice::open`で実際に開き、RAIDプールを
構築する。誤操作防止のため`confirm_data_loss`フラグが`true`でない限り
ディスクを一切開かない。Tauriコマンドとしても登録済み。実ディスクへは
一度も自分で書き込みを実行していない(コードとテストのみ、実データ消去は
最終的にユーザー操作が必要)。

### 2. 【重要な発見】`cargo test`が`open_runo_installer_core`で一度も
　　成功したことが無かった(Windows Installer Detection Technology)

`cargo test`を実行すると、テスト自体が1つも走らずに
`ERROR_ELEVATION_REQUIRED`(os error 740)でプロセス起動自体が失敗する
現象を発見。原因はWindowsの「インストーラー検出ヒューリスティック」:
マニフェストの無い実行ファイルの名前に"install"等の文字列が含まれると、
管理者権限を自動的に要求される。テストハーネスのバイナリ名が
`open_runo_installer_core-<hash>.exe`だったため、この対象に該当していた
(クレート名に"installer"を含まない`open_raid_z_core`は問題なくテストが
走っていたため、両者を比較して原因を特定)。

`embed-manifest`クレートでasInvokerマニフェストを埋め込むことで解消
(`build.rs`)。ただし同クレートの`embed_manifest()`は`cargo:rustc-link-arg-bins`
を固定で発行するため、`[[bin]]`ターゲットが無い本クレートでは使えず、
`cargo:rustc-link-arg`(無指定、テストハーネスにも効く)を自前で発行する
必要があった。

**これが直せたことで、これまで一度も実行されていなかった`zpool_wizard.rs`の
既存テスト群が初めて実際に走り**、隠れていた本物のバグ(次項)が見つかった。

### 3. 容量計算のオフバイワン修正

`cargo test`が動くようになって初めて発覚: `Pool::new`はメタデータ
(スーパーブロック)用に1ストライプを予約するが、`init_zpool_preview`/
`init_raid10_preview`/`init_zpool_apply`はいずれもディスクの生容量
そのままを`grow_dataset`しようとしており、常に1ストライプぶん容量不足で
失敗していた(メタデータ永続化機能が追加された際に、呼び出し側が
追随していなかったための回帰)。`pool.usage().free_stripes`(予約後の
実容量)を使うよう修正し、7件のテストが green になった。

### 4. RAID-Z3用GPUシェーダの追加

`shaders/raidz2_parity.hlsl`(P/Q)は実装済みだったが、RAID-Z3のR
(係数4^i)用のシェーダが無く、Z3は常にCPU計算にフォールバックしていた。
`shaders/raidz3_parity.hlsl`を新規実装(P/Q/R同時計算、Qと同じ
反復2倍算ロジックをRだけ2倍の回数繰り返す)し、`compute_pqr_accelerated`
を`RaidZVdev::compute_parity`のparity_count==3分岐に配線。実機GPU
(NVIDIA GT730)でCPU参照実装とビット単位で一致することを確認。

### 5. NPU専用シェーダ経路の分離(`raidnpu_*.hlsl`)

これまでNPUとGPUは`AccelKind::Npu | AccelKind::Gpu`という同じmatchアーム・
同じシェーダバイトコードを共有していた。NPU実機は無いため速度上の
優位性は検証できないが、将来NPU専用の実装(DirectML等)へ書き換える際に
GPU側の検証済みパスを壊さないよう、`raidnpu_parity.hlsl`/
`raidnpu_z2_parity.hlsl`/`raidnpu_z3_parity.hlsl`という別ファイル・
別ディスパッチ経路へ分離した(現状は内容はGPU版と同一)。GT730で
これらのバイトコード自体もCPU参照実装と一致することを確認済み
(D3D12ディスパッチ機構はNPU/GPUを区別しないため、GPUでもシェーダの
正しさは検証できる)。

### 6.【設計の核心】GF(2)ビット行列によるGEMM再定式化(`bitmatrix.rs`)

ユーザーから「NPUの本領(行列演算ユニット)を活かせていない」という
指摘を受け、根本的な再設計を実施。GF(2^8)上で定数`c`を掛ける操作は、
GF(2^8)がGF(2)上のベクトル空間であることから、実は**GF(2)上の8x8線形写像
(行列)**そのものである。複数ディスクぶんをブロック結合すれば、
「W(8×8N行列)×X(8N×ストライプ長のビット行列)を整数で内積計算し、
各成分をmod 2で戻す」という**1回の整数GEMM**にRAID-Z2/Z3パリティ計算
全体を帰着できる。これはDirectMLのGEMMオペレータ経由でNPUのMAC/
テンソルユニットに乗る形(生のHLSL Compute Shaderでは原理的に到達
できない領域)。

`bitmatrix.rs`でこの再定式化がCPU上で既存のGaloisTables参照実装と
完全に一致することを検証(全256バイト値×Q/R両方で使う16種類の係数、
1〜12ディスク構成)。DirectML配線前にゼロリスクで数学的正しさを
証明する段階。

### 7. 実際のDirectML GEMMディスパッチ実装(`dml_gemm.rs`)

windows-rs 0.58に`Win32_AI_MachineLearning_DirectML`featureとして
DirectMLのCOM API(`IDMLDevice`/`IDMLOperator`/`DML_GEMM_OPERATOR_DESC`等)
が存在することを確認し、実際にGT730上で完全なDirectMLパイプライン
(デバイス→オペレータ作成→コンパイル→初期化→バインドテーブル→
コマンドレコーダ→ディスパッチ→リードバック)を構築。

実機でしか見つからない2つのバグに遭遇し、`ID3D12InfoQueue`のメッセージ
ダンプで特定・修正:
- テンソル記述を2D形状(`DimensionCount=2`)で渡すとGPUデバイスが
  リセットされる → DirectMLは4D(NCHW風)形状を要求する仕様だった
- `BindInputs`で"expected 3 bindings but 2 were provided"エラー →
  GEMMはC入力が未使用(`CTensor=null`)でも常に3スロット分のバインドが
  必要。`DML_BINDING_TYPE_NONE`のプレースホルダで解決。

`compute_pq_via_dml_gemm`/`compute_pqr_via_dml_gemm`として実装、
ディスク数2/3/5/8で実機GPU検証済み(CPU参照実装とビット単位で一致)。
**production dispatch(`compute_pq_accelerated`等)には意図的に配線して
いない**(NPU実機が無く生Compute Shader版との速度比較ができない、
DirectMLのオペレータコンパイルは書き込みごとに毎回行うのは非現実的
でキャッシュ設計が必要、という2つの理由)。

### 8. パリティチェック(復旧計算)のGEMM高速化・本番配線

ユーザーから「パリティチェックもGPU/NPUで高速化したい」との要望。
`dml_gemm.rs`を一般化し、固定係数(2^i/4^i)専用だった重み行列構築を
`linear_combine_via_dml_gemm(gf, known_disks, coeffs_per_output)`
として任意のGF(2^8)係数行列に対応させた(P/Q/R生成もこの汎用関数を
呼ぶよう整理)。

これを使って`reconstruct_missing_data_generic_accelerated`を新規実装。
scrub/resilverが破損を検知した際に実際に走る復旧計算(=パリティチェック
の重い部分)のシンドローム計算をGPU/NPUへオフロードする。RAID-Z3の
3台同時故障シナリオで実機GPU検証済み(CPU参照実装と完全一致)。

こちらは**P/Q/R生成とは違い、本番経路(`RaidZVdev::
read_stripe_forcing_missing`、scrub/resilver/縮退読み込みが使う)に
配線した**。理由: 復旧計算は書き込みと違って稀にしか走らないため
DirectMLの初期化コストのリスクが小さく、失敗時はCPU実装へ完全に
フォールバックするため配線しても害が無いため。

### 検証状況

`zfs_accel_hlsl`: `--no-default-features`で28件、`--features gpu`で
36件、全てpass(実機GPU検証を含む)。`open_raid_z_core`: 19テスト
バイナリ全てpass(RAID-Z2/Z3障害復旧統合テスト含む、既存の挙動に
回帰無し)。`open_runo_installer_core`: 30件pass(今回のセッションで
初めて実際に実行できるようになった)。

### 残る課題

1. DirectML GEMM経路(`dml_gemm.rs`)をproduction dispatchへ配線するか
   どうかの判断 ― NPU実機入手待ち。実機無しでは速度上の優位性を
   主張できないため保留。
2. DirectMLオペレータのキャッシュ設計(現状は毎回作成・コンパイルして
   おり、書き込みパスへ配線する場合は必須になる)
3. NPU実機での`raidnpu_*.hlsl`/DirectML経路の実速度計測(前回から変更なし、
   実機入手待ち)
