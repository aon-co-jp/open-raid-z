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

- FAT32/FAT16読み書き(`orzctl foreign ls`/`cat`/`put`/`mount`): 実装済み。
  `foreign_fs.rs`(`ForeignFatVolume`)・`foreign_fuse_mount.rs`参照。
- exFAT読み書き(リネーム・サブディレクトリ書き込みは上流`hadris-fat`の
  制約により未対応): 実装済み。`foreign_fs.rs`(`ForeignExfatVolume`)参照。
- **ext2/ext4読み取り(2026-07-20実装)**: 純Rustの`ext4-view`クレートを
  ラップした読み取り専用ブリッジ`ForeignExt4Volume`を実装
  (`orzctl foreign --format ext4 ls`/`cat`/`mount`。mountは
  `MountOption::RO`の読み取り専用マウント)。書き込みは、書き込み対応の
  成熟した純Rust ext4実装が2026-07時点で存在しないため未対応
  (各書き込みAPIは明示的にエラーを返す)。実`mkfs.ext4`(e2fsprogs
  1.47系)製イメージをフィクスチャとする統合テスト
  `tests/foreign_ext4.rs`(8テスト)で検証済み。
- NTFS読み取り(`ntfs`クレート想定)・APFS: 未着手(次の増分候補)。

## 追記: Mac対応の型レベル検証成功、Android対応の具体的な障害を特定(2026-07-10)

### コア(CPU専用ロジック)のクロスプラットフォーム性、実際に確認済み

`rustup target add aarch64-linux-android x86_64-apple-darwin`でターゲットを
追加し、実機無しで以下を確認した:

- `zfs_accel_hlsl`(`--no-default-features`、CPUフォールバックのみ)・
  `open_raid_z_core`(`--no-default-features`、WinFsp/FUSE/GPU全て無効)は、
  Android(`aarch64-linux-android`)ターゲットで**そのまま`cargo check`が
  通る**(`windows`クレートがWindows以外では中身が空になる、という既知の
  制約のおかげでコンパイルが通る点も含め、以前からの設計方針が正しく
  機能している)。

### Mac対応: `fuse_mount.rs`が型レベルで正しいことを確認

`Cargo.toml`に`[target.'cfg(target_os = "macos")'.dependencies]`セクションを
新設し、`fuser`クレートの`macfuse-4-compat` featureを有効化する構成にした。
`fuser`のbuild.rsはmacOSターゲットの場合、既定では実際にmacFUSEを
pkg-config経由で探しに行く(実機Mac+macFUSEが無いと失敗する)仕様だが、
同クレートには`macos-no-mount`という「マウント機能自体を提供しない
スタブ実装」featureが用意されており、これへ一時的に切り替えることで
**macFUSE未インストールのcrossビルド環境でも型チェックだけは通せる**
ことを発見した。

この方法で`cargo check --no-default-features --features fuse_backend
--target x86_64-apple-darwin`を実行し、**`fuse_mount.rs`(Linux版と
共有しているマウント実装コード)がmacOS向けにも型エラー無くコンパイル
できることを確認した**(検証後、本番用の`macfuse-4-compat`設定へ戻して
コミットしている。実際にマウントできるかどうかは実機Mac+macFUSE
インストール環境でしか検証できない、という制約は変わらず残る)。

### Android対応: 具体的な技術的障害を特定(重要な発見)

Android(Linuxカーネルベース)向けに同様の対応を試みたところ、
`fuser` 0.17クレート自体に起因する明確なブロッカーを発見した:

`fuser`のbuild.rsは、libfuseへリンクしない「pure-rust」実装
(`/dev/fuse`へ直接システムコールで話しかける、ネイティブライブラリ
不要の実装)を`target_os`が`linux`/`freebsd`/`dragonfly`/`openbsd`/
`netbsd`の場合にのみ許可しており、**`android`はこのリストに含まれて
いない**。そのため`android`向けにこの依存を有効化すると、
libfuse2/libfuse3をpkg-config経由で要求してしまうが、Android NDK環境には
そのようなライブラリが存在しないためビルドが失敗する
(`cargo check --target aarch64-linux-android`で実際に確認済み、
エラーメッセージ: `Failed to configure libfuse3 or libfuse2: pkg-config
has not been configured to support cross-compilation`)。

**技術的な考察**: AndroidもLinuxカーネルをそのまま使っており、
`/dev/fuse`のプロトコル自体はLinuxと同一のはずである。つまり
`fuser`のpure-rust実装は、原理的にはAndroidでもそのまま動作する
可能性が高いが、**上流クレートの`build.rs`が`target_os`の許可リストに
`android`を含めていないだけ**という状況だと考えられる。

**今後の対応候補**:
1. `fuser`へのアップストリームパッチ(pure-rust対象の`target_os`
   リストに`android`を追加する提案・PR)を検討する。
2. それまでの繋ぎとして、`fuser`をフォーク(`[patch.crates-io]`で
   差し替え)し、この1点だけ変更したバージョンを使う。
3. いずれにせよ、実際にAndroid端末上でマウントするには「rootedデバイス」
   または「Storage Access Framework経由でアプリコンテキストに
   `/dev/fuse`アクセス権を与える」という、ライブラリ側の対応とは別の
   OSレベルの権限問題も残っている(ロードマップの当初の記述通り)。

### 結論・優先順位への影響

- **Mac対応**は「設計・コードは型レベルで正しいと確認済み、実機での
  マウント動作確認のみが残課題」という、当初の想定通りの状態まで
  前進した。
- **Android対応**は、当初の想定(「Android Studio AVDで検証可能」)
  よりも根が深く、**ライブラリレベルの障害(`fuser`が非対応)を
  まず解消する必要がある**ことが判明した。次回はこの障害の解消
  (upstream提案またはフォーク)から着手するのが妥当。

## 追記2: Android対応のライブラリレベル障害を解消(フォーク・クロスコンパイル確認済み)(2026-07-10)

上記「今後の対応候補」の2(フォーク)を実施した。

- `open_runo_zfs_source/third_party/fuser-0.17.0-android-patch/`に、
  `fuser` 0.17.0のパッチ済みフォークを配置(パッケージ名は
  `fuser_android_patch`)。
  - `build.rs`のpure-rust許可OS一覧へ`android`を追加。
  - `src/`以下の`target_os = "linux"`ゲート(`mount()`呼び出し、
    `renameat2`フラグ、ioctl番号定義等)を機械的に
    `any(target_os = "linux", target_os = "android")`へ拡張。
  - `src/rename_flags.rs`のみ、bionic libcの`libc`クレートで
    `RENAME_*`定数がi32型になる(glibcではu32)差異を`as u32`
    キャストで吸収。
  - 詳細・アップストリーム提案用diffは
    `third_party/README.md`・`third_party/fuser-android-upstream.patch`
    参照。
- Cargoの制約(同一依存名で異なるソース(レジストリ/パス)をOSごとに
  切り替えることはできない)のため、android向けは別の依存キー
  `fuser_android`とし、`lib.rs`側で
  `extern crate fuser_android as fuser;`によりクレート名を
  `fuser`へエイリアスすることで、`fuse_mount.rs`等の既存コードは
  Linux/macOS/Androidの3OSで無変更のまま使い回せるようにした。
- 検証: `cargo ndk -t arm64-v8a check --no-default-features --features
  fuse_backend`・`--features fuse_backend,foreign_fs`ともに成功
  (`fuse_mount`・`foreign_fuse_mount`・`orzctl`バイナリ含む)。
  既存のWindows/Linux向け`cargo test --no-default-features`にも
  リグレッション無し。
- 未検証: 実機Android端末上での実際のマウント動作(root権限・
  SELinuxポリシー起因の追加制約が別途あり得るため、上記「対応候補3」
  は引き続き残課題)。

これにより、「ライブラリレベルの障害(`fuser`が非対応)」自体は
解消された。次の残課題は、実機(rooted Android端末またはAVD)での
実際のマウント検証。
