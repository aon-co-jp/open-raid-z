# open-raid-z(日本語版)

**Rust製の実マウント可能なRAID-Z/Z2/Z3ストレージプール実装**。
ZFS(OpenZFS)の「パリティ分散ストライピング・チェックサム自己修復・
Copy-on-Write・スナップショット/クローン」という設計思想を、
**OpenZFS自体には一切依存せず**Rustで一から実装したものです。
CLIツール`orzctl`でプールを作成し、Windows(WinFsp)・Linux/macOS/Android
(FUSE)へ**実際にマウント**できます。

> [ルートREADME](README.md) / [English](README-English.md) /
> [中文](README-Chinese.md) / [한국어](README-Korea.md) / [Español](README-Spain.md) /
> [Français](README-France.md) / [Deutsch](README-Germany.md) / [Italiano](README-Italy.md) /
> [Русский](README-Russia.md) / [العربية](README-Arabic.md)

## 重要な前提

open-raid-zは**独自のオンディスクフォーマット**(ZFS風のCoW/ストライピング)
であり、実際のZFSのオンディスク構造(uberblock/ZIL等)とは互換性が
ありません。既存ZFS/NTFS/ext4/他社製RAIDから移行する場合は必ず
「①既存フォーマットから読み出し → ②open-raid-zプールへ通常の
ファイルコピー」という手順になります。詳細は [MIGRATION.md](MIGRATION.md)。

## 構成(3クレート + 補助コンポーネント)

| コンポーネント | 役割・現状 |
|---|---|
| `open_raid_z_core` | 中核ライブラリ。RAIDレベル(`Raid0`/`Raid1`/`Raid5`/`Raid6`≡`Z2`/`Z3`。`vdev.rs`の`RaidLevel` enum)、sha2チェックサム、Copy-on-Write、スナップショット/クローン、ACLエミュレーション、FAT32/exFAT相互運用(`foreign_fs` feature、書き込み対応)+ext2/ext4読み取り(同featureに含む、読み取り専用)、実マウント(Windows=WinFsp、Linux/macOS/Android=FUSE)、`orzctl`CLIバイナリを含む |
| `zfs_accel_hlsl` | RAID-Z/Z2/Z3のガロア体(GF)パリティ計算をHLSLシェーダ+D3D12/DirectMLでGPU高速化。`gpu_accel` feature無効時は純Rust CPUフォールバックのみで動作(CI等WinFsp/dxc無し環境向け) |
| `open_runo_installer_core` | ディスク検出・zpool構成助言・プレビューのOS非依存ロジック。Tauri本体が要求しうるedition2024制約に巻き込まれないよう、意図的にTauri非依存の独立クレートとして切り出し済み |
| `open_runo_installer`(Tauri GUI) | 上記installer_coreを使うTauri 2 + TypeScriptデスクトップアプリ。**このエコシステムで唯一Tauriパッケージへ直接依存する箇所**(Web系リポジトリ群のTauri自前再現方針とは別枠) |
| `wdk_driver/orzflt` | Windowsカーネルモードドライバの最小スケルトン(WDF/KMDF 1.35)。ロード/アンロードのみのビルド確認済み、**実ロードテストは隔離VM前提**でまだ実施していない開発初期段階 |
| `third_party/fuser-0.17.0-android-patch` | `fuser`0.17をAndroidのpure-rustビルドに対応させたパッチ済みフォーク。`cargo ndk`によるarm64-v8aクロスコンパイルは成功済みだが実機検証は未実施 |

## `orzctl` コマンドライン

```sh
# Z2(6台構成)でプール作成
orzctl create --level z2 --chunk-size 4096 --stripes 100000 --dataset tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# 実マウント(フォアグラウンドで待機し続ける)
orzctl mount --level z2 --chunk-size 4096 --stripes 100000 --mountpoint /mnt/tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# 既存FAT32/exFATボリュームとの相互運用(移行・疎通確認用)
orzctl foreign ls /dev/sdb1
orzctl foreign cat /dev/sdb1 /DCIM/100ANDRO/IMG_0001.JPG ./IMG_0001.JPG
orzctl foreign --format exfat mount /dev/sdc1 /mnt/old_exfat

# 既存ext2/ext4ボリュームの読み取り(読み取り専用マウントにも対応)
orzctl foreign --format ext4 ls    /dev/sdd1 /home
orzctl foreign --format ext4 mount /dev/sdd1 /mnt/old_ext4
```

Windowsでは`\\.\PhysicalDriveN`形式でディスクを指定します。

## ビルド・テスト(実測)

```sh
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features
```

WinFsp SDK・dxc・Windows SDKいずれも不要な**CPUフォールバック構成**で、
2026-07-11時点の実測値:

| クレート | passed | failed |
|---|---|---|
| `open_raid_z_core`(`--no-default-features`) | 101 | 0 |
| `zfs_accel_hlsl`(`--no-default-features`、CPUフォールバック) | 32 | 0 |
| `open_runo_installer_core` | 30 | 0 |
| **合計** | **163** | **0** |

`default` feature(`winfsp_backend` + `gpu_accel`)を有効にした実マウント・
実GPU計算経路は、Windows実機+WinFsp SDK+dxc環境が必要なため別途確認が
必要です。

## ドキュメント

- [MIGRATION.md](MIGRATION.md) — 既存ZFS/NTFS/ext4/他社製RAIDからの移行手順
- [PORTING.md](PORTING.md) — 他プロジェクトへの導入・お引越し1枚ガイド
- [CLAUDE.md](CLAUDE.md) — 開発ルール・技術スタック(このエコシステムの正本)
- [CHAT_HANDOFF.md](CHAT_HANDOFF.md) — 開発経緯・引き継ぎ記録

## 関連プロジェクト

`open-web-server` を中心に、`poem-cosmo-tauri`/`open-runo`・PostgreSQL・
`aruaru-db`・このリポジトリを組み合わせ、3Dオンラインゲームの課金アイテム・
金融/証券データをネットワーク上で紛失させないための目標アーキテクチャ
(通信層四重化・DB書き込み四重化、2026-07-11改訂)がある。open-raid-zは
このディスク冗長化基盤として関与し、実装するZFS類似のチェックサム/
Copy-on-Write/スナップショット特性はDATABASE(PostgreSQL・aruaru-db)の
読み書き信頼性とも実務上の関連性がある(詳細・出典は
[open-web-server](https://github.com/aon-co-jp/open-web-server)の
`README.md`/`CLAUDE.md`参照)。

## ライセンス

MPL-2.0。
