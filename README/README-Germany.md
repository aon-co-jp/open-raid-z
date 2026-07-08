# open-raid-z

Ein experimentelles Dateisystemprojekt für Windows, das weitgehend kompatibel mit NTFS/exFAT bleibt und ZFS-ähnliche Funktionen (selbstheilende Prüfsummen, Speicherpools, Copy-on-Write, Snapshots/Klone) zusammen mit RAID0/1/5/6/10/Z2/Z3 bietet.

Sprache: [日本語](README-Japan.md) | [UK English](README-UK-English.md) | [US English](README-US-English.md) | [Italiano](README-Italy.md) | [Français](README-France.md) | **Deutsch** | [Русский](README-Russia.md) | [Українська](README-Ukraine.md) | [العربية](README-Arabic.md) | [فارسی](<README-Iran(Persian).md>)

## Namenskonvention

Von diesem Projekt selbst definierte Bezeichner – Verzeichnisnamen, Crate-Namen, npm-Paketnamen, Cargo-Feature-Namen, HTML/CSS-IDs/Klassen usw. – verwenden einheitlich **den Unterstrich (`_`) anstelle des Bindestrichs (`-`)** (z. B. `open_zfs_winfsp_bridge`, `zfs_accel_hlsl`, `open_runo_installer`, `open_runo_installer_core` sowie die Cargo-Features `winfsp_backend`/`gpu_accel`). Zuvor mit Bindestrichen geschriebene Namen wie `openzfs-winfsp-bridge` wurden aus Gründen der Konsistenz innerhalb des Projekts umbenannt.

Folgendes ist davon ausgenommen, da es externen Spezifikationen oder Ökosystem-Konventionen folgt und nicht der eigenen Namenswahl dieses Projekts:

- Der Name des Repositorys selbst (`open-raid-z`; dies ist der tatsächliche GitHub-Repository-Name und kann nicht geändert werden)
- HTML5-`data-*`-Attribute (`data-i18n`; der Bindestrich ist durch die Spezifikation vorgeschrieben)
- Externe npm-Paketnamen (z. B. `@tauri-apps/api`, die tatsächlich veröffentlichten Paketnamen)
- CSS-Eigenschaftsnamen (z. B. `font-family`; dies ist die Syntax der CSS-Sprache selbst)
- Englische zusammengesetzte Begriffe, die tatsächlich einen Bindestrich enthalten, wie Reed-Solomon oder Copy-on-Write

## Komponenten

| Komponente | Rolle |
|---|---|
| `open_zfs_winfsp_bridge` | RAID-Z-/RAID0-10-vdevs, Speicherpool, NTFS-ACL-/exFAT-Attribut-Kompatibilitätsschicht, echtes WinFsp-Mounting |
| `zfs_accel_hlsl` | Auslagerung der Paritätsberechnung auf GPU-/NPU-Hardwarebeschleuniger (DirectX 12 Compute + DirectML) |
| `open_runo_installer_core` | Betriebssystemunabhängige Logik für Festplattenerkennung, den Copilot-artigen Konfigurationsberater und die zpool-Init-Vorschau (keine Tauri-Abhängigkeit; `cargo test` funktioniert auch unter Linux/macOS) |
| `open_runo_installer` | Der eigentliche Tauri-Installer (eine dünne UI-Schicht, die `open_runo_installer_core` aufruft): Hardwareerkennung, zpool-Init-Assistent, Copilot-artige Konfigurationsberater-Oberfläche |

## Hauptfunktionen

- **Vollständige RAID-Reihe**: RAID0 / RAID1 (Spiegelung) / RAID5 / RAID6 / RAID10 (gestreifte Spiegel) / RAID-Z2 / RAID-Z3
- **Festplattenpartitionierung und -wiederverwendung**: eine physische Festplatte teilen und eine Hälfte als Spiegelmitglied nutzen, während die andere Hälfte einem separaten RAID6-/Z2-Array beitritt
- **Selbstheilende Prüfsummen, Copy-on-Write, Snapshots/Klone**: emulieren den ZFS-Ansatz. `Pool::scrub` kann stille Beschädigungen im gesamten Pool in einem Durchgang erkennen und reparieren, über dieselbe API sowohl bei RAID-Z-Familien- als auch bei RAID10-Backends
- **NTFS-Kompatibilität**: ACL-Übersetzung (NFSv4 ⇔ NTFS), UID/GID-⇔-SID-Zuordnung (deterministische RID-basierte Zuordnung für lokale SAM-/AD-Domänen)
- **exFAT-Kompatibilität**: Konvertierung von Dateiattributen und Zeitstempeln, Unterstützung für Dateien/Volumes über 4 GB
- **GPU-/NPU-Hardwarebeschleunigung**: Die RAID-Z1/Z2-Paritätsberechnung wird tatsächlich über DirectX 12 Compute + DirectML ausgeführt (automatischer Rückfall auf die CPU, wenn keine Hardware vorhanden ist)
- **Copilot-artiger Konfigurationsberater**: empfiehlt eine RAID-Stufe basierend auf Festplattenlayout, Beschleuniger und CPU-Kernanzahl (erster heuristischer Ansatz; ein Grundgerüst zur Erkennung lokaler LLMs ist ebenfalls vorhanden). Die Logik liegt in `open_runo_installer_core`, unabhängig von Tauri, und kann auch unter Linux/macOS mit `cargo test` überprüft werden
- **Echtes WinFsp-Mounting (Prototyp)**: kann tatsächlich als Windows-Laufwerksbuchstabe eingebunden werden. Jedes Dataset im Pool erscheint als eigene Datei, mit Unterstützung für beliebige Byte-Offsets/-Längen bei Lese- und Schreibvorgängen (Verzeichnishierarchien sowie Erstellen/Löschen/Umbenennen werden noch nicht unterstützt – es bleibt ein flacher Namensraum)
- **Mehrsprachige Unterstützung**: Der Installer verwendet standardmäßig Japanisch mit einem Sprachumschalter in der Benutzeroberfläche, der auch nach der Installation geändert werden kann

## Aktuelle Einschränkungen (Prototypstadium)

- Das WinFsp-Mounting unterstützt nur einen flachen Namensraum (jedes Dataset im Pool erscheint als eine Datei im Stammverzeichnis). Noch keine Verzeichnishierarchie oder dateibezogenes Erstellen/Löschen/Umbenennen.
- Lese-/Schreibvorgänge laufen über `Pool::read_unaligned`/`Pool::write_unaligned` (eine Read-Modify-Write-Schicht), sodass beliebige Byte-Offsets und -Längen unterstützt werden. Anfragen, die die zugewiesene Kapazität eines Datasets überschreiten (festgelegt über `grow_dataset`), schlagen weiterhin fehl (es gibt keine implizite automatische Erweiterung).
- `Pool` unterstützt sowohl `RaidZVdev` als auch `Raid10Vdev`, aber die Integration von RAID10 in die Datensatz-API ist an manchen Stellen noch oberflächlich.
- Der Code für das echte WinFsp-Mounting (`mount.rs`) kann nicht mit einer Rust-Toolchain vor Version 1.85 gebaut werden, da das `winfsp`-Crate das Cargo-Feature `edition2024` erfordert (siehe Build & Test unten).
- `mount.rs` und die GPU-Implementierung von `zfs_accel_hlsl` (Feature `gpu`) hängen vom `windows`-Crate ab, dessen Inhalt vollständig leer ist, sofern das Kompilierungsziel nicht tatsächlich Windows ist. Dieser Code kann daher nur auf einer echten Windows-Maschine (oder bei Cross-Kompilierung für ein Windows-Ziel) gebaut und getestet werden; unter Linux/macOS lässt er sich nur bauen, wenn diese Features über `--no-default-features` deaktiviert werden.

## Build & Test

```powershell
cd open_runo_zfs_source/open_zfs_winfsp_bridge
cargo test --no-default-features   # ohne WinFsp-Mounting/GPU-Beschleunigung (reine CPU-Logik; weder dxc noch das WinFsp-SDK werden benötigt)
cargo test                         # Standard (inklusive echtem WinFsp-Mounting und GPU-/NPU-Beschleunigung; benötigt WinFsp + dxc)
```

`--no-default-features` deaktiviert sowohl das `winfsp_backend`- als auch das `gpu_accel`-Feature und ermöglicht es, die Kernlogik – RAID0/1/5/6/10/Z2/Z3, selbstheilende Prüfsummen, CoW, Snapshots/Klone, Resilver usw. – betriebssystemunabhängig zu überprüfen (funktioniert auch unter Linux/macOS). Weder WinFsp noch der DirectX Shader Compiler (dxc) noch GPU-/NPU-Hardware werden benötigt.

Der Build mit den Standard-Features (`winfsp_backend` + `gpu_accel`) erfordert:

- Die WinFsp-Laufzeitumgebung (https://winfsp.dev/), installiert auf dem System (die zur Build-Zeit verwendeten SDK-Header werden automatisch mitgeliefert, eine separate Installation der Entwicklerkomponente ist daher nicht erforderlich).
- `dxc` (den DirectX Shader Compiler, im Windows SDK oder im Vulkan SDK enthalten) im `PATH` (wird zum Kompilieren der RAID-Z/Z2-Paritäts-HLSL-Shader zur Build-Zeit verwendet).
- **Rust 1.85 oder neuer** (die Version, in der `edition2024`, benötigt vom `winfsp`-Crate, stabilisiert wurde; ältere Toolchains scheitern bereits beim Parsen des `Cargo.toml`-Manifests).

WinFsp oder dxc können auch einzeln deaktiviert werden (z. B. `--no-default-features --features gpu_accel` für nur GPU, ohne WinFsp).

### Installer (`open_runo_installer` / `open_runo_installer_core`)

```powershell
# Logikschicht (keine Tauri-Abhängigkeit; läuft auch unter Linux/macOS)
cd open_runo_zfs_source/open_runo_installer_core
cargo test                    # nur CPU-Fallback (Standard)
cargo test --features gpu     # inklusive echtem GPU-/NPU-Dispatch (benötigt eine echte Windows-Maschine + dxc)

# Frontend (TypeScript, betriebssystemunabhängig)
cd open_runo_zfs_source/open_runo_installer
npm install
npx tsc --noEmit               # nur Typüberprüfung
npx vite build                 # tatsächlicher Build

# Die Tauri-App selbst (benötigt eine echte Windows-Maschine oder ein ausreichend aktuelles Rust plus Linux-Desktop-Abhängigkeiten)
cd open_runo_zfs_source/open_runo_installer/src-tauri
cargo tauri dev / cargo tauri build
```

`open_runo_installer_core` (Festplattenerkennung, Copilot-artiger Konfigurationsberater, zpool-Init-Vorschau) ist ein eigenständiges Crate ohne Tauri-Abhängigkeit, sodass seine Logik auch in Umgebungen überprüft werden kann, in denen das für Tauri selbst benötigte Build-Umfeld (eine WebView, GTK usw. sowie eine ausreichend aktuelle Rust-Toolchain) fehlt. Nur die eigentliche Festplattenaufzählung (`\\.\PhysicalDriveN`) verwendet eine reine Windows-API und ist hinter `#[cfg(windows)]` isoliert; alles Übrige (Konfigurationsberater und zpool-Vorschau-Berechnungen) ist betriebssystemunabhängig, und alle 26 zugehörigen Tests bestehen nachweislich.

## Lizenz

MPL-2.0
