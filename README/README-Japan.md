# open-raid-z

Windows上でNTFS/exFATとほぼ互換性を保ちながら、ZFS風の機能(チェックサム自己修復・ストレージプール・コピーオンライト・スナップショット/クローン)とRAID0/1/5/6/10/Z2/Z3を提供する、実験的なファイルシステムプロジェクトです。

言語: **日本語** | [UK English](README-UK-English.md) | [US English](README-US-English.md) | [Italiano](README-Italy.md) | [Français](README-France.md) | [Deutsch](README-Germany.md) | [Русский](README-Russia.md) | [Українська](README-Ukraine.md) | [العربية](README-Arabic.md) | [فارسی](<README-Iran(Persian).md>)

## 命名規則

ディレクトリ名・crate名・npmパッケージ名・Cargo feature名・HTML/CSSの
id/class名など、このプロジェクト自身が定義する識別子は、ハイフン(`-`)
ではなく**アンダースコア(`_`)区切りに統一**している
(例: `open_zfs_winfsp_bridge`、`zfs_accel_hlsl`、`open_runo_installer`、
`open_runo_installer_core`、Cargo featureの`winfsp_backend`/`gpu_accel`)。
以前は`openzfs-winfsp-bridge`のようにハイフン区切りだった箇所を、
このプロジェクト内で一貫性を持たせるために変更した。

ただし以下は対象外(このプロジェクト自身の命名規則ではなく、外部の
仕様・エコシステム側の規約に従う必要があるため):

- リポジトリ名自体(`open-raid-z`、GitHub上の実際のリポジトリ名のため変更不可)
- HTML5の`data-*`カスタム属性(`data-i18n`。ハイフンが仕様上必須)
- 外部npmパッケージ名(`@tauri-apps/api`等、公開されている実際のパッケージ名)
- CSSプロパティ名(`font-family`等、CSS言語仕様そのもの)
- Reed-Solomon、Copy-on-Writeなど、英語の複合語として本来ハイフンを含む用語

## 構成

| コンポーネント | 役割 |
|---|---|
| `open_zfs_winfsp_bridge` | RAID-Z/RAID0-10 vdev、ストレージプール、NTFS ACL/exFAT属性互換層、WinFsp実マウント |
| `zfs_accel_hlsl` | GPU/NPUハードウェアアクセラレータ(DirectX 12 Compute + DirectML)によるパリティ計算オフロード |
| `open_runo_installer_core` | ディスク検出・Copilot風構成アドバイザー・zpool初期化プレビューのOS非依存ロジック(Tauri非依存、Linux/macOSでも`cargo test`可能) |
| `open_runo_installer` | Tauri製インストーラー本体(`open_runo_installer_core`を呼び出す薄いUI層)。ハードウェア検出・zpool初期化ウィザード・Copilot風構成アドバイザーのUI |

## 主な機能

- **RAID全系列に対応**: RAID0 / RAID1(ミラー) / RAID5 / RAID6 / RAID10(ストライプ+ミラー) / RAID-Z2 / RAID-Z3
- **ディスクのパーティション分割・使い回し**: 1台のディスクを分割し、片方をミラー、もう片方を別のRAID6/Z2配列のメンバーにする、といった構成も可能
- **チェックサム自己修復・コピーオンライト・スナップショット/クローン**: ZFSと同じ考え方をエミュレーション。`Pool::scrub`でプール全体のサイレント破損を一括検知・修復可能(RAID-Z系・RAID10のどちらでも共通のAPIで実行可能)
- **NTFS互換**: ACL(NFSv4⇔NTFS)・UID/GID⇔SIDマッピング(ローカルSAM/ADドメインのRIDベース決定論的マッピング)
- **exFAT互換**: ファイル属性・タイムスタンプの相互変換、4GB超ファイル/大容量ボリューム対応
- **GPU/NPUハードウェアアクセラレーション**: DirectX 12 Compute + DirectMLでRAID-Z1/Z2のパリティ計算を実際にオフロード(ハードウェアが無い場合はCPUへ自動フォールバック)
- **Copilot風構成アドバイザー**: ディスク構成・アクセラレータ・CPUコア数から推奨RAIDレベルを提案(ヒューリスティック版。ローカルLLM検知の骨組みも搭載)。ロジックは`open_runo_installer_core`としてTauriから独立しており、Linux/macOS上でも`cargo test`で検証可能
- **WinFsp実マウント(プロトタイプ)**: 実際にWindows上のドライブレターとしてマウント可能。プール内の全データセットがそれぞれ1ファイルとして見え、バイト単位の任意オフセット読み書きに対応(現状はフラットな名前空間のみで、ディレクトリ階層・create/delete/renameは未対応)
- **多言語対応**: インストーラーは日本語をデフォルトに、UI言語切り替えに対応(インストール後も変更可能)

## 現状の制約(プロトタイプ段階)

- WinFspマウントはフラットな名前空間(ルート直下にプール内の全データセットがそれぞれ1ファイルとして並ぶ)のみ対応。ディレクトリ階層・ファイル単位でのcreate/delete/renameは未対応。
- ファイルの読み書きは`Pool::read_unaligned`/`Pool::write_unaligned`(read-modify-write層)経由でバイト単位の任意オフセット・任意長に対応済み。ただしデータセットの割当容量(`grow_dataset`で確保済みの範囲)を超えるリクエストは引き続きエラーになる(暗黙の自動拡張は行わない)。
- `Pool`はまだ`RaidZVdev`/`Raid10Vdev`両対応だが、RAID10はデータセットAPIとの統合が浅い部分がある。
- WinFsp実マウント関連のコード(`mount.rs`)は`winfsp`クレートがedition2024を要求するため、Rust 1.85未満のツールチェインではビルドできない(後述のビルド・テスト参照)。
- `mount.rs`・`zfs_accel_hlsl`のGPU実装(`gpu` feature)は`windows`クレートに依存するが、同クレートはコンパイルターゲットが実際にWindowsでない限り中身が空になる。そのためこれらのコードはWindows実機(またはWindowsターゲットへのクロスコンパイル)でのみビルド・テストでき、Linux/macOS上では`--no-default-features`でこれらを無効化した場合のみビルドできる。

## ビルド・テスト

```powershell
cd open_runo_zfs_source/open_zfs_winfsp_bridge
cargo test --no-default-features        # WinFsp/GPUアクセラレータ無し(CPUロジックのみ、dxcもWinFsp SDKも不要)
cargo test                              # 既定(WinFsp実マウント+GPU/NPUアクセラレータを含む、要WinFsp+dxc)
```

`--no-default-features`は`winfsp_backend`・`gpu_accel`の両featureを無効化し、RAID0/1/5/6/10/Z2/Z3・チェックサム自己修復・CoW・スナップショット/クローン・resilverなどのコアロジックをOS非依存(Linux/macOSでも可)で検証できる。WinFsp・DirectX Shader Compiler(dxc)・GPU/NPUハードウェアは一切不要。

既定feature(`winfsp_backend` + `gpu_accel`)でのビルドには以下が必要:

- WinFsp本体(https://winfsp.dev/)がシステムにインストールされていること(SDKヘッダはビルド時に自動でベンダリングされたものを使用するため、開発者向けコンポーネントの追加インストールは不要)。
- `dxc`(DirectX Shader Compiler。Windows SDKまたはVulkan SDKに同梱)がPATH上にあること(RAID-Z/Z2パリティ計算用HLSLシェーダのビルド時コンパイルに使用)。
- **Rust 1.85以降**(`winfsp`クレートが要求する`edition2024`が安定化されたバージョン。これより古いツールチェインでは`Cargo.toml`のマニフェスト解析自体が失敗する)。

WinFsp・dxcのどちらか一方だけを個別に無効化することも可能(`--no-default-features --features gpu_accel`でWinFsp無し・GPUのみ有効、など)。

### インストーラー(`open_runo_installer` / `open_runo_installer_core`)

```powershell
# ロジック層(Tauri非依存、Linux/macOSでも動作)
cd open_runo_zfs_source/open_runo_installer_core
cargo test                    # CPUフォールバックのみ(既定)
cargo test --features gpu     # 実GPU/NPUディスパッチを含む(要Windows実機+dxc)

# フロントエンド(TypeScript、OS非依存)
cd open_runo_zfs_source/open_runo_installer
npm install
npx tsc --noEmit               # 型チェックのみ
npx vite build                 # 実際にビルド

# Tauriアプリ本体(要Windows実機、または十分に新しいRust+Linuxデスクトップ依存関係)
cd open_runo_zfs_source/open_runo_installer/src-tauri
cargo tauri dev / cargo tauri build
```

`open_runo_installer_core`(ディスク検出・Copilot風構成アドバイザー・zpool
初期化プレビュー)はTauriに依存しない独立クレートのため、Tauri自体の
ビルドに必要な諸依存(WebView・GTK等、および十分に新しいRustツールチェイン)
が無い環境でも、ロジックの正しさをそのまま検証できる。実際のディスク列挙
(`\\.\PhysicalDriveN`)のみWindows専用APIを使うため`#[cfg(windows)]`で
分離しており、それ以外(構成助言・zpoolプレビュー計算)はOS非依存で
26件のテストが全て通ることを確認済み。

## ライセンス

MPL-2.0
