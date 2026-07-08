# OpenRuno ZFS プロジェクト — チャット記録

このファイルはClaudeとの会話の要約記録です。会話の完全な逐語ログは
Claude.ai側のシステム上に保存されており、このファイル単体で
「新しいAIサービスに読み込ませてそのまま再現する」ことはできません。
別のAIに引き継ぐ場合は、このMarkdownを要約コンテキストとして貼り付けてください。

---

## プロジェクト概要

Windows版NTFS互換・全RAID対応・NPU/GPUハードウェアアクセラレータ対応の
ZFSファイルシステム導入システムを開発する、という依頼から開始。

### 技術的に指摘した前提の誤り

- DirectXはグラフィックス/計算APIであり、ファイルシステムを
  「DirectXのプラグイン」として実装することはできない
- ZFSとNTFSはオンディスクフォーマットが根本的に異なり、
  「完全互換」はバイナリレベルでは不可能。実現できるのは
  ACL/タイムスタンプ等のセマンティクスのエミュレーション
- NPUは本来ニューラルネット推論向けハードウェアだが、
  DirectMLを使えばNPU/GPUを同一インターフェースから
  ディスパッチ可能

### 決定したアーキテクチャ(3コンポーネント構成)

| # | コンポーネント | 役割 | 技術スタック |
|---|---|---|---|
| ① | open_zfs_winfsp_bridge | ZFS on Windows のI/OをWinFsp経由でフックし、NTFS ACLセマンティクスをエミュレーション | Rust + WinFsp (winfsp-rs) + windows-rs |
| ② | zfs_accel_hlsl | チェックサム/RAID-Zパリティ/圧縮をNPU/GPUへオフロード | Rust + DirectX 12 Compute + DirectML(HLSL) |
| ③ | open_runo_installer | ハードウェア検出・ドライバ登録・zpool初期化のGUIインストーラー | Tauri |

## 実装済み内容(スキャフォールディング/PoC段階)

### ① open_zfs_winfsp_bridge
- `Cargo.toml`: winfsp, windows-rs, thiserror, tracing, serde, bitflags 依存関係定義
- `error.rs`: BridgeError型(PoolNotFound, MountFailed, AclTranslationFailed等)
- `acl_emulation.rs`: ZFS(NFSv4 ACL) ⇔ NTFS ACLの中間表現と変換関数
  (`zfs_ace_to_ntfs`実装済み、`ntfs_ace_to_zfs`は未実装)
- `fs_ops.rs`: `ZfsBackend`トレイト(open_dataset/read/write/get_acl/set_acl)、
  WinFsp GetSecurity相当のハンドラ骨格
- **[2026-07-08] 実機Windows環境で`cargo build`/`cargo test`が成功することを確認済み。**
  `winfsp`クレートは0.11→0.13(+winfsp-2.1)へ更新。旧0.11系はインストール済み
  WinFsp 2025(2.1.25156)のヘッダ生成と噛み合わず`_FSP_FILE_SYSTEM`が
  不透明型(`UserContext`フィールド欠落)になりビルド不能だったため。
  ビルドにはLLVM(libclang、bindgen用)とWinFsp SDK/ランタイムのインストールが必須
  (winget: `LLVM.LLVM`, `WinFsp.WinFsp`)。

### ② zfs_accel_hlsl
- `Cargo.toml`: windows-rs(Direct3D12, DirectML等の機能フラグ)依存関係定義
- `device.rs`: **[2026-07-08] DXGIアダプタ列挙・D3D12デバイス作成検証を実装。**
  `CreateDXGIFactory1`→`EnumAdapters1`でアダプタを列挙し、ソフトウェアアダプタを除外、
  `D3D12CreateDevice`で実際にデバイス作成可能かを検証。アダプタ名に
  "AI Boost"/"XDNA"/"Hexagon"/"NPU"等が含まれればNPU、それ以外はGPUとして分類。
  実機検証では `NVIDIA GeForce GT 730` をGPUとして正しく検出(このマシンにNPUはない)。
  DirectMLデバイス生成(`DMLCreateDevice`)自体はディスパッチ時に利用側で行う設計とし、
  本関数の責務はアダプタ選定までに限定。
- `raidz_parity.rs`: RAID-Z1(単一パリティ)のCPU参照実装(XOR)を実装し、
  ユニットテストで正しさを検証済み。GPU/NPUディスパッチは
  現状CPUへフォールバックするスタブ(実HLSLディスパッチは未実装のまま)
- `shaders/raidz_parity.hlsl`: RAID-Z1パリティ計算用Compute Shader(XOR)

### ③ open_runo_installer
- **[2026-07-08] `npm create tauri-app` (Tauri v2, vanilla-ts, npm, identifier
  `com.openruno.installer`) で雛形を生成。`npm install`・`cargo build`(src-tauri)
  ともに実機で成功を確認済み。** UI/バックエンドロジックは未実装(雛形のまま)。

### 共通の注意点
- `E:\open-runo\Cargo.toml` に無関係な別プロジェクト(aruaru-*)のCargoワークスペースが
  存在し、配下の全Rustクレートが誤ってそこに巻き込まれる。各`Cargo.toml`に
  空の`[workspace]`テーブルを追加して切り離し済み(①②③すべて対応済み)。
- 作業ディレクトリを `openruno-zfs-source` → `open_runo_zfs_source` にリネーム済み。

## ユーザーのプロジェクト方針(userPreferencesより)

- PureRust + Poem + 独自AI予測判断 を基本方針とする「OpenRuno」構想
- Tauriを採用してデスクトップアプリの高速化・サーバー負荷軽減を志向
- 関連プロジェクト群: open-aruaru(iLumi)、open-e-gov、OpenDirectX、
  OpenCuda、OpenLLM(aruaru-llm)、OpenCosmo、OpenRedmine、OpenWordPress
- 参考: WunderGraph Cosmo(Go製、OpenRunoではPure Rust化する方針)
- 参照ドキュメント: Rust Book/Documentation、Poem docs、Tauri v2 docs

## 次のステップ(未着手)

1. ①のCargo.tomlのwinfsp依存が実在するクレート名/バージョンか要確認
   (実機Windows環境でのビルド検証が必須)
2. ②のDXGIアダプタ列挙・DirectMLデバイス生成の実装(Windows実機必須)
3. ③open_runo_installerのTauriプロジェクト雛形作成
4. NTFS ACL⇔ZFS ACIDのUID/GIDマッピングテーブル設計
5. RAID-Z2/Z3(Reed-Solomon)パリティ計算の実装

---

*このMarkdownは会話の技術的要点をまとめた引き継ぎ資料です。
実際の対話の言い回しやニュアンスは含まれていません。*
