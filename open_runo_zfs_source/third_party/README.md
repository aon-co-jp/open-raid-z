# third_party/fuser-0.17.0-android-patch

`fuser` 0.17.0(crates.io版)へ、Android(`target_os = "android"`)対応の
パッチを当てたローカルフォーク。`open_raid_z_core/Cargo.toml`の
`fuser_android`依存(android target限定)としてパスで直接参照される。

## 何を・なぜパッチしたか

Androidも`target_os`こそ`"linux"`とは別値だが、実体はLinuxカーネル
そのものであり、`/dev/fuse`のプロトコル・`mount(2)`システムコール・
`renameat2(2)`のフラグ番号は全てLinuxと同一のはず、という前提から
出発した。しかし上流の`fuser` 0.17は:

1. `build.rs`が「pure-rust実装」を許可するOS一覧に`linux`/`freebsd`/
   `dragonfly`/`openbsd`/`netbsd`しか含めておらず、`android`が漏れている。
   → Android向けにビルドすると、代わりにlibfuse2/libfuse3を
   pkg-config経由で要求してしまい、Android NDK環境にはそれらが
   存在しないためビルド不能になる。
2. `src/`以下の複数箇所(実際の`mount()`呼び出し、`renameat2`フラグ、
   ioctl番号の定義等)が`#[cfg(target_os = "linux")]`で直接ゲートされて
   おり、`android`では代替パスが提供されない。

このフォークでは、上記の`target_os = "linux"`ゲートを機械的に
`any(target_os = "linux", target_os = "android")`へ拡張し(意味的には
「Linuxカーネルベースであること」を表しているだけなので、この置換で
挙動が変わることはない)、加えて`src/rename_flags.rs`で発覚した
「bionic libcの`libc`クレートでは`RENAME_*`定数がi32型で提供される
(glibcではu32)」という型差異のみ、`as u32`キャストで吸収した。

## 検証状況

- `cargo ndk -t arm64-v8a check --no-default-features --features
  fuse_backend` … 成功。
- `cargo ndk -t arm64-v8a check --no-default-features --features
  fuse_backend,foreign_fs` … 成功。
- 実機Androidデバイス上での実際のマウント動作(`/dev/fuse`のopen・
  実際のmount(2)呼び出し成功、ファイルI/O)は**未検証**(実機/エミュ
  レータでのroot権限・SELinuxポリシー起因の追加制約が別途あり得る)。

## アップストリームへの提案について

`fuser-android-upstream.patch`(このディレクトリ)に、上流
(https://github.com/zargony/fuser)へ提案する想定の統一diffを
切り出してある。実際のPR送付にはGitHubアカウント経由のフォーク・
プッシュ・PR作成が必要なため、このリポジトリからは未実施
(`CHAT_HANDOFF.md`参照)。
