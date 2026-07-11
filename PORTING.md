# PORTING.md — open-raid-z お引越しファイル

> このファイル1枚で、他プロジェクト/他マシンへ open-raid-z を
> 導入・移設できます。
>
> 対象バージョン: `open_raid_z_core` 0.0.1 / `zfs_accel_hlsl` /
> `open_runo_installer_core` 0.1.0(3クレート・163テスト、
> `--no-default-features`のCPUフォールバック構成での実測値)
> 最終更新: 2026-07-11

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
| 既存フォーマット連携 | FAT32/exFAT の読み書き相互運用(`foreign_fs` feature、純Rust実装でネイティブライブラリ不要) |
| GPU高速化(任意) | RAID-Z/Z2/Z3のガロア体パリティ計算をHLSL+D3D12/DirectMLでGPUオフロード(`gpu_accel` feature) |
| インストーラGUI | ディスク検出・zpool構成助言(`installer_core`、OS非依存ロジック) + Tauri 2 GUI(`open_runo_installer`) |

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
通れば移設成功(163テスト、下記4節参照)。ライブラリとして使う場合は
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
- `foreign_fs`: FAT32/exFAT読み書き(全OSで有効化可、ネイティブライブラリ不要)

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

### 4.3 既存FAT32/exFATボリュームとの相互運用だけを使う

```toml
open_raid_z_core = { path = "...", default-features = false, features = ["foreign_fs"] }
```

```sh
orzctl foreign ls /dev/sdb1
orzctl foreign --format exfat mount /dev/sdc1 /mnt/old_exfat
```

`foreign_fs`は`fatfs`/`fscommon`/`hadris-fat`という純Rustクレートのみに
依存し、追加のネイティブライブラリを要しません。

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

## 5. 動作確認

```sh
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features   # 101テスト(2026-07-11実測)

cd ../zfs_accel_hlsl
cargo test --no-default-features   # 32テスト(CPUフォールバック)

cd ../open_runo_installer_core
cargo test                          # 30テスト
```

3クレート合計 **163テストpassed、failed 0**(2026-07-11実測、
WinFsp SDK/dxc/Windows SDK不要の構成)。`default`feature(実マウント+
GPU高速化)を有効にした構成はWindows実機+WinFsp SDK+dxcが必要なため
別途確認してください。

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
