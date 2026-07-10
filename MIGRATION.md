# 既存環境から open-raid-z へのお引越しガイド(データ移行)

このドキュメントは、既存のZFS(実際のOpenZFS)・NTFS・ext4・他社製RAIDから、
`open-raid-z`のプールへデータを移行する際の手順・注意点をまとめたもの。

**前提**: `open-raid-z`は独自のオンディスクフォーマット(ZFS「風」の
CoW/ストライピング実装であり、実際のZFSのオンディスク構造(uberblock/ZIL
等)とは互換性がない)を採用している。そのため、**「既存ディスクをそのまま
`open-raid-z`のプールとして読み込む」ことはできない**。移行は必ず
「①既存フォーマットから読み出し → ②`open-raid-z`プールへコピー」という
コピーベースの手順になる。

## 移行方式の選び方

| 移行元 | 読み出し方法 | 対応状況 |
|---|---|---|
| FAT32/FAT16(USBメモリ・SDカード等) | `orzctl foreign`(本ツール内蔵) | 対応済み |
| exFAT | `orzctl foreign --format exfat`(本ツール内蔵、読み書き両対応) | 対応済み |
| NTFS | OS標準のマウント機能(Windowsならそのままドライブレターとして見える) | OS任せ(`orzctl`は関与しない) |
| ext4 | Linux上でOS標準の`mount`(Windowsからは`orzctl foreign`非対応) | OS任せ |
| 実際のZFS(OpenZFS) | `zfs send`/`zfs receive`または`zpool import`+通常ファイルコピーで**別途OpenZFS環境上に一時展開**してから、そのマウントポイントを通常のファイルコピー元として使う | `open-raid-z`は関与しない(OpenZFS自体は別物) |
| 他社製RAID(mdadm/Storage Spaces等) | RAIDを解除・再構成せず、まずは通常のファイルシステムとしてマウントしたうえで通常のファイルコピー | OS任せ |

いずれの移行元も、**最終的には「読み出せる状態にする」→「`open-raid-z`
プール上へ通常のファイルコピー(`cp -r`/`robocopy`/`rsync`等)を行う」**
という共通の流れになる。`orzctl foreign`はこのうち「FAT32/exFATを
読み出せる状態にする」部分だけを担当する専用機能(それ以外の既存フォーマットは
OS標準のドライバで直接マウントできるため、`orzctl`側で特別な対応は不要)。

## 手順

### 1. 移行先の`open-raid-z`プールを新規作成する

移行先ディスク群のRAIDレベル・チャンクサイズ・総ストライプ数を決めて、
`orzctl create`でプールとデータセットを作成する(移行元データを一切
含まない、空のプール)。

```sh
# Linux(例: Z2、6台構成)
orzctl create --level z2 --chunk-size 4096 --stripes 100000 --dataset tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg
```

```powershell
# Windows(同じコマンド・同じオプション名。ディスクは\\.\PhysicalDriveN形式)
orzctl.exe create --level z2 --chunk-size 4096 --stripes 100000 --dataset tank `
  \\.\PhysicalDrive1 \\.\PhysicalDrive2 \\.\PhysicalDrive3 \\.\PhysicalDrive4 \\.\PhysicalDrive5 \\.\PhysicalDrive6
```

`--stripes`は、移行したい総データ量より十分大きい値を指定すること
(容量が足りないと後述のコピー作業が`CapacityExceeded`で失敗する)。
目安: `(移行したい総バイト数) / (num_data_disks × chunk_size)` に
余裕を持たせた値。

### 2. 作成したプールを実際にマウントする

```sh
# Linux
orzctl mount --level z2 --chunk-size 4096 --stripes 100000 --mountpoint /mnt/tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg
```

```powershell
# Windows
orzctl.exe mount --level z2 --chunk-size 4096 --stripes 100000 --mountpoint Z: `
  \\.\PhysicalDrive1 \\.\PhysicalDrive2 \\.\PhysicalDrive3 \\.\PhysicalDrive4 \\.\PhysicalDrive5 \\.\PhysicalDrive6
```

マウント中はプロセスがフォアグラウンドで待機し続ける。**別のターミナル/
PowerShellウィンドウを開いて次のステップへ進むこと**(Linuxでは
`fusermount3 -u <PATH>`、Windowsでは元のウィンドウでEnterキーを押すまで
アンマウントされない)。

### 3. 移行元データを読み出し可能な状態にする

- **FAT32/exFAT**: `orzctl foreign`をそのまま使う(下記参照)。
- **NTFS/ext4/他社製RAID**: OS標準の方法でマウントするだけでよい
  (`open-raid-z`は関与しない)。
- **実際のZFS(OpenZFS)**: OpenZFSが動く環境(Linux/FreeBSD/実機Mac等)で
  `zpool import`してマウントし、その先を移行元として使う。

### 4. 移行元 → `open-raid-z`プールへ通常のファイルコピー

マウント済みの移行元と、ステップ2でマウントした`open-raid-z`プールの
間で、OS標準のコピーコマンドを使う。`open-raid-z`側はマウント後は
通常のファイルシステムとして見えるため、特別な手順は不要。

```sh
# Linux/macOS例
rsync -avh --progress /mnt/old_volume/ /mnt/tank/
```

```powershell
# Windows例
robocopy D:\ Z:\ /E /Z /MT:8
```

### 5. FAT32/exFATからの読み出し(`orzctl foreign`の具体例)

```sh
# ディレクトリ一覧
orzctl foreign ls /dev/sdb1
orzctl foreign --format exfat ls /dev/sdc1

# 1ファイルだけ取り出す(疎通確認用。大量コピーには不向き)
orzctl foreign cat /dev/sdb1 /DCIM/100ANDRO/IMG_0001.JPG ./IMG_0001.JPG

# Linux/macOS上へ実際にマウントしてから、通常のcp/rsyncで一括コピーする
# (大量ファイルの移行はこちらを推奨。1ファイルずつの`cat`はCLIの疎通確認用)
orzctl foreign --format exfat mount /dev/sdc1 /mnt/old_exfat
rsync -avh --progress /mnt/old_exfat/ /mnt/tank/
```

## 注意点

- **移行中は移行元を読み取り専用として扱うことを強く推奨する**
  (`orzctl foreign put`で書き込むのは、あくまで検証・小規模な用途を
  想定しており、大規模な移行での書き込み利用は非推奨)。
- コピー完了後は、コピー先(`open-raid-z`プール)の内容を移行元と
  突き合わせて検証してから、移行元ディスクの初期化・転用を行うこと。
- `--stripes`(プール容量)は事前に決め打ちする方式であり、後からの
  オンライン拡張には対応していない(容量不足が判明した場合は、
  より大きい`--stripes`で作り直す必要がある)。
- カーネルモード起動ドライバ(`open_runo_zfs_source/wdk_driver/`)は
  開発初期段階であり、**「OS自体を`open-raid-z`プール上から起動する」
  という移行はまだ実現していない**。現時点での移行は「データドライブ
  としての移行」に限られる。

## 関連ドキュメント

- 各国語版README: [`README/`](README/README-Japan.md)
- 開発ルール: [`CLAUDE.md`](CLAUDE.md)
- 開発経緯・引き継ぎ情報: [`CHAT_HANDOFF.md`](CHAT_HANDOFF.md)
- マルチOS対応ロードマップ: [`open_runo_zfs_source/open_raid_z_core/contrib/systemd/MULTIPLATFORM_ROADMAP.md`](open_runo_zfs_source/open_raid_z_core/contrib/systemd/MULTIPLATFORM_ROADMAP.md)
