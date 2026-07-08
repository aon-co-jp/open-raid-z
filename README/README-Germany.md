# open-raid-z

Ein experimentelles Dateisystemprojekt für Windows, das weitgehend kompatibel mit NTFS/exFAT bleibt und ZFS-ähnliche Funktionen (selbstheilende Prüfsummen, Speicherpools, Copy-on-Write, Snapshots/Klone) zusammen mit RAID0/1/5/6/10/Z2/Z3 bietet.

Sprache: [日本語](README-Japan.md) | [UK English](README-UK-English.md) | [US English](README-US-English.md) | [Italiano](README-Italy.md) | [Français](README-France.md) | **Deutsch** | [Русский](README-Russia.md) | [Українська](README-Ukraine.md) | [العربية](README-Arabic.md) | [فارسی](<README-Iran(Persian).md>)

## Komponenten

| Komponente | Rolle |
|---|---|
| `openzfs-winfsp-bridge` | RAID-Z-/RAID0-10-vdevs, Speicherpool, NTFS-ACL-/exFAT-Attribut-Kompatibilitätsschicht, echtes WinFsp-Mounting |
| `zfs-accel-hlsl` | Auslagerung der Paritätsberechnung auf GPU-/NPU-Hardwarebeschleuniger (DirectX 12 Compute + DirectML) |
| `openruno-installer` | Tauri-Installer: Hardwareerkennung, zpool-Init-Assistent, Copilot-artiger Konfigurationsberater |

## Hauptfunktionen

- **Vollständige RAID-Reihe**: RAID0 / RAID1 (Spiegelung) / RAID5 / RAID6 / RAID10 (gestreifte Spiegel) / RAID-Z2 / RAID-Z3
- **Festplattenpartitionierung und -wiederverwendung**: eine physische Festplatte teilen und eine Hälfte als Spiegelmitglied nutzen, während die andere Hälfte einem separaten RAID6-/Z2-Array beitritt
- **Selbstheilende Prüfsummen, Copy-on-Write, Snapshots/Klone**: emulieren den ZFS-Ansatz
- **NTFS-Kompatibilität**: ACL-Übersetzung (NFSv4 ⇔ NTFS), UID/GID-⇔-SID-Zuordnung (deterministische RID-basierte Zuordnung für lokale SAM-/AD-Domänen)
- **exFAT-Kompatibilität**: Konvertierung von Dateiattributen und Zeitstempeln, Unterstützung für Dateien/Volumes über 4 GB
- **GPU-/NPU-Hardwarebeschleunigung**: Die RAID-Z1/Z2-Paritätsberechnung wird tatsächlich über DirectX 12 Compute + DirectML ausgeführt (automatischer Rückfall auf die CPU, wenn keine Hardware vorhanden ist)
- **Copilot-artiger Konfigurationsberater**: empfiehlt eine RAID-Stufe basierend auf Festplattenlayout, Beschleuniger und CPU-Kernanzahl (erster heuristischer Ansatz; ein Grundgerüst zur Erkennung lokaler LLMs ist ebenfalls vorhanden)
- **Echtes WinFsp-Mounting (Prototyp)**: kann tatsächlich als Windows-Laufwerksbuchstabe eingebunden werden (derzeit ein minimaler Build mit einer einzelnen Datei)
- **Mehrsprachige Unterstützung**: Der Installer verwendet standardmäßig Japanisch mit einem Sprachumschalter in der Benutzeroberfläche, der auch nach der Installation geändert werden kann

## Aktuelle Einschränkungen (Prototypstadium)

- Das WinFsp-Mounting unterstützt nur einen flachen Namensraum (eine feste Datei `\pool.dat` im Stammverzeichnis). Noch keine Verzeichnishierarchie oder mehrere Dateien.
- Lese-/Schreibvorgänge müssen an der Chunk-Grenze des Datensatzes ausgerichtet sein.
- `Pool` unterstützt sowohl `RaidZVdev` als auch `Raid10Vdev`, aber die Integration von RAID10 in die Datensatz-API ist an manchen Stellen noch oberflächlich.

## Build & Test

```powershell
cd open-runo-zfs-source/openzfs-winfsp-bridge
cargo test --no-default-features        # ohne WinFsp-Mounting
cargo test --features winfsp-backend    # mit echtem WinFsp-Mounting (benötigt die WinFsp-Laufzeitumgebung)
```

Die WinFsp-Laufzeitumgebung (https://winfsp.dev/) muss auf dem System installiert sein (die zur Build-Zeit verwendeten SDK-Header werden automatisch mitgeliefert, eine separate Installation der Entwicklerkomponente ist daher nicht erforderlich).

## Lizenz

MPL-2.0
