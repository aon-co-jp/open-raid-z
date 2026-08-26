; open-raid-z Windowsインストーラー(Inno Setup)。
;
; ユーザー指示「パワーシェル版とリポジトリ名-installer.exeの二種類を
; ご用意して」への対応。既存の`install.ps1`(gitクローン+ソースから
; ビルドする方式、Rust未導入ならwingetで自動導入)はそのまま維持し、
; これは第二の選択肢——ビルド済みバイナリを直接インストールする
; GUIインストーラー(Rustツールチェーンのインストール不要、より速い)。
;
; 正直な開示: 挙動の違い——install.ps1は毎回最新のソースをgit clone/
; pullしてローカルでビルドするため常に最新コミットを反映するが、本
; インストーラーはビルド時点のバイナリを固定で配布する(通常のリリース
; バイナリ配布と同じ方式、他のリポジトリ〈aruaru-llm等〉と同一の
; パターンに統一)。管理者権限は不要(CLIツールを%LOCALAPPDATA%へ
; コピーするのみ)。
;
; ビルド方法: リポジトリルートで`cargo build --release --bin orzctl`
; を実行した後、このディレクトリで`ISCC.exe orzctl.iss`を実行する。

#define MyAppName "open-raid-z"
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-local-build"
#endif
#define MyAppPublisher "aon-co-jp"
#define MyAppURL "https://github.com/aon-co-jp/open-raid-z"
#define MyAppExeName "orzctl.exe"

[Setup]
PrivilegesRequired=lowest
AppId={{B2C3D4E5-7777-4A5B-8C6D-2E7F9A0B1C2D}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
OutputDir=dist
OutputBaseFilename=open-raid-z-installer
ArchitecturesInstallIn64BitMode=x64compatible
DisableProgramGroupPage=yes

[Languages]
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "addtopath"; Description: "Add {#MyAppName} to your PATH (current user) / {#MyAppName}をPATHへ追加(現在のユーザー)"; Flags: checkedonce

[Files]
; orzctlはopen_runo_zfs_source/open_raid_z_core(ネストしたcrate)の
; ビルド出力(通常のtarget/release直下ではない)。
Source: "..\..\open_runo_zfs_source\open_raid_z_core\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"

[Registry]
; ユーザー単位のPATHへ追加(管理者権限不要、install.ps1が案内していた
; 手動手順をインストーラー側で自動化しただけ)。
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; \
    ValueData: "{olddata};{app}"; Tasks: addtopath; Check: NeedsAddPath('{app}')

[Code]
function NeedsAddPath(Param: string): boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
  begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Param + ';', ';' + OrigPath + ';') = 0;
end;

[UninstallDelete]
Type: filesandordirs; Name: "{app}"
