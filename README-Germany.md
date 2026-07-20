# open-raid-z (Deutsch)

**Eine Rust-Implementierung eines echten, mountbaren RAID-Z/Z2/Z3-
Storage-Pools.** Sie implementiert die Designideen von ZFS/OpenZFS —
paritätsverteiltes Striping, selbstheilende Prüfsummen, Copy-on-Write,
Snapshots/Clones — **von Grund auf in Rust, ganz ohne Abhängigkeit von
OpenZFS selbst**. Das CLI-Tool `orzctl` erstellt Pools und **mountet sie
tatsächlich** unter Windows (WinFsp) und Linux/macOS/Android (FUSE).

> [Root-README](README.md) / [日本語](README-Japan.md) / [English](README-English.md) /
> [中文](README-Chinese.md) / [한국어](README-Korea.md) / [Español](README-Spain.md) /
> [Français](README-France.md) / [Italiano](README-Italy.md) / [Русский](README-Russia.md) /
> [العربية](README-Arabic.md)

## Wichtiger Hinweis

open-raid-z verwendet ein **eigenes On-Disk-Format** (ein ZFS-artiges
CoW-/Striping-Layout) und ist **nicht on-disk-kompatibel mit echtem ZFS**
(kein Uberblock, kein ZIL usw.). Eine Migration von bestehendem ZFS/NTFS/
ext4/anderem RAID bedeutet immer „① aus dem bestehenden Format lesen →
② Dateien ganz normal in einen open-raid-z-Pool kopieren". Siehe
[MIGRATION.md](MIGRATION.md).

## Workspace-Struktur (3 Crates + unterstützende Komponenten)

| Komponente | Rolle / Status |
|---|---|
| `open_raid_z_core` | Kernbibliothek: RAID-Level (`Raid0`/`Raid1`/`Raid5`/`Raid6`≡`Z2`/`Z3`, Enum `RaidLevel` in `vdev.rs`), sha2-Prüfsummen, Copy-on-Write, Snapshots/Clones, ACL-Emulation, FAT32/exFAT-Interoperabilität (`foreign_fs`, Lesen+Schreiben) sowie schreibgeschützter ext2/ext4-Zugriff (gleiches Feature), echtes Mounten (WinFsp unter Windows, FUSE unter Linux/macOS/Android) sowie das `orzctl`-CLI-Binary |
| `zfs_accel_hlsl` | Beschleunigt die Galois-Feld-Paritätsberechnung für RAID-Z/Z2/Z3 per GPU über HLSL-Shader + D3D12/DirectML. Bei deaktiviertem `gpu_accel`-Feature fällt es auf eine reine Rust-CPU-Implementierung zurück (nützlich für CI ohne WinFsp/dxc) |
| `open_runo_installer_core` | Betriebssystemunabhängige Logik für Disk-Erkennung, zpool-Konfigurationsberatung und Vorschau; bewusst als von Tauri unabhängiges Crate ausgelagert, um nicht von Tauris edition2024-Anforderungen betroffen zu sein |
| `open_runo_installer` (Tauri-GUI) | Eine Tauri-2 + TypeScript-Desktop-App, die `installer_core` nutzt. **Dies ist die einzige Stelle im gesamten Ökosystem, die direkt vom Tauri-Paket abhängt** (getrennt von der Richtlinie der Web-Ökosystem-Repos, Tauri selbst nachzubauen) |
| `wdk_driver/orzflt` | Ein minimales Skelett eines Windows-Kernel-Mode-Treibers (WDF/KMDF 1.35). Nur Laden/Entladen wurde per Build verifiziert; **echte Ladetests sind bewusst einer isolierten VM vorbehalten** — frühes Stadium |
| `third_party/fuser-0.17.0-android-patch` | Ein gepatchter Fork des `fuser`-Crates, der reine Rust-Builds für Android ermöglicht. Cross-kompiliert per `cargo ndk` nach arm64-v8a; noch nicht auf echtem Gerät verifiziert |

## `orzctl`-Kommandozeile

```sh
# Z2-Pool über 6 Festplatten erstellen
orzctl create --level z2 --chunk-size 4096 --stripes 100000 --dataset tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# tatsächlich mounten (bleibt im Vordergrund)
orzctl mount --level z2 --chunk-size 4096 --stripes 100000 --mountpoint /mnt/tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# bestehendes FAT32/exFAT-Volume lesen/schreiben (Migrationshilfe)
orzctl foreign ls /dev/sdb1
orzctl foreign --format exfat cat /dev/sdc1 /video.mp4 ./video.mp4

# bestehendes ext2/ext4-Volume lesen (schreibgeschützt)
orzctl foreign --format ext4 ls  /dev/sdd1 /home
orzctl foreign --format ext4 cat /dev/sdd1 /etc/hostname
```

Unterstützte RAID-Level: `Raid0` / `Raid1` (Spiegel) / `Raid5` / `Raid6`
(identisch zu `Z2`) / `Z2` / `Z3`. RAID10 wird separat als Bündel von
`Raid1`-Spiegelgruppen bereitgestellt (`raid10.rs`).

## Build & Tests (gemessen)

```sh
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features
```

Dies ist ein CPU-Fallback-Build, der weder das WinFsp-SDK noch `dxc` noch
das Windows-SDK benötigt. Gemessen am 2026-07-11:

| Crate | bestanden | fehlgeschlagen |
|---|---|---|
| `open_raid_z_core` (`--no-default-features`) | 101 | 0 |
| `zfs_accel_hlsl` (`--no-default-features`, CPU-Fallback) | 32 | 0 |
| `open_runo_installer_core` | 30 | 0 |
| **Gesamt** | **163** | **0** |

Der `default`-Feature-Satz (`winfsp_backend` + `gpu_accel`, echtes Mounten
+ echte GPU-Berechnung) erfordert eine Windows-Maschine mit WinFsp-SDK
und `dxc` und muss separat verifiziert werden.

## Dokumentation

- [MIGRATION.md](MIGRATION.md) — Migration von ZFS/NTFS/ext4/anderem RAID
- [PORTING.md](PORTING.md) — Ein-Datei-Anleitung zur Übernahme in ein anderes Projekt
- [CLAUDE.md](CLAUDE.md) — Entwicklungsregeln / Tech-Stack (kanonisch für dieses Ökosystem)
- [CHAT_HANDOFF.md](CHAT_HANDOFF.md) — Entwicklungsverlauf / Übergabenotizen

## Verwandte Projekte

Es gibt eine Zielarchitektur, die `open-web-server` mit
`poem-cosmo-tauri`/`open-runo`, PostgreSQL, `aruaru-db` und diesem
Repository kombiniert, um den Netzwerkverlust von Bezahlgegenständen
sowie Finanz-/Wertpapierdaten in 3D-Online-Spielen zu verhindern
(vierfach redundanter Transport und vierfach redundante
DB-Schreibvorgänge, überarbeitet am 2026-07-11). open-raid-z wirkt darin
als Festplatten-Redundanzfundament mit, und seine ZFS-ähnlichen
Eigenschaften (Prüfsummen, Copy-on-Write, Snapshots) haben ebenfalls
reale, dokumentierte Relevanz für die Lese-/Schreibzuverlässigkeit von
Datenbanken (PostgreSQL/aruaru-db) (vollständiges Bild mit Quellen in
`README.md`/`CLAUDE.md` von
[open-web-server](https://github.com/aon-co-jp/open-web-server)).

## Lizenz

MPL-2.0.
