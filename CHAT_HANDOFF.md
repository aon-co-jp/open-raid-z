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
