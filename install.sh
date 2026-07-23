#!/bin/sh
# open-raid-z (orzctl) インストールスクリプト(FUSEを使う主要Linux
# ディストリ共通)。
#
# **正直な開示**: このスクリプトは`orzctl`バイナリを配置するのみ。
# プールの自動マウント用systemdユニットは、プールごとにディスク構成
# (デバイスパス・RAIDレベル・chunk-size・stripes)が異なるため、
# `open_runo_zfs_source/open_raid_z_core/contrib/systemd/
# open-raid-z-pool.service.example` を実環境に合わせて手動で複製・
# 編集すること(このスクリプトでは自動生成しない)。
#
# 使い方:
#   curl -fsSL https://github.com/aon-co-jp/open-raid-z/releases/latest/download/open-raid-z-linux-x86_64.tar.gz | tar xz
#   sudo ./install.sh

set -eu

BIN_SRC="$(dirname "$0")/orzctl"
INSTALL_DIR="/usr/local/bin"

if [ "$(id -u)" -ne 0 ]; then
    echo "root権限で実行してください(例: sudo ./install.sh)" >&2
    exit 1
fi

if [ ! -f "$BIN_SRC" ]; then
    echo "orzctl バイナリが見つかりません($BIN_SRC)。同梱のtar.gzを展開したディレクトリで実行してください。" >&2
    exit 1
fi

if ! command -v fusermount3 >/dev/null 2>&1 && ! command -v fusermount >/dev/null 2>&1; then
    echo "警告: FUSE(fusermount/fusermount3)が見つかりません。マウント機能を使うには"
    echo "      libfuse3(またはfuse)パッケージを別途インストールしてください。" >&2
fi

echo "==> バイナリを ${INSTALL_DIR}/orzctl へ配置"
install -m 755 "$BIN_SRC" "${INSTALL_DIR}/orzctl"

echo "==> 完了。プール作成例:"
echo "    sudo orzctl create --level z2 --chunk-size 4096 --stripes 1000 /dev/sdb /dev/sdc /dev/sdd /dev/sde"
echo "    sudo orzctl mount  --level z2 --chunk-size 4096 --stripes 1000 --mountpoint /mnt/tank /dev/sdb /dev/sdc /dev/sdd /dev/sde"
echo "    自動マウント設定(systemd)は contrib/systemd/open-raid-z-pool.service.example を参照。"
