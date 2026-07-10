# orzflt — open-raid-z 起動ドライバ(WDK, 開発初期段階)

Windowsを RAID-Z プール上から起動できるようにするという長期目標
(`CHAT_HANDOFF.md` 追記1〜8、追記30に隣接する追記31参照)へ向けた、
最初のスケルトン。現時点では **実I/Oを一切行わない**、KMDFドライバ
オブジェクトのロード/アンロードのみを確認するための最小構成。

## 現在のスコープ

- `orzflt/driver.c`: `DriverEntry` → `WdfDriverCreate` → (AddDeviceで)
  `WdfDeviceCreate` するだけの、素の制御デバイス。
- `orzflt/orzflt.inf`: テスト署名専用の最小INF(`Root\Orzflt` 上に
  1台の疑似デバイスとして登録する体裁)。

## ビルド方法(このホストで確認済み)

前提: `winget install Microsoft.VisualStudio.2022.BuildTools`
(C++ Build Tools)+ `winget install Microsoft.WindowsWDK.10.0.26100`
が導入済みであること。

```
wdk_driver\build.bat
```

`cl.exe`/`link.exe` を直接呼び出し、KMDFのみをリンクした
`orzflt\orzflt.sys` を生成する(VS用WDK拡張(VSIX)は未導入のため、
vcxprojではなくコマンドラインビルドを採用)。

## 未着手・次のステップ(重要: 隔離VM前提)

カーネルドライバのロードはバグがあるとブート不能・BSODに直結するため、
**このホストでは絶対にロードテストを行わない**。次回以降は:

1. 隔離されたWindows VM(本番機とは別)を用意する。
2. テスト署名モード(`bcdedit /set testsigning on`)を有効化する。
3. 自己署名テスト証明書で`orzflt.sys`/`orzflt.inf`に署名し、
   `pnputil`でインストール、ロードを確認する。
4. ロード確認が取れたら、段階的に実際のRAID-Z読み書きロジックを
   カーネル空間へ移植していく(まずはブート専用の読み取り専用実装から)。
