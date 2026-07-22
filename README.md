# open-raid-z

**Rust製の実マウント可能なRAID-Z/Z2/Z3ストレージプール実装**
(ZFS「風」のCoW/チェックサム/スナップショット。ZFS自体・OpenZFSへの
依存やオンディスク互換性はなし。純Rust + オプションでGPU高速化)。

[![License](https://img.shields.io/badge/license-MPL--2.0-blue)](open_runo_zfs_source/open_raid_z_core/Cargo.toml)
![Tests](https://img.shields.io/badge/tests-166%20passed-brightgreen)

📖 詳細: [日本語 README](README-Japan.md) / [English README](README-English.md) /
[中文](README-Chinese.md) / [한국어](README-Korea.md) / [Español](README-Spain.md) /
[Français](README-France.md) / [Deutsch](README-Germany.md) / [Italiano](README-Italy.md) /
[Русский](README-Russia.md) / [العربية](README-Arabic.md) —
他プロジェクトへの導入は **[PORTING.md](PORTING.md)** 1枚で完結します。

> 旧来の10ヶ国語版は [`README/`](README/README-Japan.md) フォルダにも
> 残っていますが(言語セットが異なる: UK/US English・Ukraine・
> Iran(Persian)を含む旧版)、上記のルート直下 `README-<言語>.md` が
> 姉妹リポジトリ(`poem-cosmo-tauri`/`open-runo`)と同じ命名規則・
> 言語セット(日英中韓西仏独伊露亜)を採用した現行版です。

---

## open-raid-z とは

ZFS(OpenZFS)が備える**RAID-Z/Z2/Z3(パリティ分散ストライピング)・
チェックサムによる自己修復・Copy-on-Write・スナップショット/クローン**
という設計思想を、**OpenZFS自体には一切依存せず**Rustで一から実装した
ストレージプールです。CLIツール`orzctl`でプールを作成し、
**実際にOSへマウントできます**(Windows: WinFsp / Linux・macOS・Android:
FUSE)。

**重要な前提**: open-raid-zは独自のオンディスクフォーマットであり、
実際のZFS(uberblock/ZIL等)とはオンディスク互換性がありません。
既存ZFS/NTFS/ext4/他社製RAIDからの移行は必ず「読み出し→通常ファイル
コピー」というコピーベースになります(詳細は [MIGRATION.md](MIGRATION.md))。

## ワークスペース構成(3クレート + 補助コンポーネント)

| クレート/ディレクトリ | 役割 |
|---|---|
| `open_runo_zfs_source/open_raid_z_core` | 中核ライブラリ: RAIDレベル(Raid0/Raid1/Raid5/Raid6/Z2/Z3)・チェックサム(sha2)・CoW・スナップショット/クローン・ACLエミュレーション・FAT32/exFAT読み書き+ext2/ext4読み取り相互運用(`foreign_fs`)・実マウント(WinFsp/FUSE)・`orzctl`バイナリ |
| `open_runo_zfs_source/zfs_accel_hlsl` | RAID-Z/Z2/Z3のガロア体(GF)パリティ計算をD3D12/DirectML/HLSLシェーダでGPU高速化するクレート(`gpu_accel` feature、無効時はCPUフォールバックのみで動作) |
| `open_runo_zfs_source/open_runo_installer_core` | ディスク検出・zpool構成助言・プレビューのOS非依存ロジック(Tauri非依存の独立クレート。Tauri本体のedition2024要求に巻き込まれず`cargo test`できるよう意図的に分離) |
| `open_runo_zfs_source/open_runo_installer` | 上記`installer_core`を利用するTauri 2 + TypeScriptデスクトップGUI(**エコシステム内で唯一Tauriへ直接依存する箇所**。Web系リポジトリ群がTauriを自前再実装する方針とは別に、この単独インストーラGUIはTauriパッケージをそのまま使う) |
| `open_runo_zfs_source/wdk_driver/orzflt` | Windowsカーネルモードドライバの最小スケルトン(WDF、KMDF 1.35。ロード/アンロードのみ確認、実I/Oは未実装。**実ロードテストは隔離VM内でのみ実施する方針**、開発初期段階) |
| `open_runo_zfs_source/third_party/fuser-0.17.0-android-patch` | `fuser`0.17へAndroidターゲット向けpure-rustビルドを許可するパッチ済みフォーク(`cargo ndk`でarm64-v8aクロスコンパイル済み、実機検証は未実施) |

## `orzctl` コマンドライン

```sh
# プール作成(Z2、6台構成)
orzctl create --level z2 --chunk-size 4096 --stripes 100000 --dataset tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# 実マウント(フォアグラウンドで待機)
orzctl mount --level z2 --chunk-size 4096 --stripes 100000 --mountpoint /mnt/tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# 既存FAT32/exFATボリュームの読み書き(既存USBメモリ/SDカード等)
orzctl foreign ls /dev/sdb1
orzctl foreign --format exfat cat /dev/sdc1 /video.mp4 ./video.mp4

# 既存ext2/ext4ボリュームの読み取り(読み取り専用、2026-07-20追加)
orzctl foreign --format ext4 ls  /dev/sdd1 /home
orzctl foreign --format ext4 cat /dev/sdd1 /etc/hostname
```

対応RAIDレベル: `Raid0` / `Raid1`(ミラー) / `Raid5` / `Raid6`(`Z2`と同義) /
`Z2` / `Z3`(いずれも`RaidLevel` enum、`vdev.rs`参照)。RAID10は別途
`raid10.rs`のミラーグループ束ねとして提供。

## ビルド・テスト

```sh
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features   # WinFsp SDK・dxc・Windows SDK不要のCPUフォールバック構成
```

3クレート合計 **166テストがpass(failed 0)** — 内訳:
`open_raid_z_core` 104・`zfs_accel_hlsl`(CPUフォールバック) 32・
`open_runo_installer_core` 30(2026-07-20実測)。`--features foreign_fs`を
加えるとext2/ext4読み取りブリッジの統合テストが加わり、Windows実測で
112テスト、Linux(WSL2、`fuse_backend,foreign_fs`)で115テストになる。
`default`feature(WinFsp実マウント+GPU高速化)はWindows実機+WinFsp SDK+dxcが
必要なため別途確認が必要。

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

**2026-07-23追記(RPoem≈Tomcat、open-web-server≈Apache+Nginx
ハイブリッド)**: `open-web-server`(Apache+Nginxハイブリッド役)と
`RPoem`(Tomcat役、アプリケーションサーバー層)の通信・DB連携で、
[RS-SmartTCP](https://github.com/aon-co-jp/RS-SmartTCP)
(IOWN/APN×Smart-TCPの良いとこ取り適応制御、arXiv 2512.00491に着想を
得た独自実装)・`open-web-server-wire::accel`(圧縮+暗号化のCPU/GPU/NPU
ハードウェアアクセラレータ抽象化、CPUのみ実装済み)・HTAP列キャッシュ
(TiDB/TiFlash方式の行→列インクリメンタル同期、`aruaru-db`)・
Multi-Raft(CockroachDB/TiKV方式のRange単位独立合意グループ、
`aruaru-db`)・UDP-IP冗長経路の受信側(`RPoem`)を新規実装した。詳細は
各リポジトリのCLAUDE.md HANDOFF(2026-07-23付近)を参照。

## ライセンス

MPL-2.0(`open_raid_z_core`/`zfs_accel_hlsl`/`open_runo_installer_core`の
`Cargo.toml`参照)。
