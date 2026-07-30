# PORTING.md — open-raid-z お引越しファイル

> ⚠️ **2026-07-26 緊急引き継ぎ**: VPS上で`karu.tokyo`がHTTPS証明書無しの
> 状態(Let's Encryptレート制限、復旧目安2026-07-27 00:17:48 UTC頃)。
> どのリポジトリから作業を再開する場合も、まず本ファイルまたは
> `CLAUDE.md`のHANDOFF最新エントリを確認してください。詳細・復旧手順は
> `CLAUDE.md`参照。`open-web-server`のTLS証明書永続化バグ(再起動で
> 全ドメインのHTTPS証明書がメモリから消える)の恒久修正が
> `open-web-server`リポジトリ側で進行中——VPS上のこのサービスを
> 安易に再起動しないこと。 開発方針ファイル(`CLAUDE.md`)の見出しを
> 「設計思想＆開発方針＆開発環境ルール」へ改名しました
> (設計思想・開発方針・開発環境ルールを明確に区別)。移設先でも
> `CLAUDE.md`の内容を必ず確認してください。

> **2026-07-25(セッション末尾チェックポイント)**: 本リポジトリの
> `offsite_backup.rs`(Email/Googleドライブ/SFTP退避)は、
> open-easy-web/open-web-server/aruaru-dbの3リポジトリへ
> path依存として移植・再利用済み(`default-features = false`、
> `offsite_backup` feature)。同じ移植パターンを他プロジェクトへ
> 適用する場合は、各リポジトリのCLAUDE.md HANDOFF内の実装例を参照。

> **2026-07-26(シャットダウン前チェックポイント)**: `aruaru-db`側の
> `RaftWriter`実装は、内部可変性(`parking_lot::Mutex<Option<Arc<..>>>`)
> による「構築後の後付け注入」パターンへ更新済み——`Arc`共有後の
> インスタンスへ`offsite_backup`系の設定を注入する必要がある他
> プロジェクトは、この実装例を参照すると良い(詳細はaruaru-db側
> CLAUDE.mdの2026-07-25(続き2)HANDOFF参照)。


> このファイル1枚で、他プロジェクト/他マシンへ open-raid-z を
> 導入・移設できます。
>
> 対象バージョン: `open_raid_z_core` 0.0.1 / `zfs_accel_hlsl` /
> `open_runo_installer_core` 0.1.0(3クレート・170テスト
> [108 + 32 + 30]、`--no-default-features`のCPUフォールバック構成での
> 実測値。`foreign_fs`有効時はさらにext2/ext4読み取りブリッジの統合
> テスト8件が加わり、Windows実測112、Linux(WSL2、`fuse_backend`込み)115。
> `offsite_backup`有効時はEmail/Googleドライブ/SFTPのモック結合テスト
> 3件が加わり111)
> 最終更新: 2026-07-25

---

## 0. 配布インストーラー(2026-07-23追加)

`install.sh`(Linux、systemdは対象外——プールごとにディスク構成が異なる
ため、`orzctl`バイナリの`/usr/local/bin`配置のみ)/`install.ps1`
(Windows、WinFsp前提)/`.github/workflows/release.yml`(タグpushで
Linux x86_64・Windows x86_64向け`orzctl`を自動ビルドしGitHub Releasesへ
添付)を追加。**移植時の注意**: `open_raid_z_core`はワークスペール
ルート直下ではなく`open_runo_zfs_source/open_raid_z_core`という
ネストした位置にあるため、CI等でビルドする際は`working-directory`を
明示すること。また既定feature`gpu_accel`はdxc(DirectX Shader Compiler)
を要求するため、dxcの無い環境(多くのCI含む)では
`--no-default-features --features winfsp_backend,foreign_fs`
(Windows)/`fuse_backend,foreign_fs`(Linux)のように明示的に除外する
必要がある。

---

## 1. open-raid-z とは(30秒版)

**open-raid-z**は、ZFS(OpenZFS)の設計思想——パリティ分散ストライピング
(RAID-Z/Z2/Z3)・チェックサムによる自己修復・Copy-on-Write・スナップ
ショット/クローン——を、**OpenZFS自体には一切依存せず**Rustで一から
実装したストレージプールです。`orzctl`というCLIツールでプールを作成し、
**実際にOSへマウントできます**(Windows=WinFsp、Linux/macOS/Android=FUSE)。

独自のオンディスクフォーマットであり、実際のZFSとはオンディスク互換性が
ありません(移行はコピーベース、[MIGRATION.md](MIGRATION.md)参照)。
poem-cosmo-tauri/open-runoのようなREST/GraphQL APIサーバーではなく、
**ライブラリクレート + CLIバイナリ + (任意で)カーネルドライバ**という
形態のプロジェクトです。

| 分類 | 提供機能 |
|---|---|
| RAID | `Raid0`/`Raid1`(ミラー)/`Raid5`/`Raid6`(=`Z2`)/`Z2`/`Z3`。RAID10は`Raid1`ミラーグループの束ね |
| データ保全 | sha2チェックサムによる破損検知、Copy-on-Write、スナップショット/クローン(参照カウント管理) |
| 実マウント | Windows: WinFsp / Linux・macOS・Android: FUSE(`fuser`クレート、Androidは自前パッチフォーク使用) |
| 既存フォーマット連携 | FAT32/exFAT の読み書き+ext2/ext4 の読み取り相互運用(`foreign_fs` feature、純Rust実装でネイティブライブラリ不要) |
| GPU高速化(任意) | RAID-Z/Z2/Z3のガロア体パリティ計算をHLSL+D3D12/DirectMLでGPUオフロード(`gpu_accel` feature) |
| インストーラGUI | ディスク検出・zpool構成助言(`installer_core`、OS非依存ロジック) + Tauri 2 GUI(`open_runo_installer`) |
| 切断耐性・オフサイト退避(任意) | 書き込みWrite-Aheadジャーナル(`journal.rs`)+再接続時の自動復旧(ライブI/O非ブロッキング、`disaster_recovery.rs`)+切断直前セグメントのEmail/Googleドライブ/SFTP退避(`offsite_backup` feature)+圧縮のCPU/GPU/NPU抽象化(`accel.rs`)。ストレージプール専用ではなく、任意のデータをバックアップしたい他アプリからもライブラリとして呼べる汎用設計 |

## 2. 持っていくもの(ファイル一覧)

```
open-raid-z/
├── open_runo_zfs_source/
│   ├── open_raid_z_core/        ← 中核クレート(RAID・CoW・チェックサム・マウント・orzctl)
│   ├── zfs_accel_hlsl/          ← GPU高速化クレート(open_raid_z_coreのpath依存)
│   ├── open_runo_installer_core/← インストーラのOS非依存ロジック(単独crate)
│   ├── open_runo_installer/     ← Tauri 2 + TypeScriptデスクトップGUI(任意)
│   ├── wdk_driver/orzflt/       ← Windowsカーネルドライバ最小スケルトン(任意・実験的)
│   └── third_party/
│       └── fuser-0.17.0-android-patch/  ← Android向けfuserパッチフォーク(fuse_backend使用時のみ必要)
├── MIGRATION.md                 ← 既存ZFS/NTFS/ext4/他社RAIDからの移行手順
├── CLAUDE.md                    ← 開発ルール
└── PORTING.md                   ← 本ファイル
```

丸ごと移設する場合は`open_runo_zfs_source/`ごとコピーして、
`open_raid_z_core`ディレクトリで`cargo test --no-default-features`が
通れば移設成功(104テスト、下記4節参照)。ライブラリとして使う場合は
`open_raid_z_core`(+ 必要なら`zfs_accel_hlsl`)だけを取り出せます。

## 3. 依存の書き方(新プロジェクトの Cargo.toml)

```toml
[dependencies]
# 同一マシンにある場合(path依存)
open_raid_z_core = { path = "../open-raid-z/open_runo_zfs_source/open_raid_z_core" }

# GitHub公開後はgit依存でも可
# open_raid_z_core = { git = "https://github.com/aon-co-jp/open-raid-z" }

[features]
# CI・WinFsp SDK/dxc無し環境向け(デフォルトはwinfsp_backend+gpu_accel有効)
default = []
```

`open_raid_z_core`側のfeatureは呼び出し側のCargo.tomlで選択します:

- `winfsp_backend`(既定): Windows実マウント(WinFsp SDKが必要)
- `gpu_accel`(既定): GPUパリティ計算(`zfs_accel_hlsl`のGPU実装、dxcが必要)
- `fuse_backend`: Linux/macOS/Android実マウント(FUSE、Windows以外)
- `foreign_fs`: FAT32/exFAT読み書き+ext2/ext4読み取り(全OSで有効化可、ネイティブライブラリ不要)

WinFsp SDK・dxcを用意できない環境では
`open_raid_z_core = { path = "...", default-features = false, features = ["foreign_fs"] }`
のように無効化すればCPUフォールバックのみでビルド・テストできます。

## 4. 組み込みレシピ

### 4.1 プールの作成・マウントをライブラリとして呼び出す

```rust
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};
use open_raid_z_core::pool::Pool;

// 6台のブロックデバイス(またはファイルベースのモックデバイス)でZ2構成
let vdev = RaidZVdev::new(devices, RaidLevel::Z2, /* chunk_size */ 4096);
let pool = Pool::new(vdev, /* stripes */ 100_000, "tank")?;
// 実マウント(Windows): mount.rs の WinFsp 実装を経由
// 実マウント(Linux/macOS/Android): fuse_mount.rs の FUSE 実装を経由
```

具体的なAPI(`Pool`/`RaidZVdev`/`mount`/`fuse_mount`)の詳細は
`open_raid_z_core/src/lib.rs`のモジュール一覧を参照してください。

### 4.2 CLIとして使う(`orzctl`)

```sh
cargo build -p open_raid_z_core --bin orzctl --release
./target/release/orzctl create --level z2 --chunk-size 4096 --stripes 100000 \
  --dataset tank /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg
./target/release/orzctl mount  --level z2 --chunk-size 4096 --stripes 100000 \
  --mountpoint /mnt/tank /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg
```

Windowsではディスク指定を`\\.\PhysicalDriveN`形式にします。

### 4.3 既存FAT32/exFAT/ext2/ext4ボリュームとの相互運用だけを使う

```toml
open_raid_z_core = { path = "...", default-features = false, features = ["foreign_fs"] }
```

```sh
orzctl foreign ls /dev/sdb1
orzctl foreign --format exfat mount /dev/sdc1 /mnt/old_exfat
orzctl foreign --format ext4  ls   /dev/sdd1 /home        # ext2/ext4は読み取り専用
orzctl foreign --format ext4  mount /dev/sdd1 /mnt/old_ext4  # 読み取り専用(RO)マウント
```

`foreign_fs`は`fatfs`/`fscommon`/`hadris-fat`/`ext4-view`という純Rust
クレートのみに依存し、追加のネイティブライブラリを要しません。
ext2/ext4は読み取り専用です(書き込み対応の成熟した純Rust実装が無いため。
`put`等の書き込み系操作は明示的にエラーを返し、FUSEマウントも`RO`で
行われます)。

### 4.4 GPU高速化なしのCPUのみでRAID-Z演算を使う

```toml
open_raid_z_core = { path = "...", default-features = false }
zfs_accel_hlsl = { path = "...", default-features = false }
```

`gpu_accel`を無効化すると`zfs_accel_hlsl`は純Rustのガロア体演算
(`galois.rs`/`gf_matrix.rs`)にフォールバックし、WinFsp SDK・dxc・
Windows SDKいずれも不要になります(CI環境向け、下記5節の163テストは
すべてこの構成で計測)。

### 4.5 インストーラGUI(Tauri)を移設する場合

`open_runo_installer_core`(OS非依存ロジック)と
`open_runo_installer`(Tauri 2 + TypeScript GUI)はセットで移設します。
`installer_core`はTauriに依存しないため、GUIを持たないCLIツールへも
単独で組み込み可能です。

```toml
open_runo_installer_core = { path = "../open-raid-z/open_runo_zfs_source/open_runo_installer_core" }
```

### 4.6 切断耐性ジャーナル・オフサイト退避・自動復帰だけを使う(他アプリからの利用も想定、2026-07-25追加)

```toml
open_raid_z_core = { path = "...", default-features = false, features = ["offsite_backup"] }
```

```rust
use open_raid_z_core::offsite_backup::{
    OffsiteBackupTarget, SftpBackupTarget, SftpBackupTargetConfig,
};

let target = SftpBackupTarget::new(SftpBackupTargetConfig {
    host: "backup.example.com".to_string(),
    port: 22,
    username: "raidz".to_string(),
    password_env: Some("RAIDZ_SFTP_PASSWORD".to_string()), // 値は環境変数から
    remote_backup_dir: "backup".to_string(),
});
target.ensure_ready()?;                                  // 初回: リモートフォルダ確認/作成
target.upload_segment("00000000000000000001.entry.gz", &journal_bytes)?; // 切断時
// 再接続後の自動復帰:
for label in target.list_segments()? {
    let data = target.download_segment(&label)?;
    // ローカルへ反映...
    target.delete_segment(&label)?;
}
```

`EmailBackupTarget`/`GoogleDriveBackupTarget`も同じ`OffsiteBackupTarget`
トレイトを実装しているため差し替え可能(Emailは送信専用、
`list_segments`/`download_segment`は非対応で`OffsiteBackupFailed`を返す
設計——できないことを`NotImplemented`ではなく明示的なエラーで正直に返す)。
**このリポジトリはストレージプール専用の機能として作っていない**——
`journal.rs`/`disaster_recovery.rs`/`offsite_backup.rs`/`accel.rs`は
`Pool`型に依存しない独立モジュールのため、`open-easy-web`のような
他アプリが「切断に強いバックアップ機能」を個別実装せずこのクレートへの
path依存だけで再利用できる(「分身の術」——1つの共有実装を複数アプリが
個別インストール無しで呼び出す既存パターンと同じ思想)。

**移植時の実装上の注意(実際に踏んだ罠)**:
- `russh_sftp::client::SftpSession::write()`(高レベル便利関数)は内部で
  `AsyncWriteExt::write_all`のみ呼び、書き込み確認応答の完了待ち・
  SSH_FXP_CLOSE送信を行わずに返る。直後に同じファイルを再オープンして
  読み取ると空データが返ることがある(実際にテストで再現・修正した罠)。
  書き込み完了を保証したい場合は`open_with_flags`+`write_all`+
  `AsyncWriteExt::shutdown`(flush+クローズの完了待ち)を明示的に呼ぶこと。
- SMTPサーバーのモック実装で認証方式を限定的にしか実装していない場合、
  `lettre`クレートは既定で`Mechanism::Plain`を`Mechanism::Login`より
  優先して選ぶ(`DEFAULT_MECHANISMS = [Plain, Login]`)。サーバーが
  EHLO応答でPLAIN/LOGIN両方を広告するとPLAINが選ばれ、LOGINしか
  実装していないモックでは「unrecognized command」になる。テスト用
  モックサーバーは実装済みのメカニズムだけを広告すること。

## 5. 動作確認

```sh
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features                        # 108テスト(2026-07-25実測)
cargo test --no-default-features --features offsite_backup  # 111テスト(オフサイト退避モック結合テスト込み)

cd ../zfs_accel_hlsl
cargo test --no-default-features   # 32テスト(CPUフォールバック)

cd ../open_runo_installer_core
cargo test                          # 30テスト
```

3クレート合計 **170テストpassed、failed 0**(2026-07-25実測、
WinFsp SDK/dxc/Windows SDK不要の構成)。`open_raid_z_core`に
`--features foreign_fs`を加えるとext2/ext4読み取りブリッジの統合テストが
加わり、Windows実測112、Linux(WSL2、`fuse_backend,foreign_fs`)で115に
なる。`--features offsite_backup`を加えるとEmail/Googleドライブ/SFTPの
モック結合テスト3件が加わり111になる(実クラウドアカウント・実SMTP・
実VPSへは接続せず、ローカルの偽サーバーのみで検証)。`default`feature
(実マウント+GPU高速化)を有効にした構成はWindows実機+WinFsp SDK+dxcが
必要なため別途確認してください。

## 6. データのお引越し(既存環境から)

既存のZFS(OpenZFS)・NTFS・ext4・他社製RAIDから`open-raid-z`へは、
**オンディスクフォーマットが異なるため直接読み込みできません**。
必ず「①既存フォーマットから読み出し可能な状態にする →
②`orzctl`で作成・マウント済みのプールへ通常のファイルコピー
(`rsync`/`robocopy`等)」という手順になります。詳しい移行方式の選び方
(FAT32/exFAT/NTFS/ext4/OpenZFS/他社RAID別)・コマンド例は
[MIGRATION.md](MIGRATION.md)を参照してください。

## 7. 命名規約(お引越し先でも守ること)

- クレート名: `open_raid_z_core` / `zfs_accel_hlsl` / `open_runo_installer_core`(いずれもスネークケース)
- CLIバイナリ名: `orzctl`
- Rustパス: `open_raid_z_core::*`
- カーネルドライバ: `orzflt`(`wdk_driver/orzflt/`)

## 8. 詳細ドキュメント

[README.md](README.md)(構成一覧・feature一覧) /
[MIGRATION.md](MIGRATION.md)(移行手順) /
[CLAUDE.md](CLAUDE.md)(開発ルール・このエコシステムの技術方針の正本) /
[CHAT_HANDOFF.md](CHAT_HANDOFF.md)(開発履歴) /
`open_runo_zfs_source/open_raid_z_core/contrib/systemd/MULTIPLATFORM_ROADMAP.md`
(マルチOS対応ロードマップ)。

## 9. 移植・拡張時の注意

他プロジェクトへ移植・拡張する際、依存クレートの現状(バージョン・
メンテナンス状況)や仕様の詳細で判断に迷う場合は、学習データからの
推測だけに頼らず、実際に検索(日本語・英語の両方のGoogle検索、および
GitHub上の実装状況調査)して裏付けを取ってから技術選定を行うことを
推奨する。

**パフォーマンス・並行処理**: このリポジトリ自体はCPU律速な処理
(Galois体演算によるパリティ計算・チェックサム検証等)が中心のため、
`zfs_accel_hlsl`のGPUオフロード(HLSL/D3D12)に加え、CPUフォールバック
経路でも`rayon`等によるデータ並列化の余地がないか確認する価値がある。
移植先で本エコシステムの他リポジトリ(tokio非同期中心)と組み合わせる
場合は、CPU律速な処理を非同期ランタイムの実行スレッドで直接ブロッキング
実行しない(`tokio::task::spawn_blocking`経由で呼び出す)よう注意する。

**最先端追従の方針(2026-07-23追記)**: 移植先で技術選定に迷う場合の
日英Web検索は上記の通り必須だが、検索結果CockroachDB/TiKV等の最先端の
実運用システムが既に対応済みと判明した設計ギャップが見つかった場合、
「今のところは大丈夫」という報告に留めず、確認を求めず自動でそのまま
実装に着手すること(ユーザー指示、2026-07-23、詳細は`CLAUDE.md`同日
エントリ参照)。移植作業自体がこの技術的ギャップの発見・解消の好機
であることが多い。

**移植可能な実装パターン一覧(2026-07-23、この方針に基づき新規実装
したもの)**:
- **未実装拡張点の安全なフォールバックパターン**: 将来対応予定の
  ハードウェア/バックエンドを`enum`で先に定義し、未実装のものが
  選ばれてもpanicせず既定実装へ安全にフォールバックしつつ
  `tracing::warn!`で可視化する。実装は
  [open-web-server-wire::accel](https://github.com/aon-co-jp/open-web-server/blob/main/crates/open-web-server-wire/src/accel.rs)
  (`AccelBackend::{Cpu,Gpu,Npu,HardwareAccelerator}`)参照。
- **RFC 6298/9002準拠のSRTT/RTTVAR EWMAによるネットワーク品質推定**:
  [RS-SmartTCP](https://github.com/aon-co-jp/RS-SmartTCP)参照
  (O(1)更新、TCP/QUICと同じ枯れたアルゴリズム)。
- **行単位デルタマージによるHTAP列キャッシュ**(TiFlash Delta Tree
  方式): `aruaru-db`の`aruaru-query::olap::OlapCache`参照
  (`arrow::compute::filter_record_batch`+`concat_batches`)。
- **Multi-Raft(Range単位の独立合意グループ)**: `aruaru-db`の
  `aruaru-dist::multi_raft::MultiRaftCluster`参照。
- **HLSL cbufferの配列パディングの罠(2026-07-23、`open-cuda`で実際に
  踏んだバグ、DirectX/HLSLを使う移植先すべてに該当)**: `cbuffer`内で
  `uint key[8]`のようなスカラー配列を宣言すると、**各要素が16バイト
  境界へパディングされる**(`float weights[3]`が3×16=48バイトを占める、
  というよく知られたHLSLの罠)。`SetComputeRoot32BitConstant`で
  隙間なく詰めたdword列を渡す設計と組み合わせると、HLSL側が読む
  バイトオフセットとズレ、値が実質ゼロになる——GPU暗号化カーネルの
  実装で「出力が暗号化されず平文のまま返る」という形で発覚した
  (`open-cuda`側`opencuda-directx`、コミット`ec6acf1`)。**回避策**:
  cbuffer内では配列宣言を避け、`key0`〜`key7`のような個別スカラー
  フィールドとして宣言する(密なレイアウトになりRust/C++側の詰め込みと
  一致する)。DirectX 12 Compute Shaderを使うあらゆる移植先で
  再発しうる罠として記録。
- **NVMe RAID6のGPUパリティアクセラレータ構想(2026-07-30追記、
  未実装・構想メモ)**: RAID6のパリティ計算(XOR/Reed-Solomon)を
  `open-directx`/`open-cuda`のVulkan Compute経由でGPUオフロードする
  ことで、NVMe SSD 4〜8枚構成でのランダムアクセス低速化
  (Read-Modify-Writeのパリティ書き込みペナルティ)を解消するという
  ユーザー要望。クロスプラットフォーム(Windows/Linux/macOS/Android、
  iOS/iPadOSはMoltenVK経由)・クロスベンダー(NVIDIA/AMD/Intel)対応が
  要件。Vulkanベースの`open-directx`は元々この特性(プラットフォーム
  非依存・ベンダー非依存)を持つため、既存の設計方針とは整合する。
  詳細・次回の着手候補は`CLAUDE.md`の2026-07-30付HANDOFFエントリ、
  および`README.md`の該当節を参照。ZFS(RAID-Z2)+高速SLOG(ZIL)や
  ハードウェアRAIDカードのWrite-Backキャッシュ化も、GPUアクセラレータ
  導入と並行して検討すべき代替/補完策としてユーザーから提示されている
  (詳細は同HANDOFFエントリ)。
- **「どこからでも始められる」相互参照の原則(2026-07-23再確認)**:
  このエコシステムの各リポジトリのCLAUDE.mdは、必ず「関連プロジェクト」
  節でopen-raid-z(正本)への参照を持ち、open-raid-z側も「エコシステム
  全体マップ」節で全リポジトリへの入口を提供する。新しい技術的発見
  (今回のHLSL cbufferの罠のような、単一リポジトリに閉じない教訓)は、
  発見元のリポジトリだけでなく正本(open-raid-z)へも必ず記録すること
  ——これにより、どのリポジトリのセッションから作業を再開しても、
  正本を辿れば全体の最新状況・教訓に到達できる状態を保つ。
