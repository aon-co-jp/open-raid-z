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

### 壁2: switch_root後もFUSEデーモンが生き続ける必要がある(調査済み・認識を修正)

当初「`switch_root`自体がinitramfs内の全プロセスをkillする」と想定して
いたが、調査した結果**この理解は不正確**だった。

- `switch_root`(busybox版/util-linux版どちらも)は、新ルートへの
  `mount --move`(/dev,/proc,/sys,/run)→ 古いinitramfs(tmpfs)の
  再帰的な削除 → `chroot`→新initへの`exec`、を行うだけで、**無関係な
  他プロセスを能動的にkillする処理は含まれていない**([Marcus Folkessonの
  解説記事](https://www.marcusfolkesson.se/blog/changing-the-root-of-your-linux-filesystem/)、
  [Gentoo Forumsのswitch_root議論](https://forums.gentoo.org/viewtopic-t-1159541-view-previous.html?sid=2dc74c0db69a5e4d047d95a8ac833726)参照)。
  したがって、**switch_root前にorzctlのFUSEデーモンを起動しておけば、
  起動そのものはおそらく問題なく生き残る**(実機での検証はまだ)。
- 本当にリスクがあるのは**次回のシャットダウン/再起動時**。システム
  停止シーケンスの`sendsigs`(`killall5`相当)が全プロセスへ終了信号を
  送る際、FUSEデーモンも巻き込まれる。ntfs-3gを実際にrootにする際にも
  同じ問題が報告されており、対処として`killall5 -o omitpid`(指定PIDを
  除外するオプション)が使われているが、**この`omitpid`実装自体が
  バグでハングする不具合報告がある**
  ([Ubuntu sysvinitパッケージのバグ#87763](https://bugs.launchpad.net/ubuntu/+source/sysvinit/+bug/87763))。
- 実際に「FUSE経由のファイルシステムをrootにする」実例として、
  [nikp123/ntfs-rootfs](https://github.com/nikp123/ntfs-rootfs)、
  [CyanoHao/NTFS-as-rootfs](https://github.com/CyanoHao/NTFS-as-rootfs)
  (NTFS-3G、つまりFUSE経由でNTFSをrootにするArch Linux/Manjaro向け
  プロジェクト)が実在する。両者とも「initramfsにFUSEドライバを
  含める」「initramfs-tools/mkinitcpioの標準フックがNTFS等の
  非標準rootを正しく扱えないため個別対応が必要」「シャットダウン時に
  正常にpoweroff/resetできず`halt`止まりになる」という制約を報告して
  おり、**起動は動くが終了処理に既知の問題がある**という状況は
  open-raid-zでも同様に想定しておくべき。

次回調査すべき点(未検証):
- 実際にorzctlのFUSEデーモンPIDをswitch_root前に確保し、switch_root後も
  生存しているかを実機で確認する
- シャットダウン/再起動時にFUSEデーモンへ`ExecStop`(既存の
  `contrib/systemd/open-raid-z-pool.service.example`と同様、明示的な
  `fusermount3 -u`)を発行できるsystemdユニット構成にすることで、
  `killall5 -o omitpid`のバグに依存しない安全な終了処理を設計する
  (systemdが起動している最終ルート環境なら、initramfsのkillall5問題を
  回避できる可能性が高い)

### 壁3: Windows側はさらに困難(カーネルモードドライバが必須)

WinFsp(ユーザーモード)は原理的にWindowsの起動ボリュームには使えない。
Windows起動時にロードされるのは署名済みのカーネルモードファイル
システムドライバのみ。したがってWindows版は「NTFS.sysに相当する
新規カーネルドライバの開発」が前提となり、本メモの対象範囲(initramfs)
とは全く別の作業(WDKでのドライバ開発、テスト署名、`bcdedit
/set testsigning on`が必要なテスト環境構築)になる。

## 【実機検証成功】switch_root機構の実証(2026-07-10)

上記の「壁1」「壁2」で計画していたアプローチを実際に実装・実機検証し、
**RAID-Z2プール上のデータをswitch_rootで実ルートファイルシステムとして
使う、というメカニズム全体が成功することを確認した**。

### 採用した設計(壁1の具体的な解決策)

`mount.rs`/`fuse_mount.rs`が現状フラットな名前空間(サブディレクトリ
非対応)しか扱えないという制約を踏まえ、**「RAID-Zデータセットの中身を
1つのext4イメージファイルにする」**という設計にした:

1. RAID-Z2プール上に1つのデータセット(`rootimg`)を作成し、その中身を
   busybox製の最小限rootfsを詰めたext4イメージファイル(生バイト列)にする。
2. initramfsのカスタムフック(`/usr/share/initramfs-tools/hooks/orzraid`)で
   `orzctl`バイナリと`losetup`をinitrdへ組み込む(`fuse`はこのカーネルでは
   `CONFIG_FUSE_FS=y`でビルトインのため、モジュールのコピーは不要だった)。
3. `local-top`スクリプト(`/usr/share/initramfs-tools/scripts/local-top/orzraid`)が
   起動時に: RAIDメンバーディスク(`/dev/sdb`〜`/dev/sde`)の出現を待つ→
   `orzctl mount`をバックグラウンドで起動しFUSE経由で`rootimg`データセットを
   ファイルとして公開→`losetup /dev/loop0 <そのファイル>`でブロックデバイス化。
4. GRUBのカーネルパラメータで`root=/dev/loop0 rootfstype=ext4`を指定。
   これにより、initramfs-tools標準の`local-premount`(fsck)→
   `mountroot`(ext4マウント)→`local-bottom`→`init-bottom`→`switch_root`という
   **既存の標準フローがそのまま`/dev/loop0`に対して機能した**(カスタムの
   `switch_root`呼び出しコードを自前で書く必要が無かった)。

この設計により、「壁1」で懸念していた`mount.rs`のディレクトリ階層
サポート(未実装)を一切必要とせずに、既存のflatな1ファイル=1データセット
という制約のまま、実用的な形でRAID-Z2をブート起動ディスクの土台にできた。

### 実機検証結果(シリアルコンソールログで確認、`open-raid-z-linux-boot`VM)

`VBoxManage grub-reboot`相当(GRUB側の`grub-reboot`コマンド)で1回限りの
専用メニューエントリから起動し、以下を全て確認:

1. `orzraid: waiting for RAID-Z member disks ... done.`
2. `orzraid: mounting RAID-Z2 pool via orzctl (FUSE) ... done.`
3. `orzraid: attaching rootimg as a loop device ...` →
   `loop0: detected capacity change from 0 to 5300`(想定通り2.7MBのイメージ)
4. `Will now check root file system ... fsck.ext4 -a -C0 /dev/loop0` →
   `/dev/loop0: clean, 33/336 files, 578/662 blocks`
5. `EXT4-fs (loop0): mounted filesystem without journal.` (実際にマウント成功)
6. `switch_root`後、独自の`/init`スクリプトが実行され
   `=== ROOT-ON-RAID-Z MINIMAL ROOTFS: switch_root SUCCEEDED ===`という
   バナーが出力され、BusyBoxシェルのプロンプト(`/ #`)まで到達した。

**「壁2」(switch_root後もFUSEデーモンが生き続ける必要がある)についても、
特別な対処(mount --moveでのFUSEマウントポイント移動等)を一切せずに
成功した**。理由は上記の設計により、switch_root自体は`/dev/loop0`という
「普通のブロックデバイス」に対して行われ、FUSE経由のマウントポイント
(`/orzfs_stage`)自体はswitch_rootの対象外(古いinitramfsのtmpfs上に
残ったまま)だったため。`orzctl mount`のFUSEデーモンプロセスは
initramfsの`/orzfs_stage`をマウントポイントとして持ったまま生き続け、
ロックされたファイル(`/orzfs_stage/rootimg`)への読み書きは引き続き
そのプロセス経由で処理される(ループデバイスが開いているファイル
ディスクリプタ経由でアクセスするため、switch_root後もこの経路は
生きたまま)。**シャットダウン/再起動時の安全な終了処理(壁2で懸念していた
`killall5`問題)は、今回はbusybox最小シェルへのwork起動確認のみで
未検証のまま**(次回以降の課題)。

### 副次的に発見した問題(次回以降の課題)

- **【重要】メタデータ(スーパーブロック)容量の実質的な上限を発見**:
  `Pool::save()`はメタデータを必ず1ストライプ(`num_data_disks × chunk_size`
  バイト)に収める設計だが、`ref_counts: HashMap<u64,u32>`(ストライプごとの
  参照カウント)のエントリ数は実際に割り当てたストライプ数に比例して
  増えるため、**`--stripes`(総ストライプ数)の値に関わらず、実際に
  書き込める総量は「スーパーブロック1ストライプに収まるref_countsの
  エントリ数」で頭打ちになる**(chunk_size=4096・4ディスクZ2構成の場合、
  実測で約530ストライプ=約4.3MBが上限)。READMEの「容量の人為的上限は
  無い、プールの空き容量の範囲内で大きなファイルも保存できる」という
  記述と矛盾する、実質的なスケーラビリティ上のバグ。`--chunk-size`を
  大きくすることで(1ストライプあたりの容量が増え、同じ論理サイズに
  必要なストライプ数=`ref_counts`エントリ数が減るため)回避は可能
  (例: chunk_size=65536なら数十MB程度まで実用上耐えられる)。
- **【要注意】特定のチャンクサイズで書き込み内容が破損する事象を発見**:
  `chunk_size=65536`(RAID-Z2・4ディスク、data_disks=2、1ストライプ=131072
  バイト)で、`cp`によるストリーミング書き込み(FUSE経由)で特定のバイト
  位置(ストライプ境界付近)にゴミバイトが混入する現象を確認
  (`chunk_size=4096`では同じファイルを byte-exact で正しく書き込めることを
  確認済みなので、chunk_size依存の問題と判明。ディスクの残存データが
  原因ではないことも、事前に該当範囲を完全にゼロ埋めした上での再現に
  より確認済み)。FUSEのカーネル書き込みバッファサイズ(典型的には128KB=
  131072バイト)と、たまたま1ストライプのサイズが一致したことが
  引き金になっている可能性がある。`write_unaligned`/
  `write_unaligned_growing`の、複数ストライプにまたがる書き込みパスに
  何らかのオフバイワン系バグがある疑いが強い。**今回の実証実験では
  chunk_size=4096を使うことで回避したが、根本原因の特定・修正は
  未着手**。
- シャットダウン/再起動時のFUSEデーモンの安全な終了処理(壁2の残課題)は
  今回も未検証のまま。
- 今回はbusybox最小rootfsでの検証に留めた。実際のUbuntu本番環境を
  丸ごとRAID-Z上のイメージへ移行する(またはUbuntuインストーラー自体に
  RAID-Zを選択肢として組み込む)には、別途大きな作業が必要。

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
