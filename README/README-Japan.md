# open-raid-z

Windows/Linux上でNTFS/exFATとほぼ互換性を保ちながら、ZFS風の機能(チェックサム自己修復・ストレージプール・コピーオンライト・スナップショット/クローン)とRAID0/1/5/6/10/Z2/Z3を提供する、実験的なファイルシステムプロジェクトです。コアロジックはOSを問わない単一の共有プログラム(`open_raid_z_core`)であり、Windows版(WinFsp)・Linux版(FUSE)はその上に載る薄いマウント層の違いに過ぎません(配布物としては`open-raid-z-win`/`open-raid-z-linux`という名前で分ける予定)。

言語: **日本語** | [UK English](README-UK-English.md) | [US English](README-US-English.md) | [Italiano](README-Italy.md) | [Français](README-France.md) | [Deutsch](README-Germany.md) | [Русский](README-Russia.md) | [Українська](README-Ukraine.md) | [العربية](README-Arabic.md) | [فارسی](<README-Iran(Persian).md>)

## Microsoft社・Apple社の皆様へ

私たちは、Windows上でZFS風の完全な機能(チェックサム自己修復・RAID6/RAID-Z2・スナップショット等)を実現する、この実験的なファイルシステムを開発しております。私たちの目標の一つは、将来的にこのファイルシステムをWindows/macOSの正式なインストール先・起動ディスクとして選択できるようにすることです。

これには、ブート起動ドライバの署名認証・インストーラーでの正式なサポートなど、各OSベンダー様のご協力が不可欠であると理解しております。もしこの取り組みにご興味をお持ちいただけましたら、ぜひご連絡・ご協力をいただければ幸いです。個人・小規模な取り組みではございますが、この技術の実現を強く望んでおります。

## 命名規則

ディレクトリ名・crate名・npmパッケージ名・Cargo feature名・HTML/CSSの
id/class名など、このプロジェクト自身が定義する識別子は、ハイフン(`-`)
ではなく**アンダースコア(`_`)区切りに統一**している
(例: `open_raid_z_core`、`zfs_accel_hlsl`、`open_runo_installer`、
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
| `open_raid_z_core` | RAID-Z/RAID0-10 vdev、ストレージプール、NTFS ACL/exFAT属性互換層、実マウント(Windows=WinFsp `mount.rs` / Linux=FUSE `fuse_mount.rs`、OS別マウント層以外は完全に共有) |
| `zfs_accel_hlsl` | GPU/NPUハードウェアアクセラレータ(DirectX 12 Compute + DirectML)によるパリティ計算オフロード |
| `open_runo_installer_core` | ディスク検出・Copilot風構成アドバイザー・zpool初期化プレビューのOS非依存ロジック(Tauri非依存、Linux/macOSでも`cargo test`可能) |
| `open_runo_installer` | Tauri製インストーラー本体(`open_runo_installer_core`を呼び出す薄いUI層)。ハードウェア検出・zpool初期化ウィザード・Copilot風構成アドバイザーのUI |

## 主な機能

- **RAID全系列に対応**: RAID0 / RAID1(ミラー) / RAID5 / RAID6 / RAID10(ストライプ+ミラー) / RAID-Z2 / RAID-Z3
- **ディスクのパーティション分割・使い回し**: 1台のディスクを分割し、片方をミラー、もう片方を別のRAID6/Z2配列のメンバーにする、といった構成も可能
- **チェックサム自己修復・コピーオンライト・スナップショット/クローン**: ZFSと同じ考え方をエミュレーション。`Pool::scrub`でプール全体のサイレント破損を一括検知・修復可能(RAID-Z系・RAID10のどちらでも共通のAPIで実行可能)
- **NTFS互換**: ACL(NFSv4⇔NTFS)・UID/GID⇔SIDマッピング(ローカルSAM/ADドメインのRIDベース決定論的マッピング)
- **exFAT互換**: ファイル属性・タイムスタンプの相互変換、4GB超ファイル/大容量ボリューム対応
- **GPU/NPUハードウェアアクセラレーション**: DirectX 12 Compute + DirectMLでRAID-Z1/Z2/Z3のパリティ生成をオフロード(ハードウェアが無い場合はCPUへ自動フォールバック)。さらに、GF(2^8)の係数倍をGF(2)ビット行列に変換して1回のDirectML GEMM呼び出しへ帰着させる方式(`zfs_accel_hlsl::dml_gemm`)も実装し、実機GPUで正しさを検証済み(実機NPUでは未検証)。この仕組みはscrub/resilverが破損を検知した際の復旧計算(=パリティチェック)にも実際に配線済み。NPU専用のシェーダ経路(`raidnpu_*.hlsl`)も用意し、将来の実機NPUでの検証・最適化に備えている
- **Vulkan Computeアクセラレーション(Windows以外)**: DirectX/DirectMLはWindows専用APIのため、Linux/Mac/Android向けに`ash`クレート経由のVulkan Compute実装(`zfs_accel_hlsl::vulkan_compute`、`vulkan` feature)を追加。RAID-Z1のXORパリティ生成が実機GPU(NVIDIA GeForce GT 730、Vulkan 1.2)で正しく動作することを確認済み
- **既存フォーマットの読み書きブリッジ(`foreign_fs`)**: open-raid-z独自のプール形式とは別に、他OSが作成した既存のFAT32/FAT16ボリューム(USBメモリ/microSD/CFカード等)を読み書き、exFATボリュームを読み取り可能(exFATは上流クレートの制約により現時点で読み取り専用)。`orzctl foreign`(`ls`/`cat`/`put`)から操作できる
- **インストーラーの「対応状況」パネル**: ボタンで開閉できるパネルで、現在のOSの対応状況、検出された全GPU/NPU(Intel/AMD/NVIDIA/Qualcommベンダー判定付き、複数対応)、検出されたストレージメディアの種別(HDD/SSD/NVMe/USB/SD/CF)を一覧表示
- **実ディスクへのzpool適用**: インストーラーのzpool初期化ウィザードに、スクラッチイメージでのプレビューだけでなく実際の物理ディスク(`\\.\PhysicalDriveN`)へ適用するコマンド(`init_zpool_apply`)を追加。既存データの消去を明示的に確認するフラグが無いと動作しない安全設計
- **Copilot風構成アドバイザー**: ディスク構成・アクセラレータ・CPUコア数から推奨RAIDレベルを提案(ヒューリスティック版。ローカルLLM検知の骨組みも搭載)。ロジックは`open_runo_installer_core`としてTauriから独立しており、Linux/macOS上でも`cargo test`で検証可能
- **WinFsp実マウント(Windows)**: 実際にWindows上のドライブレターとしてマウント可能。プール内の全データセットがそれぞれ1ファイルとして見え、バイト単位の任意オフセット読み書き・ファイルの新規作成/削除/名前変更/追記/切り詰めに対応(現状はルート直下のフラットな名前空間のみで、サブディレクトリは未対応)。実機での読み書き・作成・削除・リネーム・追記・切り詰めをそれぞれ実際にマウントした状態で検証済み。
- **FUSE実マウント(Linux)**: WinFsp版と同じ`Pool`をそのままLinux上へマウント可能(`fuse_mount.rs`)。機能はWindows版と同等(作成/削除/リネーム/追記/切り詰め)で、実際にWSL2 Ubuntu上でマウント・`std::fs`経由の読み書きまで検証済み。inodeベースの設計のため、WinFsp版にある「リネーム中に別ハンドルが古い名前を参照し続ける」という既知の制約はこちらには無い。`fuser`クレートには`macfuse-4-compat` featureがあり、将来macOS(データボリュームとして、起動ディスクとしてではない)にも同じ設計を拡張できる見込み。
- **多言語対応**: インストーラー(OpenRaidZインストーラー)は英語をデフォルトに、10言語のUI言語切り替えに対応(インストール後も変更可能)
- **既存データの移行ツール(`migrate`モジュール、実験的)**: 既存のNTFS等のディレクトリをプールへコピーして取り込む。コピー元(ソース)には一切書き込まないため、**起動中のWindowsを止めずに**実行できる。ただし**現在起動中のシステムドライブ(C:等)自体をその場でRAID形式へ無停止変換することはできない**(OS自身が使用中のボリュームを、そのOS上のソフトが書き換えることは原理的に不可能なため)。あくまで「別の場所(プール)へコピーする」ツール。現状はライブラリ関数のみでCLI/GUIは未実装、サブディレクトリは区切り文字で1階層に平坦化される
- **メタデータの永続化(`Pool::save`/`Pool::open`)**: データセット一覧・ストライプ割当・スナップショット等の管理情報を、プール内の予約領域(スーパーブロック)へ保存・復元できる。以前はこの仕組みが無く、実データのバイト列はディスクに残っていても、プロセスを終了(アンマウント)すると「どのファイルがどこにあるか」という情報ごと失われていた。Windows(WinFsp)・Linux(FUSE)いずれも、マウント中の変更操作のたびに自動保存し、**実際にアンマウント→再マウントしてもファイルが残ること**を実機で検証済み。

## 容量・ファイルサイズの上限

- データセット(ファイル)1つあたりの論理サイズは`u64`で一貫して管理しており、FAT32の4GB境界のような人為的な上限は無い(理論上は2^64バイトまで)。動画・画像など大きなファイルも、以下の実際の制約の範囲内であれば問題なく保存できる。
- 実際の上限は、**プールの空き容量**(接続した各ディスクの実容量の合計から、RAIDレベルごとの冗長化オーバーヘッドを引いたもの)で決まる。例えばRAID-Z2(2重パリティ)なら「データ用ディスク実容量の合計」が実質的な上限。
- WinFspの1回のread/write呼び出しあたりの転送量はWindows API自体の制約で最大約4GiB(`u32`)だが、これは実際のファイルシステムと同じ制約であり、OS/アプリケーション側が自動的に分割して読み書きするため実運用上の上限にはならない。
- コピーオンライト(CoW)の性質上、書き込み(新規作成・追記・上書きいずれも)には常に最低1ストライプぶんの空き容量がプールに必要(ZFSの`slop space`と同じ考え方)。さらに、メタデータ保存用に1ストライプが常に予約される。プールを完全に100%使い切った状態にすると、既存データの上書きすら失敗する。運用上は、プール容量の数%は常に空けておくことを推奨する。

## 現状の制約(プロトタイプ段階)

- WinFspマウントはルート直下のフラットな名前空間のみ対応。サブディレクトリの作成・列挙は未対応(ファイル単位のcreate/delete/renameはサポート済み)。
- ファイルの読み書きは`Pool::read_unaligned`/`Pool::write_unaligned_growing`(read-modify-write層)経由でバイト単位の任意オフセット・任意長に対応し、書き込みが現在のサイズを超える場合は自動的にファイルが伸びる(容量・PATH設定については上記「容量・ファイルサイズの上限」を参照)。
- `Pool`はまだ`RaidZVdev`/`Raid10Vdev`両対応だが、RAID10はデータセットAPIとの統合が浅い部分がある。
- WinFsp実マウント関連のコード(`mount.rs`)は`winfsp`クレートがedition2024を要求するため、Rust 1.85未満のツールチェインではビルドできない(後述のビルド・テスト参照)。
- `mount.rs`・`zfs_accel_hlsl`のGPU実装(`gpu` feature)は`windows`クレートに依存するが、同クレートはコンパイルターゲットが実際にWindowsでない限り中身が空になる。そのためこれらのコードはWindows実機(またはWindowsターゲットへのクロスコンパイル)でのみビルド・テストでき、Linux/macOS上では`--no-default-features`でこれらを無効化した場合のみビルドできる。
- リネーム(`rename`)は、対象を指す**他の**オープンハンドルが残っている状態で行うと、そのハンドル経由の以後の操作が失敗しうる(`FileHandle`が名前を直接保持する設計のため。詳細は`Pool::rename_dataset`のドキュメント参照)。

## ビルド・テスト

```powershell
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features        # WinFsp/GPUアクセラレータ無し(CPUロジックのみ、dxcもWinFsp SDKも不要)
cargo test                              # 既定(WinFsp実マウント+GPU/NPUアクセラレータを含む、要WinFsp+dxc)
```

`--no-default-features`は`winfsp_backend`・`gpu_accel`の両featureを無効化し、RAID0/1/5/6/10/Z2/Z3・チェックサム自己修復・CoW・スナップショット/クローン・resilverなどのコアロジックをOS非依存(Linux/macOSでも可)で検証できる。WinFsp・DirectX Shader Compiler(dxc)・GPU/NPUハードウェアは一切不要。

既定feature(`winfsp_backend` + `gpu_accel`)でのビルドには以下が必要:

- WinFsp本体(https://winfsp.dev/)がシステムにインストールされていること(SDKヘッダはビルド時に自動でベンダリングされたものを使用するため、開発者向けコンポーネントの追加インストールは不要)。
- `dxc`(DirectX Shader Compiler。Windows SDKまたはVulkan SDKに同梱)がPATH上にあること(RAID-Z/Z2パリティ計算用HLSLシェーダのビルド時コンパイルに使用)。
- **Rust 1.85以降**(`winfsp`クレートが要求する`edition2024`が安定化されたバージョン。これより古いツールチェインでは`Cargo.toml`のマニフェスト解析自体が失敗する)。

WinFsp・dxcのどちらか一方だけを個別に無効化することも可能(`--no-default-features --features gpu_accel`でWinFsp無し・GPUのみ有効、など)。

**実際に`winfsp_backend`のテスト(実マウント)を実行する場合の注意**: `winfsp`クレートはWinFspのDLL(`winfsp-x64.dll`)を`LoadLibraryW`で動的にロードするが、標準のDLL検索パス(実行ファイルのあるフォルダ・`System32`・`PATH`)しか見ないため、WinFspインストーラーが`PATH`に追加してくれない環境では、WinFsp SDKヘッダ無しでビルド自体は通っても**実行時に必ず失敗する**(エラー`WIN32(1285)`=`ERROR_DELAY_LOAD_FAILED`)。この場合はテスト実行時のみ`PATH`にWinFspの`bin`ディレクトリを一時的に追加すること:

```powershell
$env:PATH = "C:\Program Files (x86)\WinFsp\bin;$env:PATH"
cargo test --features winfsp_backend,gpu_accel
```

この`PATH`追加を行わずに実行すると、`mount_pool`が`Err`を返すため、テスト側は(実行環境依存の問題として)`eprintln`でスキップメッセージを出して早期リターンする。**`--nocapture`を付けずに実行すると、このスキップは`ok`としか表示されず、実際にマウント・読み書きが検証されたのか単にスキップされただけなのか区別が付かない**ので、実マウント系のテストを確認する際は必ず`--nocapture`を付けて、スキップメッセージが出ていないことを目視で確認すること。

### Linux版(FUSE)のビルド・テスト

```bash
# Ubuntu/Debian系の場合。build-essential・pkg-config・libfuse3-devが必要。
sudo apt-get install -y build-essential pkg-config libfuse3-dev

cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features --features fuse_backend
```

`fuse_backend` featureは`fuser`クレート(Linuxの`libfuse3`への実バインディング)を有効化する。`winfsp_backend`/`gpu_accel`とは独立しており、Linux以外のターゲットでは`fuser`自体が依存関係に存在しないため有効化できない(Cargo.tomlで`target.'cfg(target_os = "linux")'.dependencies`配下に置いているため)。実マウントの統合テスト(`tests/fuse_mount.rs`)はWSL2 Ubuntu 26.04上で実際に検証済み(新規作成・書き込み・読み込み・リネーム・切り詰め・削除・複数ストライプにまたがる大きめファイルの往復、アンマウント→再マウントをまたいだデータ永続化)。WindowsのみでLinuxビルドを試す場合はWSL2(`wsl --install`)の利用を推奨する。

コマンドラインから直接プールを作成・マウントするための`orzctl`も同梱している:

```bash
cargo build --no-default-features --features fuse_backend --bin orzctl
./target/debug/orzctl create --level z2 --chunk-size 4096 --stripes 1000 --dataset tank /path/to/disk0 /path/to/disk1 ...
./target/debug/orzctl mount  --level z2 --chunk-size 4096 --stripes 1000 --mountpoint /mnt/tank /path/to/disk0 /path/to/disk1 ...
```

起動時に自動マウントしたい場合は
[`contrib/systemd/open-raid-z-pool.service.example`](../open_runo_zfs_source/open_raid_z_core/contrib/systemd/open-raid-z-pool.service.example)
をsystemdユニットとして登録する(実際にVirtualBox VM上で4台の独立した
ブロックデバイスに対して作成したプールを、本物の再起動をまたいで
自動マウントできることを検証済み)。

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

## マルチOS対応・既存フォーマット相互運用ロードマップ

open-raid-z自体をWindows/Mac/Linux/Android/iOS/iPadで読み書きできるようにし、
既存の他OSフォーマット(NTFS/exFAT/FAT32/ext4/APFS等)とも相互運用できるように
することを目指している。現状の実現可否・優先順位・技術的制約(特にiOS/iPadは
サードパーティのブロックデバイスRAID構成をAppleが許可していないため
File Provider Extension経由の閲覧に限定される見込み)は
[`MULTIPLATFORM_ROADMAP.md`](open_runo_zfs_source/open_raid_z_core/contrib/systemd/MULTIPLATFORM_ROADMAP.md)
に記録している。GPU/NPUアクセラレーションはWindows以外では各OS標準の
ネイティブAPI(Mac=Metal Performance Shaders、Android=NNAPI等)へ順次
対応していく方針。また、mdadm(Linux)やStorage Spaces(Windows)といった
**他社製RAID形式との相互運用**も将来的な対応範囲として検討中。

## ライセンス

MPL-2.0
