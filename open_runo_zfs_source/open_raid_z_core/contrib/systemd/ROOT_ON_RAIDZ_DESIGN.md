# OS自体をRAID-Z上に配置する(起動ディスク化)ための設計メモ

このセッションで実機検証した内容と、その先に必要な作業を記録する。
まだ実装・実機テストしていない部分は明示的に「未検証」と書く。

## 現状(実機検証済み)

`open-raid-z-linux-boot`という名前のVirtualBox VM(Ubuntu Server
24.04.4)で以下を確認済み:

1. `orzctl`で4台の**独立したブロックデバイス**(ループバックファイルでは
   ない)にRAID-Z2プールを作成
2. `contrib/systemd/open-raid-z-pool.service.example`のsystemdユニット
   経由で、**本物の`systemctl reboot`をまたいで**プールが自動マウント
   され、書き込んだデータが正しく残ることを確認(**4回連続の再起動で
   再現性を確認済み**)
3. シリアルコンソール出力を有効化済み(`VBoxManage modifyvm <vm> --uart1
   0x3F8 4 --uartmode1 file <ログパス>`+GRUBの
   `GRUB_CMDLINE_LINUX_DEFAULT="console=tty0 console=ttyS0,115200n8"`)。
   起動シーケンス全体がテキストログとして残るようになり、
   `open-raid-z-tank.service`の起動成功もログで確認できる。
   下記「推奨する進め方」の3番目の項目は完了済み。

つまり「OSは通常のディスク(ext4)、追加のデータボリュームがRAID-Z」という
構成は実証済み。これは`zpool`をデータプールとして使う一般的なZFS運用と
同じ形。

## 次の段階: OS自体をRAID-Z上に置く(未検証)

「WindowsやLinuxをNVMe SSD/HDDにRAIDでインストールできるようにする」
という目標には、**ルートファイルシステム自体がRAID-Zプール上のデータ
セット**である必要がある。これは上記の「データボリューム」構成とは
質的に異なり、以下の技術的な壁がある。

### 壁1: initramfsからorzctl(FUSE)を実行する

Linuxの起動シーケンスは、ブートローダ(GRUB)がカーネル+initramfsを
読み込み、initramfs内の`/init`スクリプトが実ルートファイルシステムを
準備してから`switch_root`する。ルートがRAID-Zプールなら、この
`/init`スクリプトの中で:

1. `udevadm settle`等でRAIDメンバーディスク(`/dev/sdb`等)の出現を待つ
2. `fuse`カーネルモジュールをロードする(`/dev/fuse`が必要)
3. `orzctl mount`相当の処理で、ルートとして使うデータセットを
   `/newroot`(switch_rootの慣習的なマウント先)へマウントする
4. `switch_root /newroot /sbin/init`を実行する

という手順が必要。initramfs-tools標準の`update-initramfs`が生成する
initramfsは通常glibc動的リンクバイナリを含められる(`copy_exec`で
共有ライブラリも一緒にコピーされる)ため、`orzctl`バイナリ自体を
含めることは技術的には可能と考えられるが、**未検証**。

### 壁2: switch_root後もFUSEデーモンが生き続ける必要がある

`switch_root`は通常、initramfs内の全プロセスを`killall5`等で終了させて
から新しいルートへ切り替える。ルートがFUSE経由だと、**orzctlの
FUSEデーモンプロセス自体がkillされてはならない**(killされた瞬間、
ルートファイルシステムへのアクセスが全て失敗しカーネルパニックする)。

対処の方向性(要調査):
- `switch_root`の実装(busybox版/util-linux版)によっては、
  `/proc/self/fd`越しに現在のマウント下のプロセスを保護する仕組みがある
- 実際に「FUSEをrootにする」運用例(一部のコンテナ・クラウド環境)が
  存在するため、先行事例の調査が必要
- 最悪の場合、initramfs側でorzctlを`setsid`+`nohup`相当で確実に
  デタッチし、initramfsのクリーンアップ対象から除外する必要がある

### 壁3: Windows側はさらに困難(カーネルモードドライバが必須)

WinFsp(ユーザーモード)は原理的にWindowsの起動ボリュームには使えない。
Windows起動時にロードされるのは署名済みのカーネルモードファイル
システムドライバのみ。したがってWindows版は「NTFS.sysに相当する
新規カーネルドライバの開発」が前提となり、本メモの対象範囲(initramfs)
とは全く別の作業(WDKでのドライバ開発、テスト署名、`bcdedit
/set testsigning on`が必要なテスト環境構築)になる。

## 推奨する進め方(次回セッション向け)

1. **必ずVMスナップショットを取ってから試す**
   (`VBoxManage snapshot <vm> take <name>`)。initramfs/switch_rootの
   実験は起動不能になるリスクが高い。
2. **既存のGRUBエントリを上書きしない**。新しいエントリを追加し、
   `grub-reboot <エントリ番号>`で1回限りそのエントリを選んで再起動する
   運用にする(デフォルトエントリは常に「正常に起動する既存の構成」の
   ままにしておく)。
3. **シリアルコンソール出力を有効化する**
   (`VBoxManage modifyvm <vm> --uart1 0x3F8 4 --uartmode1 file <ログパス>`、
   カーネルパラメータに`console=ttyS0`を追加)。ヘッドレスVMを
   スクリーンショットで見るより、テキストログの方がinitramfsの
   デバッグには圧倒的に有効。**→ 完了済み**(`open-raid-z-linux-boot`
   VMで設定済み、`F:\ISO\linux-boot-vm\screenshots\serial-console.log`
   へ出力される。4回の再起動でも正常に動作を確認済み)。
4. 最初から本番のUbuntu環境を壊すのではなく、まず**tankプール上に
   別のテスト用データセットを作り、busybox等の最小限のルートスケルトン
   だけを置いて`switch_root`できるかどうかの実験**に限定するとよい
   (実OSの起動を最初から賭けない)。
