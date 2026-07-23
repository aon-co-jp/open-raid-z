# open-raid-z (orzctl) インストールスクリプト(Windows / Windows Server 共通)。
#
# **正直な開示**: WindowsではWinFsp経由でのマウントになるため、事前に
# WinFsp(https://winfsp.dev/)がインストールされている必要がある
# (このスクリプトはWinFsp自体のインストールは行わない)。
#
# 使い方(管理者権限のPowerShellで):
#   Invoke-WebRequest -Uri "https://github.com/aon-co-jp/open-raid-z/releases/latest/download/open-raid-z-windows-x86_64.zip" -OutFile open-raid-z.zip
#   Expand-Archive open-raid-z.zip -DestinationPath open-raid-z
#   cd open-raid-z
#   .\install.ps1

#Requires -RunAsAdministrator

$ErrorActionPreference = "Stop"

$InstallDir = "C:\Program Files\open-raid-z"

Write-Host "==> インストール先: $InstallDir"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$BinSrc = Join-Path $PSScriptRoot "orzctl.exe"
if (-not (Test-Path $BinSrc)) {
    Write-Error "orzctl.exe が見つかりません($BinSrc)。zipを展開したディレクトリで実行してください。"
    exit 1
}
Copy-Item $BinSrc -Destination $InstallDir -Force

$env:Path += ";$InstallDir"
[Environment]::SetEnvironmentVariable("Path", [Environment]::GetEnvironmentVariable("Path", "Machine") + ";$InstallDir", "Machine")

if (-not (Get-Command "winfsp" -ErrorAction SilentlyContinue)) {
    Write-Host "==> 警告: WinFsp が見つかりません。マウント機能を使うには https://winfsp.dev/ から別途インストールしてください。"
}

Write-Host "==> 完了。プール作成例(orzctl.exeのフルパスまたは新しいシェルでPATHが有効になった後):"
Write-Host "    orzctl create --level z2 --chunk-size 4096 --stripes 1000 D: E: F: G:"
Write-Host "    orzctl mount  --level z2 --chunk-size 4096 --stripes 1000 --mountpoint X: D: E: F: G:"
