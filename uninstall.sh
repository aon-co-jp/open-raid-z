#!/bin/sh
# open-raid-z (orzctl) アンインストールスクリプト(FUSEを使う主要Linux
# ディストリ共通、install.shと対になる新規スクリプト、2026-07-30追記)。
#
# **安全性の設計方針**: このスクリプトが削除するのは`install.sh`が配置した
# `orzctl`バイナリのみ。プールのデータそのもの(RAID Z2/Z3を構成する
# 実ディスク上のデータ)はインストール先ディレクトリの外側(ユーザーが
# `orzctl create`/`orzctl mount`で指定した実デバイス)に存在するため、
# このスクリプトは一切触れない。ただし、アンインストール実行時点で
# `orzctl`経由でマウント中のプールが存在すると、バイナリ削除後は
# アンマウント操作自体ができなくなり得るため、既知のマウントポイントが
# 残っていないか`/proc/mounts`をチェックし、見つかった場合は削除前に
# 警告して確認を求める(データそのものは削除しない、あくまで
# 「マウント中に消すと不便になる」という利便性の警告)。

set -eu

INSTALL_DIR="/usr/local/bin"
BIN_PATH="${INSTALL_DIR}/orzctl"

if [ "$(id -u)" -ne 0 ]; then
    echo "root権限で実行してください(例: sudo ./uninstall.sh)" >&2
    exit 1
fi

if [ ! -f "$BIN_PATH" ]; then
    echo "==> ${BIN_PATH} が見つかりません(既にアンインストール済み、またはインストールされていません)"
    exit 0
fi

# fuse.orzctl的なマウントが残っていないか確認(orzctl自体は独自の
# ファイルシステムタイプ名を名乗っている可能性があるため、汎用的に
# "fuse"を含む行のうちorzctlに関連しそうなものを警告として提示する。
# 判定を誤って処理を止めることのないよう、ここでは削除の可否判断には
# 使わず、単なる注意喚起に留める)。
if [ -r /proc/mounts ] && grep -qi "orzctl\|fuse\.orz" /proc/mounts 2>/dev/null; then
    echo "警告: orzctl関連と思われるマウントが検出されました。" >&2
    echo "      アンインストール前に 'orzctl unmount' 等で明示的にアンマウントすることを推奨します。" >&2
    echo "      (このスクリプトはプールのデータには一切触れません。バイナリのみ削除します)" >&2
    printf "続行しますか? [y/N]: "
    read -r answer
    case "$answer" in
        y|Y) : ;;
        *) echo "中断しました。データは変更されていません。"; exit 1 ;;
    esac
fi

echo "==> ${BIN_PATH} を削除"
rm -f "$BIN_PATH"

echo "==> 完了。"
echo "    プールのデータ(実ディスク上のRAID Z2/Z3構成)は一切削除していません。"
echo "    contrib/systemd/open-raid-z-pool.service.example から複製したsystemdユニットが"
echo "    残っている場合は、必要に応じて手動で無効化・削除してください(このスクリプトは"
echo "    プール構成がディスク構成ごとに異なるため自動生成/自動削除の対象にしていません)。"
