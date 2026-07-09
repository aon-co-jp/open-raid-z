# open-raid-z

Ein experimentelles Dateisystemprojekt für Windows/Linux, das weitgehend kompatibel mit NTFS/exFAT bleibt und ZFS-ähnliche Funktionen (selbstheilende Prüfsummen, Speicherpools, Copy-on-Write, Snapshots/Klone) zusammen mit RAID0/1/5/6/10/Z2/Z3 bietet. Die Kernlogik ist ein einziges betriebssystemunabhängiges gemeinsames Programm (`open_raid_z_core`); die Windows-Variante (WinFsp) und die Linux-Variante (FUSE) unterscheiden sich nur in der dünnen Mount-Schicht darüber (vertrieben unter den Namen `open-raid-z-win`/`open-raid-z-linux`).

Sprache: [日本語](README-Japan.md) | [UK English](README-UK-English.md) | [US English](README-US-English.md) | [Italiano](README-Italy.md) | [Français](README-France.md) | **Deutsch** | [Русский](README-Russia.md) | [Українська](README-Ukraine.md) | [العربية](README-Arabic.md) | [فارسی](<README-Iran(Persian).md>)

## Ein Hinweis an Microsoft und Apple

Wir entwickeln dieses experimentelle Dateisystem, um vollständige ZFS-artige Funktionen (selbstheilende Prüfsummen, RAID6/RAID-Z2, Snapshots und mehr) auf Windows zu bringen. Eines unserer langfristigen Ziele ist es, dass dieses Dateisystem eines Tages als offizielles Installationsziel und Startlaufwerk unter Windows und macOS auswählbar wird.

Uns ist bewusst, dass dies die Mitwirkung der jeweiligen Betriebssystemhersteller erfordert – Signierung/Zertifizierung von Boot-Start-Treibern, offizielle Unterstützung im Installationsprogramm und Ähnliches. Sollten Sie an diesem Vorhaben interessiert sein, würden wir uns über eine Kontaktaufnahme und Zusammenarbeit sehr freuen. Es handelt sich um ein kleines, unabhängiges Projekt, doch wir sind aufrichtig entschlossen, diese Technologie zu verwirklichen.

## Namenskonvention

Von diesem Projekt selbst definierte Bezeichner – Verzeichnisnamen, Crate-Namen, npm-Paketnamen, Cargo-Feature-Namen, HTML/CSS-IDs/Klassen usw. – verwenden einheitlich **den Unterstrich (`_`) anstelle des Bindestrichs (`-`)** (z. B. `open_raid_z_core`, `zfs_accel_hlsl`, `open_runo_installer`, `open_runo_installer_core` sowie die Cargo-Features `winfsp_backend`/`gpu_accel`). Zuvor mit Bindestrichen geschriebene Namen wie `openzfs-winfsp-bridge` wurden aus Gründen der Konsistenz innerhalb des Projekts umbenannt.

Folgendes ist davon ausgenommen, da es externen Spezifikationen oder Ökosystem-Konventionen folgt und nicht der eigenen Namenswahl dieses Projekts:

- Der Name des Repositorys selbst (`open-raid-z`; dies ist der tatsächliche GitHub-Repository-Name und kann nicht geändert werden)
- HTML5-`data-*`-Attribute (`data-i18n`; der Bindestrich ist durch die Spezifikation vorgeschrieben)
- Externe npm-Paketnamen (z. B. `@tauri-apps/api`, die tatsächlich veröffentlichten Paketnamen)
- CSS-Eigenschaftsnamen (z. B. `font-family`; dies ist die Syntax der CSS-Sprache selbst)
- Englische zusammengesetzte Begriffe, die tatsächlich einen Bindestrich enthalten, wie Reed-Solomon oder Copy-on-Write

## Komponenten

| Komponente | Rolle |
|---|---|
| `open_raid_z_core` | RAID-Z-/RAID0-10-vdevs, Speicherpool, NTFS-ACL-/exFAT-Attribut-Kompatibilitätsschicht, echtes Mounting (Windows = WinFsp `mount.rs` / Linux = FUSE `fuse_mount.rs`; alles außer der OS-spezifischen Mount-Schicht ist vollständig gemeinsam genutzt) |
| `zfs_accel_hlsl` | Auslagerung der Paritätsberechnung auf GPU-/NPU-Hardwarebeschleuniger (DirectX 12 Compute + DirectML) |
| `open_runo_installer_core` | Betriebssystemunabhängige Logik für Festplattenerkennung, den Copilot-artigen Konfigurationsberater und die zpool-Init-Vorschau (keine Tauri-Abhängigkeit; `cargo test` funktioniert auch unter Linux/macOS) |
| `open_runo_installer` | Der eigentliche Tauri-Installer (eine dünne UI-Schicht, die `open_runo_installer_core` aufruft): Hardwareerkennung, zpool-Init-Assistent, Copilot-artige Konfigurationsberater-Oberfläche |

## Hauptfunktionen

- **Vollständige RAID-Reihe**: RAID0 / RAID1 (Spiegelung) / RAID5 / RAID6 / RAID10 (gestreifte Spiegel) / RAID-Z2 / RAID-Z3
- **Festplattenpartitionierung und -wiederverwendung**: eine physische Festplatte teilen und eine Hälfte als Spiegelmitglied nutzen, während die andere Hälfte einem separaten RAID6-/Z2-Array beitritt
- **Selbstheilende Prüfsummen, Copy-on-Write, Snapshots/Klone**: emulieren den ZFS-Ansatz. `Pool::scrub` kann stille Beschädigungen im gesamten Pool in einem Durchgang erkennen und reparieren, über dieselbe API sowohl bei RAID-Z-Familien- als auch bei RAID10-Backends
- **NTFS-Kompatibilität**: ACL-Übersetzung (NFSv4 ⇔ NTFS), UID/GID-⇔-SID-Zuordnung (deterministische RID-basierte Zuordnung für lokale SAM-/AD-Domänen)
- **exFAT-Kompatibilität**: Konvertierung von Dateiattributen und Zeitstempeln, Unterstützung für Dateien/Volumes über 4 GB
- **GPU-/NPU-Hardwarebeschleunigung**: Die RAID-Z1/Z2/Z3-Paritätserzeugung wird über DirectX 12 Compute + DirectML ausgeführt (automatischer Rückfall auf die CPU, wenn keine Hardware vorhanden ist). Implementiert außerdem ein Verfahren, das die GF(2^8)-Koeffizientenmultiplikation in eine GF(2)-Bitmatrix umwandelt und auf einen einzigen DirectML-GEMM-Aufruf reduziert (`zfs_accel_hlsl::dml_gemm`), dessen Korrektheit auf echter GPU-Hardware verifiziert wurde (auf echter NPU-Hardware noch nicht verifiziert). Derselbe Mechanismus ist in die Wiederherstellungsberechnung eingebunden, die scrub/resilver bei erkannter Beschädigung ausführt (also die Paritätsprüfung). Zudem stehen eigene NPU-Shader-Pfade (`raidnpu_*.hlsl`) bereit, vorbereitet für künftige Verifikation/Optimierung auf echter NPU-Hardware
- **Anwendung des zpool auf echte Datenträger**: Der zpool-Initialisierungsassistent des Installers verfügt nun über einen Befehl (`init_zpool_apply`), der auf tatsächliche physische Datenträger (`\\.\PhysicalDriveN`) angewendet wird, nicht nur auf Vorschauen mit temporären Abbildern. Durch ein explizites Bestätigungsflag für das Löschen vorhandener Daten abgesichert
- **Copilot-artiger Konfigurationsberater**: empfiehlt eine RAID-Stufe basierend auf Festplattenlayout, Beschleuniger und CPU-Kernanzahl (erster heuristischer Ansatz; ein Grundgerüst zur Erkennung lokaler LLMs ist ebenfalls vorhanden). Die Logik liegt in `open_runo_installer_core`, unabhängig von Tauri, und kann auch unter Linux/macOS mit `cargo test` überprüft werden
- **Echtes WinFsp-Mounting (Windows)**: kann tatsächlich als Windows-Laufwerksbuchstabe eingebunden werden. Jedes Dataset im Pool erscheint als eigene Datei, mit Unterstützung für beliebige Byte-Offsets bei Lese-/Schreibvorgängen sowie für Erstellen/Löschen/Umbenennen/Anhängen/Kürzen von Dateien (es bleibt ein flacher Namensraum im Stammverzeichnis – Unterverzeichnisse werden noch nicht unterstützt). Auf echter Hardware verifiziert: Lesen, Schreiben, Erstellen, Löschen, Umbenennen, Anhängen und Kürzen von Dateien über ein tatsächlich eingebundenes Laufwerk.
- **Echtes FUSE-Mounting (Linux)**: derselbe `Pool` lässt sich auch direkt unter Linux einbinden (`fuse_mount.rs`), mit denselben Funktionen wie die Windows-Variante (Erstellen/Löschen/Umbenennen/Anhängen/Kürzen). End-to-End auf WSL2 Ubuntu 26.04 verifiziert – eingebunden und über gewöhnliche `std::fs`-Aufrufe getestet. Da es Inode-basiert ist, hat es nicht die bekannte Einschränkung der WinFsp-Variante, bei der ein anderes offenes Handle nach einem Umbenennen weiterhin auf den alten Namen verweisen kann. Das `fuser`-Crate verfügt außerdem über ein `macfuse-4-compat`-Feature, sodass sich dasselbe Design künftig plausibel auf macOS erweitern ließe (als Datenlaufwerk, nicht als Startdisk).
- **Mehrsprachige Unterstützung**: Der Installer (OpenRaidZ Installer) verwendet standardmäßig Englisch mit einem Sprachumschalter (10 Sprachen) in der Benutzeroberfläche, der auch nach der Installation geändert werden kann
- **Migrationswerkzeug für vorhandene Daten (`migrate`-Modul, experimentell)**: kopiert einen vorhandenen NTFS-(oder ähnlichen) Verzeichnisbaum in den Pool. Es schreibt niemals auf die Quelle, kann also **ausgeführt werden, während Windows weiterläuft**. Es kann jedoch **das aktuell laufende Systemlaufwerk (C: usw.) nicht an Ort und Stelle** unterbrechungsfrei in das RAID-Format umwandeln (ein Betriebssystem kann nicht zulassen, dass das gerade aktiv genutzte Volume von einer auf ihm selbst laufenden Software umgeschrieben wird – das ist eine grundlegende Einschränkung, keine fehlende Funktion). Es ist strikt ein Werkzeug, das „an einen anderen Ort (den Pool) kopiert". Derzeit nur als Bibliotheksfunktion verfügbar, noch ohne CLI/GUI; Unterverzeichnisse werden mittels eines Trennzeichens auf eine Ebene abgeflacht.
- **Persistenz der Metadaten (`Pool::save`/`Pool::open`)**: Dataset-Liste, Stripe-Zuordnungen, Snapshots und weitere Verwaltungsinformationen können nun in einem reservierten Bereich (Superblock) innerhalb des Pools gespeichert und daraus wiederhergestellt werden. Zuvor gab es diesen Mechanismus nicht – die rohen Datenbytes blieben zwar auf der Festplatte erhalten, aber die Information, welche Datei sich wo befand, ging verloren, sobald der Prozess (die Einbindung) beendet wurde. Sowohl die Windows-Variante (WinFsp) als auch die Linux-Variante (FUSE) speichern nun bei jeder ändernden Operation automatisch, und es wurde auf echter Hardware verifiziert, dass Dateien ein echtes Aushängen und erneutes Einhängen tatsächlich überstehen.

## Kapazitäts- und Dateigrößenlimits

- Die logische Größe eines Datasets (einer Datei) wird durchgängig als `u64` verwaltet, es gibt daher keine künstliche Grenze wie die 4-GB-Grenze von FAT32 (die theoretische Obergrenze liegt bei 2^64 Byte). Große Dateien wie Videos oder Bilder sind unproblematisch, solange sie innerhalb der tatsächlichen unten genannten Beschränkungen liegen.
- Die tatsächliche Grenze ist die **freie Kapazität des Pools** – die Summe der nutzbaren Kapazität der angeschlossenen Festplatten, abzüglich des Redundanz-Overheads der jeweiligen RAID-Stufe. Bei RAID-Z2 (doppelte Parität) entspricht die effektive Grenze beispielsweise etwa der kombinierten Kapazität der Datenfestplatten.
- Ein einzelner WinFsp-Lese-/Schreibaufruf ist durch die Windows-API selbst auf etwa 4 GiB (`u32`) begrenzt, doch das ist dieselbe Beschränkung wie bei jedem echten Dateisystem – das Betriebssystem/die Anwendung teilt größere Übertragungen automatisch in mehrere Aufrufe auf, sodass dies keine praktische Grenze darstellt.
- Aufgrund von Copy-on-Write benötigt jeder Schreibvorgang (Erstellen, Anhängen oder Überschreiben gleichermaßen) stets mindestens einen freien Stripe im Pool (dasselbe Prinzip wie der `slop space` von ZFS). Ein weiterer Stripe ist zusätzlich dauerhaft für die Speicherung von Metadaten reserviert. Wird der Pool zu 100 % ausgelastet, schlägt selbst das Überschreiben vorhandener Daten fehl. In der Praxis sollte stets ein paar Prozent des Pools frei bleiben.

## Aktuelle Einschränkungen (Prototypstadium)

- Das WinFsp-Mounting unterstützt nur einen flachen Namensraum im Stammverzeichnis. Unterverzeichnisse werden nicht unterstützt (dateibezogenes Erstellen/Löschen/Umbenennen hingegen schon).
- Lese-/Schreibvorgänge laufen über `Pool::read_unaligned`/`Pool::write_unaligned_growing` (eine Read-Modify-Write-Schicht) und unterstützen beliebige Byte-Offsets/-Längen; ein Schreibvorgang, der die aktuelle Größe überschreitet, lässt die Datei automatisch wachsen (siehe „Kapazitäts- und Dateigrößenlimits“ oben zu Kapazität und PATH-Hinweisen).
- `Pool` unterstützt sowohl `RaidZVdev` als auch `Raid10Vdev`, aber die Integration von RAID10 in die Datensatz-API ist an manchen Stellen noch oberflächlich.
- Der Code für das echte WinFsp-Mounting (`mount.rs`) kann nicht mit einer Rust-Toolchain vor Version 1.85 gebaut werden, da das `winfsp`-Crate das Cargo-Feature `edition2024` erfordert (siehe Build & Test unten).
- `mount.rs` und die GPU-Implementierung von `zfs_accel_hlsl` (Feature `gpu`) hängen vom `windows`-Crate ab, dessen Inhalt vollständig leer ist, sofern das Kompilierungsziel nicht tatsächlich Windows ist. Dieser Code kann daher nur auf einer echten Windows-Maschine (oder bei Cross-Kompilierung für ein Windows-Ziel) gebaut und getestet werden; unter Linux/macOS lässt er sich nur bauen, wenn diese Features über `--no-default-features` deaktiviert werden.
- Das Umbenennen (`rename`) einer Datei, während ein anderes offenes Handle noch darauf zeigt, kann dieses andere Handle für nachfolgende Operationen funktionsunfähig machen (`FileHandle` speichert den Namen konstruktionsbedingt direkt – Details siehe Dokumentation von `Pool::rename_dataset`).

## Build & Test

```powershell
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features   # ohne WinFsp-Mounting/GPU-Beschleunigung (reine CPU-Logik; weder dxc noch das WinFsp-SDK werden benötigt)
cargo test                         # Standard (inklusive echtem WinFsp-Mounting und GPU-/NPU-Beschleunigung; benötigt WinFsp + dxc)
```

`--no-default-features` deaktiviert sowohl das `winfsp_backend`- als auch das `gpu_accel`-Feature und ermöglicht es, die Kernlogik – RAID0/1/5/6/10/Z2/Z3, selbstheilende Prüfsummen, CoW, Snapshots/Klone, Resilver usw. – betriebssystemunabhängig zu überprüfen (funktioniert auch unter Linux/macOS). Weder WinFsp noch der DirectX Shader Compiler (dxc) noch GPU-/NPU-Hardware werden benötigt.

Der Build mit den Standard-Features (`winfsp_backend` + `gpu_accel`) erfordert:

- Die WinFsp-Laufzeitumgebung (https://winfsp.dev/), installiert auf dem System (die zur Build-Zeit verwendeten SDK-Header werden automatisch mitgeliefert, eine separate Installation der Entwicklerkomponente ist daher nicht erforderlich).
- `dxc` (den DirectX Shader Compiler, im Windows SDK oder im Vulkan SDK enthalten) im `PATH` (wird zum Kompilieren der RAID-Z/Z2-Paritäts-HLSL-Shader zur Build-Zeit verwendet).
- **Rust 1.85 oder neuer** (die Version, in der `edition2024`, benötigt vom `winfsp`-Crate, stabilisiert wurde; ältere Toolchains scheitern bereits beim Parsen des `Cargo.toml`-Manifests).

WinFsp oder dxc können auch einzeln deaktiviert werden (z. B. `--no-default-features --features gpu_accel` für nur GPU, ohne WinFsp).

**Hinweis zum tatsächlichen Ausführen der `winfsp_backend`-Tests (echtes Mounting)**: Das `winfsp`-Crate lädt die WinFsp-DLL (`winfsp-x64.dll`) dynamisch über `LoadLibraryW`, das nur den Standard-DLL-Suchpfad durchsucht (den Ordner der ausführbaren Datei, `System32` und `PATH`). In Umgebungen, in denen sich der WinFsp-Installer nicht selbst zum `PATH` hinzugefügt hat, gelingt der Build problemlos (keine WinFsp-SDK-Header erforderlich), aber die Ausführung **schlägt zur Laufzeit stets fehl** (Fehler `WIN32(1285)` = `ERROR_DELAY_LOAD_FAILED`). Fügen Sie das `bin`-Verzeichnis von WinFsp nur für den Testlauf zum `PATH` hinzu:

```powershell
$env:PATH = "C:\Program Files (x86)\WinFsp\bin;$env:PATH"
cargo test --features winfsp_backend,gpu_accel
```

Ohne dies gibt `mount_pool` einen `Err` zurück, und der Test behandelt dies als umgebungsabhängiges Problem, gibt über `eprintln` eine Übersprungsmeldung aus und kehrt vorzeitig zurück. **Ohne `--nocapture` erscheint dieses Überspringen dennoch nur als `ok`, nicht unterscheidbar von einem tatsächlich erfolgreichen Mount+Lesen/Schreiben.** Verwenden Sie beim Überprüfen von Echt-Mounting-Tests stets `--nocapture` und prüfen Sie visuell, dass keine Übersprungsmeldung erscheint.

### Build & Test der Linux-Variante (FUSE)

```bash
# Unter Ubuntu/Debian werden build-essential, pkg-config und libfuse3-dev benötigt.
sudo apt-get install -y build-essential pkg-config libfuse3-dev

cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features --features fuse_backend
```

Das Feature `fuse_backend` aktiviert das `fuser`-Crate (eine echte Anbindung an Linux' `libfuse3`). Es ist unabhängig von `winfsp_backend`/`gpu_accel` und kann auf Nicht-Linux-Zielen nicht aktiviert werden, da `fuser` dort nicht einmal als Abhängigkeit existiert (es liegt in `Cargo.toml` unter `target.'cfg(target_os = "linux")'.dependencies`). Der Echt-Mounting-Integrationstest (`tests/fuse_mount.rs`) wurde auf WSL2 Ubuntu 26.04 verifiziert – Erstellen, Schreiben, Lesen, Umbenennen, Kürzen, Löschen, ein Rundlauf einer größeren, über mehrere Stripes verteilten Datei sowie das Überstehen eines echten Aushängens und erneuten Einhängens durch die Metadaten. Wer nur unter Windows arbeitet, sollte für Build/Test des Linux-Ziels WSL2 (`wsl --install`) verwenden.

Ein kleines `orzctl`-Kommandozeilenwerkzeug ist ebenfalls enthalten, um einen Pool direkt über die Kommandozeile zu erstellen und einzubinden:

```bash
cargo build --no-default-features --features fuse_backend --bin orzctl
./target/debug/orzctl create --level z2 --chunk-size 4096 --stripes 1000 --dataset tank /path/to/disk0 /path/to/disk1 ...
./target/debug/orzctl mount  --level z2 --chunk-size 4096 --stripes 1000 --mountpoint /mnt/tank /path/to/disk0 /path/to/disk1 ...
```

Für automatisches Einbinden beim Systemstart
[`contrib/systemd/open-raid-z-pool.service.example`](../open_runo_zfs_source/open_raid_z_core/contrib/systemd/open-raid-z-pool.service.example)
als systemd-Unit registrieren (verifiziert auf einer VirtualBox-VM: ein
über 4 tatsächlich getrennte Blockgeräte erstellter Pool wird auch nach
einem echten Neustart automatisch wieder eingebunden).

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
