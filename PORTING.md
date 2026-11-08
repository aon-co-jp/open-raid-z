# PORTING.md — open-raid-z お引越しファイル

> 🎯 **移植時の前提(2026-08-29)**: aruaru-dbはRPoemとのSETで初めて
> 「REST API不要・Cosmo有料版互換」の価値が成立する(正本:
> aruaru-db/CLAUDE.md冒頭)。open-raid-zはaruaru-dbのZFS互換ストレージ
> 層としてのみ関与し、REST/APIキー管理の対象外。

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

## 0.5. アンインストーラー + `orzctl status` + Web管理UI(2026-07-30追加)

- `uninstall.sh`/`uninstall.ps1`: `orzctl`バイナリのみ削除(プールデータは
  完全に分離された場所にあるため一切触れない設計)。
- `orzctl status`サブコマンド: 保存済みプールを開き(マウントしない)、
  ストライプ使用状況・データセット一覧をJSON出力(`rust-json`経由、
  `json_status` feature既定ON)。Web管理UI等からの機械可読な問い合わせ用。
- `web/`(新規独立クレート): RPoem(`open-runo-poem-compat`)によるWeb
  管理UI。`orzctl status`をサブプロセス呼び出しし、管理者トークン
  (`OPEN_RAID_Z_ADMIN_TOKEN`)必須の`POST /api/create`、
  `OPEN_RAID_Z_READ_ONLY=1`によるread-onlyデモモードに対応。移植先でも
  同じ「CLIをサブプロセス呼び出しし、Web層は薄いラッパーに徹する」設計
  パターンを踏襲できる。

## 0.6. RAID6 GPUパリティアクセラレーション(`open-cuda`連携、2026-07-30)

NVMe RAID6のランダムアクセス低速化(parity write penalty)対策として、
`open-cuda`側にP-parity(XOR)・Q-parity(Reed-Solomon、GF(2^8))のGPU
カーネルを実装・実機検証済み(詳細は`open-cuda`のCLAUDE.md/PORTING.md
参照)。本リポジトリ側の実パリティ計算経路(`open_raid_z_core::vdev`)
への統合はまだ未着手——移植先で統合する場合は、まず`open-cuda`側の
`raid6_xor_parity`/`raid6_q_parity`カーネルのAPI契約(データレイアウト・
係数バッファ形式)を確認してから着手すること。

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

## API 方針: REST 全廃・gRPC / Connect へ (2026-11-04)

### 日本語
- **REST は全廃する。新しい API は REST で作らない。** ブラウザや `curl` から呼ぶ API は **Connect の単純呼び出し**(`POST /<パッケージ>.<Service>/<Method>`、`content-type: application/json`、本文は JSON)、サービス間は **gRPC / gRPC-web**(RPoem の管理API `open_runo.admin.v1.Admin` と同じ流儀)。ストリーミングは、必要になってから。
- **既存の REST は、触るときに順に移す。** 移したら旧 REST の経路は消す。移行が終わるまで残るが、**新規には追加しない**。
- **JSON は RS-JSON に統一**する(読み込みは `from_str_secure` / `parse_secure` でサイズ・深さ・重複キーの上限つき、書き出しは `rust_json::fast`)。ブラウザの WASM に `prost` は入れない(サイズが増えるため)。
- **例外(自分たちの API ではなく、相手の規格が決めているもの)**: ACME、OAuth/OIDC のコールバック、第三者(GitHub など)からの Webhook、ヘルスチェック、静的ファイル配信、OTLP など。これらは規格どおりの HTTP を受ける。
- 参考実装: `RPoem`(`open_runo.admin.v1.Admin`)、`open-easy-web`(`server/src/connect.rs`、`proto/open_easy_web/admin/v1/admin.proto`)。
- ※ このリポジトリのコードが既に方針どおりかどうかは、各リポジトリの現状による。この節は方針の記録であり、コードの変更を意味しない。

### English
- **REST is being abolished. New APIs are not built as REST.** APIs called from browsers or `curl` use **Connect unary calls** (`POST /<package>.<Service>/<Method>`, `content-type: application/json`, JSON body); service-to-service calls use **gRPC / gRPC-web** (the same style as RPoem's admin API `open_runo.admin.v1.Admin`). Streaming only when it is actually needed.
- **Existing REST endpoints are migrated one by one, when they are touched.** The old REST route is removed once migrated. Until migration is finished they remain, but **no new ones are added**.
- **JSON is unified on RS-JSON**: reading with `from_str_secure` / `parse_secure` (size, depth and duplicate-key limits), writing with `rust_json::fast`. `prost` is not put into browser WASM (it would grow the binary).
- **Exceptions (shapes dictated by someone else's standard, not our own API)**: ACME, OAuth/OIDC callbacks, webhooks from third parties (e.g. GitHub), health checks, static file serving, OTLP, and so on. These accept plain HTTP as the standard requires.
- Reference implementations: `RPoem` (`open_runo.admin.v1.Admin`) and `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Note: whether the code in a given repository already follows this policy depends on that repository. This section records the policy; it does not mean the code has been changed.

### Deutsch (KI-Übersetzung, redaktionell überarbeitet; noch nicht von Muttersprachlern geprüft)
- **REST wird vollständig abgeschafft; neue APIs entstehen nicht mehr als REST.** APIs, die aus dem Browser oder per `curl` aufgerufen werden, verwenden **unäre Connect-Aufrufe** (`POST /<Paket>.<Service>/<Methode>`, `content-type: application/json`, JSON-Body). Zwischen Diensten kommen **gRPC bzw. gRPC-web** zum Einsatz, im selben Stil wie die Admin-API von RPoem (`open_runo.admin.v1.Admin`). Streaming wird erst ergänzt, wenn es tatsächlich gebraucht wird.
- **Bestehende REST-Endpunkte werden schrittweise umgestellt, und zwar immer dann, wenn man ohnehin an ihnen arbeitet.** Sobald ein Endpunkt umgestellt ist, wird die alte REST-Route entfernt. Bis zum Abschluss der Migration bleiben die übrigen bestehen, es werden aber **keine neuen mehr hinzugefügt**.
- **JSON wird einheitlich über RS-JSON verarbeitet**: Gelesen wird mit `from_str_secure` bzw. `parse_secure` (mit Obergrenzen für Größe, Verschachtelungstiefe und doppelte Schlüssel), geschrieben mit `rust_json::fast`. `prost` wird nicht in das Browser-WASM aufgenommen, da es die Datei vergrößern würde.
- **Ausnahmen (Schnittstellen, deren Form ein fremder Standard vorgibt und die nicht zu unserer eigenen API gehören)**: ACME, OAuth/OIDC-Callbacks, Webhooks von Drittanbietern (z. B. GitHub), Health-Checks, die Auslieferung statischer Dateien, OTLP und Ähnliches. Diese nehmen, wie vom jeweiligen Standard verlangt, gewöhnliches HTTP entgegen.
- Referenzimplementierungen: `RPoem` (`open_runo.admin.v1.Admin`) und `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Hinweis: Ob der Code eines einzelnen Repositorys diese Richtlinie bereits erfüllt, hängt vom jeweiligen Repository ab. Dieser Abschnitt dokumentiert lediglich die Richtlinie; er besagt nicht, dass der Code schon angepasst wurde.

### Русский (перевод выполнен с помощью ИИ и отредактирован; носителями языка пока не проверен)
- **От REST мы полностью отказываемся; новые API больше не создаются в стиле REST.** API, к которым обращаются из браузера или через `curl`, используют **унарные вызовы Connect** (`POST /<пакет>.<Service>/<Method>`, `content-type: application/json`, тело запроса — JSON). Для взаимодействия между сервисами применяются **gRPC и gRPC-web** — в том же стиле, что и административный API RPoem (`open_runo.admin.v1.Admin`). Потоковая передача данных добавляется лишь тогда, когда в ней действительно возникает необходимость.
- **Существующие REST-эндпоинты переводятся поэтапно — по мере того, как с ними приходится работать.** После перевода старый REST-маршрут удаляется. Пока миграция не завершена, оставшиеся эндпоинты сохраняются, но **новые не добавляются**.
- **Работа с JSON унифицирована на основе RS-JSON**: чтение — через `from_str_secure` или `parse_secure` (с ограничениями на размер, глубину вложенности и повторяющиеся ключи), запись — через `rust_json::fast`. `prost` в браузерный WASM не включается, поскольку это увеличило бы размер файла.
- **Исключения (интерфейсы, форму которых определяет чужой стандарт, а не наш собственный API)**: ACME, обратные вызовы OAuth/OIDC, вебхуки сторонних сервисов (например, GitHub), проверки работоспособности, раздача статических файлов, OTLP и тому подобное. Такие интерфейсы принимают обычный HTTP, как того требует соответствующий стандарт.
- Эталонные реализации: `RPoem` (`open_runo.admin.v1.Admin`) и `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Примечание: соответствует ли код конкретного репозитория этой политике, зависит от самого репозитория. Данный раздел лишь фиксирует политику и не означает, что код уже изменён.

## API 方針の補足: 用語の整理と使い分け (2026-11-04)

### 日本語
**1つに統一するものではなく、別の層の話です。** 「REST の代わり」になるのは Connect・gRPC・GraphQL だけで、WASM は画面を作る技術です。

| 用語 | 何か | REST の代わりか |
|---|---|---|
| REST | URL と HTTP メソッドで操作を表す API の作り方 | 全廃する側 |
| **Connect** | `POST /サービス名/メソッド名` に JSON を送る形式。`curl` でもブラウザの `fetch` でも呼べる | はい(**採用**) |
| **gRPC** | 同じ「サービス/メソッド」の考え方を、バイナリ(protobuf)と HTTP/2 で行う形式。速く、ストリーミングも得意 | はい(**サービス同士に採用**) |
| gRPC-web | gRPC のブラウザ向けの変形版 | はい(必要なら) |
| GraphQL | 欲しい項目を問い合わせる形式 | 一部だけ(**問い合わせ(読み取り)専用**) |
| WASM | Rust をブラウザの中で動かす仕組み | いいえ(画面を作る技術) |

- **ブラウザの画面や `curl` から呼ぶ API は Connect**(POST、JSON を RS-JSON で読み書き)。ブラウザの WASM は素の gRPC を話せず、Connect なら `fetch` のままで、`prost` を足さずに済む(画面のサイズも増えない)。
- **サービス同士の通信(サーバー間、重い処理、ストリーミング)は gRPC。** 多くは Connect で足りるので、ストリーミングが必要になった所だけ gRPC に上げる。
- **GraphQL は、まとめて問い合わせたいときの読み取り専用。** 書き込み(操作)には使わない。
- **WASM は画面を作る技術として、そのまま使う。** 画面から呼ぶ API が Connect になっただけ。
- 外部の規格が形を決めるもの(ACME、OAuth のコールバック、第三者の Webhook、ヘルスチェック、静的ファイル、OTLP)は、規格どおりの HTTP のまま。
- **迷ったら Connect。**

### English
**These are not alternatives to unify; they sit at different layers.** Only Connect, gRPC and GraphQL can replace REST; WASM is a way to build the UI.

| Term | What it is | Replaces REST? |
|---|---|---|
| REST | API style that expresses operations through URLs and HTTP methods | The thing being abolished |
| **Connect** | `POST /Service/Method` with a JSON body; callable from `curl` and from a browser `fetch` | Yes (**adopted**) |
| **gRPC** | The same service/method idea over binary (protobuf) and HTTP/2; fast, good at streaming | Yes (**for service-to-service**) |
| gRPC-web | A browser-friendly variant of gRPC | Yes (if needed) |
| GraphQL | A query language: ask for exactly the fields you want | Partly (**read-only queries**) |
| WASM | Runs Rust inside the browser | No (a UI technology) |

- **APIs called from browser screens or `curl` use Connect** (POST, JSON read and written with RS-JSON). Browser WASM cannot speak plain gRPC; with Connect it stays a `fetch`, and `prost` is not needed (the binary does not grow).
- **Service-to-service calls (server to server, heavy work, streaming) use gRPC.** Connect is enough in most places; move a call to gRPC only where streaming is actually needed.
- **GraphQL is read-only**, for queries that gather several pieces of data. It is not used for writes (operations).
- **WASM stays as the way to build the UI.** The API it calls simply became Connect.
- Interfaces whose shape an outside standard dictates (ACME, OAuth callbacks, third-party webhooks, health checks, static files, OTLP) stay plain HTTP.
- **When in doubt, use Connect.**

### Deutsch (KI-Übersetzung, sprachlich überarbeitet; noch nicht von Muttersprachlern geprüft)
**Es geht nicht darum, eine einzige Technik auszuwählen – die Begriffe liegen auf unterschiedlichen Ebenen.** Als Ersatz für REST kommen nur Connect, gRPC und GraphQL in Frage; WASM dagegen ist eine Technik, mit der die Benutzeroberfläche gebaut wird.

| Begriff | Was ist das? | Ersatz für REST? |
|---|---|---|
| REST | Ein API-Stil, bei dem Operationen über URLs und HTTP-Methoden ausgedrückt werden | wird abgeschafft |
| **Connect** | `POST /Dienst/Methode` mit JSON im Body; aufrufbar mit `curl` ebenso wie mit `fetch` im Browser | ja (**gewählt**) |
| **gRPC** | Dasselbe Prinzip aus Dienst und Methode, aber über binäres protobuf und HTTP/2; schnell und gut für Streaming geeignet | ja (**zwischen Diensten**) |
| gRPC-web | Eine für den Browser angepasste Variante von gRPC | ja (bei Bedarf) |
| GraphQL | Abfragesprache, mit der man genau die benötigten Felder anfordert | teilweise (**nur für lesende Abfragen**) |
| WASM | Führt Rust-Code direkt im Browser aus | nein (Technik für die Oberfläche) |

- **APIs, die von Browseroberflächen oder per `curl` aufgerufen werden, verwenden Connect** (POST; JSON wird mit RS-JSON gelesen und geschrieben). WASM im Browser kann kein reines gRPC sprechen. Mit Connect bleibt es bei einem gewöhnlichen `fetch`, und `prost` ist nicht nötig – die WASM-Datei wird dadurch nicht größer.
- **Für die Kommunikation zwischen Diensten (Server zu Server, rechenintensive Aufgaben, Streaming) wird gRPC eingesetzt.** In den meisten Fällen reicht Connect aus; auf gRPC wird nur dort umgestellt, wo Streaming tatsächlich gebraucht wird.
- **GraphQL dient ausschließlich lesenden Abfragen**, wenn mehrere Informationen gebündelt abgefragt werden sollen. Für Schreibzugriffe (Operationen) wird es nicht verwendet.
- **WASM bleibt die Technik für die Oberfläche.** Geändert hat sich lediglich, dass die aufgerufene API nun auf Connect beruht.
- Schnittstellen, deren Form durch einen externen Standard festgelegt ist (ACME, OAuth-Callbacks, Webhooks von Drittanbietern, Health-Checks, statische Dateien, OTLP), bleiben gewöhnliches HTTP.
- **Im Zweifel: Connect.**

### Русский (перевод выполнен с помощью ИИ и литературно отредактирован; носителями языка пока не проверен)
**Это не варианты, из которых нужно выбрать один, а технологии разных уровней.** Заменить REST могут только Connect, gRPC и GraphQL; WASM же — это способ создания пользовательского интерфейса.

| Термин | Что это | Заменяет REST? |
|---|---|---|
| REST | Стиль построения API, при котором операции выражаются через URL и HTTP-методы | отменяется |
| **Connect** | `POST /Сервис/Метод` с телом в формате JSON; вызывается как из `curl`, так и из браузерного `fetch` | да (**выбран**) |
| **gRPC** | Тот же принцип «сервис — метод», но поверх двоичного формата protobuf и HTTP/2; работает быстро и хорошо подходит для потоковой передачи | да (**для связи между сервисами**) |
| gRPC-web | Адаптированный для браузера вариант gRPC | да (при необходимости) |
| GraphQL | Язык запросов: клиент запрашивает ровно те поля, которые ему нужны | частично (**только запросы на чтение**) |
| WASM | Позволяет выполнять код на Rust прямо в браузере | нет (технология для интерфейса) |

- **API, которые вызываются из браузерных интерфейсов или через `curl`, строятся на Connect** (POST; JSON читается и записывается через RS-JSON). Браузерный WASM не умеет работать с «чистым» gRPC. С Connect достаточно обычного `fetch`, а `prost` не требуется — размер WASM-файла не увеличивается.
- **Для обмена между сервисами (сервер — сервер, ресурсоёмкие задачи, потоковая передача) применяется gRPC.** В большинстве случаев достаточно Connect; на gRPC переходят только там, где потоковая передача действительно необходима.
- **GraphQL используется только для чтения**, когда нужно получить сразу несколько связанных данных. Для записи (операций) он не применяется.
- **WASM по-прежнему остаётся технологией для интерфейса.** Изменилось лишь то, что вызываемый им API теперь построен на Connect.
- Интерфейсы, форма которых задана внешним стандартом (ACME, обратные вызовы OAuth, вебхуки сторонних сервисов, проверки работоспособности, статические файлы, OTLP), остаются обычным HTTP.
- **Если есть сомнения — выбирайте Connect.**

## 2026-11-08 本番(VPS)反映: /api/status・/api/create → Connect
- `bb31771` のバイナリをWSL(Ubuntu-24.04, glibc 2.39)でビルドし、VPSの `web/target/release/open-raid-z-web` と入れ替え(旧は `.before-connect-20261108c`)。VPSのクローンの未コミット変更4件は触っていない。
- 実測: Status=200(公開・読み取り専用)、CreatePool 無認証=401、旧 `/api/status`=404、`/healthz`=200、`easy-web.tokyo/open-raid-z/` は白画面にならず状態が表示される。1・2回目は検証条件の誤り(Statusは公開、CreatePoolは本文検証が先)で自動ロールバックし、3回目で確定。
- *Deployed bb31771 binary on the VPS; Status 200 (public), CreatePool unauth 401, old route 404, UI renders.*
