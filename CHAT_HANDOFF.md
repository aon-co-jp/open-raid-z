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
