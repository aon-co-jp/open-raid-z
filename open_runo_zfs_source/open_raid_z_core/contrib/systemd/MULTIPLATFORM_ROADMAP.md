# マルチOS対応 + 既存フォーマット相互運用 ロードマップ

このドキュメントは、以下2つの目標をどのように段階的に実現するかを記録する。

1. **open-raid-z自体のフォーマットを、Windows/Mac/Linux/Android/iOS/iPadで
   読み書きできるようにする**(HDD/SSD/NVMe/USBメモリ/microSD/CFカードを
   1〜4台以上でRAID構成、NPU/GPUハードウェアアクセラレーション対応)。
2. **open-raid-zをインストールした環境から、他OSの既存フォーマット
   (NTFS/exFAT/FAT32/ext4/APFS等)も読み書きできるようにする**
   (同じくNPU/GPUアクセラレーション対応)。

ユーザーとの合意事項(2026-07-09):
- 優先着手範囲: Mac対応、Android対応、既存フォーマット読み込み対応、
  iOS/iPad対応(可能な範囲で)の**全て**を対象とする。
- GPU/NPUアクセラレーションは、Windows以外のプラットフォームでは
  **各OS標準のネイティブAPIに置き換える**(DirectX/DirectMLはWindows専用
  APIのため他OSでは動作しない)。

## 目標①: プラットフォームごとの実現可能性と方針

| OS | ファイルシステム実装方式 | 実現可能性 | GPU/NPU API |
|---|---|---|---|
| Windows | WinFsp(既存実装済み) | 済 | DirectX 12 Compute + DirectML(既存実装済み) |
| Linux | FUSE(`fuser`、既存実装済み) | 済 | Vulkan Compute / CUDA(NVIDIA)。要検討: ROCm(AMD)、oneAPI(Intel) |
| Mac | macFUSE または FUSE-T(ユーザーモード) | **技術的に実現可能。ただし実機(Apple製ハードウェア)でのビルド・署名・公証・実動作検証が必須**(Appleのソフトウェア使用許諾契約上、macOSは原則Apple製ハードウェア上でのみ実行が許される。VirtualBox等での仮想化検証は不可) | Metal Performance Shaders / Metal Performance Shaders Graph |
| Android | FUSE経由の専用アプリ(Storage Access Framework、root化不要) | 実現可能。Android Studio AVD(エミュレータ)でWindows上でも開発・検証可能 | NNAPI (Android Neural Networks API) / Vulkan Compute |
| iOS/iPad | **ブロックデバイスへの直接RAID構成は不可**(サードパーティのカーネル/ファイルシステムドライバをApple非公式に許可していないため)。File Provider Extension経由で、既にマウント済みのプールの内容をファイル一覧としてアクセスさせる形に限定 | 部分的(閲覧・ファイル単位の読み書きのみ)。RAID構成そのものはiOS上では組めない | Core ML / Metal Performance Shaders(File Provider Extension内での重い処理は基本的に想定しない) |

### 着手順序(実装コスト・検証容易性で決定)

1. **Linux GPU/NPUネイティブAPI(Vulkan Compute)**: 既存のFUSE実装済み
   Linux環境で追加ハードウェアなしに検証できるため最優先。
2. **Android**: Android Studio AVDで検証可能(Windows機のみで開発〜検証まで完結)。
3. **Mac**: コードはこのセッションでも書けるが、**実機での動作確認は
   Apple製ハードウェア入手まで不可能**。設計・実装は先行させ、
   「実機未検証」と明記して進める。
4. **iOS/iPad**: Mac(Xcode)が前提のため、Macでの土台ができてから着手。

## 目標②: 既存フォーマットの読み書き対応

こちらは「open-raid-zプール」とは独立した、**既存ディスク/パーティション
上のファイルシステムを読み書きするブリッジ機能**として設計する。

### 対応方式の方針

自前でパーサーを一から書くのではなく、実績のあるRust実装(またはFFI経由の
既存ライブラリ)をラップし、`open_raid_z_core`が提供する共通トレイト越しに
アクセスできるようにする。

| フォーマット | 想定実装 | 難易度 | 優先度 |
|---|---|---|---|
| FAT32/FAT16 | `fatfs`クレート(純Rust、読み書き対応、実績あり) | 低 | **最優先(着手済み)** |
| exFAT | 純Rustのメンテ中クレートが少ない。当面は読み取り専用の自前実装(既存の`exfat_emulation.rs`の属性/タイムスタンプ変換ロジックを再利用) | 中 | 次点 |
| NTFS | `ntfs`クレート(純Rust、読み取り専用、メンテ中) | 中 | Windows以外での読み取りに有用 |
| ext4 | `ext4-view`等の純Rust読み取り専用クレート、または`e2fsprogs`(libext2fs) FFI | 中〜高 | Windows/Mac側での読み取りに有用 |
| APFS | 成熟した純Rust実装が無い。リバースエンジニアリングベースの`apfs-fuse`(C++, 読み取り専用)等をFFIラップするか、当面は非対応 | 高 | 最後回し |

### 共通設計

`orzctl`に新サブコマンド`foreign`系(例: `orzctl foreign ls`、
`orzctl foreign cat`)を追加し、対象パーティション/イメージファイルと
フォーマット種別を指定して読み書きできるようにする。将来的には
WinFsp/FUSE経由でマウントもできるようにするが、まずはCLIでの
読み書き(ls/cat/cp相当)から始め、実機・実イメージでの動作確認を優先する。

## 現状の実装状況

- FAT32読み取り(`orzctl foreign ls`/`cat`相当): **このセッションで着手**。
  `foreign_fs.rs`参照。
