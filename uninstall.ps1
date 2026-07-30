# open-raid-z (orzctl) アンインストールスクリプト(Windows / Windows Server
# 共通、install.ps1と対になる新規スクリプト、2026-07-30追記)。
#
# **安全性の設計方針**: install.shの日本語コメントと同じ理由により、
# このスクリプトが削除するのは`install.ps1`が配置した`orzctl.exe`
# バイナリと、それをインストール時に追加したPATH環境変数のエントリのみ。
# プールのデータ(実ドライブ上のRAID Z2/Z3構成)には一切触れない。
#
# 使い方(管理者権限のPowerShellで):
#   cd "C:\Program Files\open-raid-z"
#   .\uninstall.ps1

#Requires -RunAsAdministrator

$ErrorActionPreference = "Stop"

$InstallDir = "C:\Program Files\open-raid-z"
$BinPath = Join-Path $InstallDir "orzctl.exe"

if (-not (Test-Path $BinPath)) {
    Write-Host "==> $BinPath が見つかりません(既にアンインストール済み、またはインストールされていません)"
    exit 0
}

Write-Host "==> $BinPath を削除"
Remove-Item -Path $BinPath -Force

# install.ps1が Machine スコープのPATHへ追加したエントリのみを取り除く
# (他のエントリには触れない、完全一致するエントリのみ除去)。
$machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
$entries = $machinePath -split ";" | Where-Object { $_ -ne "" -and $_ -ne $InstallDir }
[Environment]::SetEnvironmentVariable("Path", ($entries -join ";"), "Machine")

Write-Host "==> 完了。"
Write-Host "    プールのデータ(実ドライブ上のRAID Z2/Z3構成)は一切削除していません。"
Write-Host "    $InstallDir ディレクトリ自体が空になった場合は、必要に応じて手動で削除してください"
Write-Host "    (このスクリプトはバイナリのみを削除し、ディレクトリそのものは残します)。"
