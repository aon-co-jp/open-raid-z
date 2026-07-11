# open-raid-z (Italiano)

**Un'implementazione Rust di un pool di storage RAID-Z/Z2/Z3 reale e
montabile.** Reimplementa le idee progettuali di ZFS/OpenZFS — striping
con parità distribuita, checksum autoriparanti, copy-on-write,
snapshot/cloni — **da zero in Rust, senza alcuna dipendenza da OpenZFS
stesso**. Lo strumento CLI `orzctl` crea pool e li **monta davvero** su
Windows (WinFsp) e Linux/macOS/Android (FUSE).

> [README radice](README.md) / [日本語](README-Japan.md) / [English](README-English.md) /
> [中文](README-Chinese.md) / [한국어](README-Korea.md) / [Español](README-Spain.md) /
> [Français](README-France.md) / [Deutsch](README-Germany.md) / [Русский](README-Russia.md) /
> [العربية](README-Arabic.md)

## Avvertenza importante

open-raid-z usa un **proprio formato su disco** (un layout CoW/striping
in stile ZFS) e **non è compatibile a livello di disco con il vero ZFS**
(niente uberblock, niente ZIL, ecc.). Migrare da ZFS/NTFS/ext4/altri RAID
esistenti significa sempre "① leggere dal formato esistente → ② copiare
normalmente i file in un pool open-raid-z". Vedi [MIGRATION.md](MIGRATION.md).

## Struttura del workspace (3 crate + componenti di supporto)

| Componente | Ruolo / stato |
|---|---|
| `open_raid_z_core` | Libreria principale: livelli RAID (`Raid0`/`Raid1`/`Raid5`/`Raid6`≡`Z2`/`Z3`, enum `RaidLevel` in `vdev.rs`), checksum sha2, copy-on-write, snapshot/cloni, emulazione ACL, interoperabilità FAT32/exFAT (`foreign_fs`, lettura+scrittura), montaggio reale (WinFsp su Windows, FUSE su Linux/macOS/Android) e il binario CLI `orzctl` |
| `zfs_accel_hlsl` | Accelera su GPU il calcolo della parità in campo di Galois per RAID-Z/Z2/Z3 tramite shader HLSL + D3D12/DirectML. Con la feature `gpu_accel` disattivata, ricade su un'implementazione CPU in Rust puro (utile per CI senza WinFsp/dxc) |
| `open_runo_installer_core` | Logica indipendente dal SO per il rilevamento dei dischi, consulenza sulla configurazione zpool e anteprima; deliberatamente separata come crate indipendente da Tauri per non essere condizionata dai vincoli edition2024 di Tauri |
| `open_runo_installer` (GUI Tauri) | Un'app desktop Tauri 2 + TypeScript che usa `installer_core`. **È l'unico punto dell'intero ecosistema che dipende direttamente dal pacchetto Tauri** (a parte la politica dei repo dell'ecosistema web di reimplementare Tauri da zero) |
| `wdk_driver/orzflt` | Uno scheletro minimo di driver in modalità kernel Windows (WDF/KMDF 1.35). È stato verificato solo il caricamento/scaricamento in fase di build; **i test di caricamento reali sono deliberatamente riservati a una VM isolata** — fase iniziale |
| `third_party/fuser-0.17.0-android-patch` | Un fork patchato del crate `fuser` che consente build puramente Rust per Android. Compila in cross-compilazione verso arm64-v8a tramite `cargo ndk`; non ancora verificato su dispositivo reale |

## Riga di comando `orzctl`

```sh
# crea un pool Z2 su 6 dischi
orzctl create --level z2 --chunk-size 4096 --stripes 100000 --dataset tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# montalo davvero (resta in primo piano)
orzctl mount --level z2 --chunk-size 4096 --stripes 100000 --mountpoint /mnt/tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# leggi/scrivi un volume FAT32/exFAT esistente (aiuto alla migrazione)
orzctl foreign ls /dev/sdb1
orzctl foreign --format exfat cat /dev/sdc1 /video.mp4 ./video.mp4
```

Livelli RAID supportati: `Raid0` / `Raid1` (mirror) / `Raid5` / `Raid6`
(uguale a `Z2`) / `Z2` / `Z3`. Il RAID10 è fornito separatamente come
gruppo di mirror `Raid1` (`raid10.rs`).

## Build e test (misurati)

```sh
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features
```

È una build di fallback su CPU che non richiede né l'SDK WinFsp, né
`dxc`, né l'SDK Windows. Misurato il 2026-07-11:

| Crate | superati | falliti |
|---|---|---|
| `open_raid_z_core` (`--no-default-features`) | 101 | 0 |
| `zfs_accel_hlsl` (`--no-default-features`, fallback CPU) | 32 | 0 |
| `open_runo_installer_core` | 30 | 0 |
| **Totale** | **163** | **0** |

Il set di feature `default` (`winfsp_backend` + `gpu_accel`, montaggio
reale + calcolo GPU reale) richiede una macchina Windows con l'SDK
WinFsp e `dxc` e deve essere verificato separatamente.

## Documentazione

- [MIGRATION.md](MIGRATION.md) — migrazione da ZFS/NTFS/ext4/altri RAID
- [PORTING.md](PORTING.md) — guida in un'unica pagina per adottarlo in un altro progetto
- [CLAUDE.md](CLAUDE.md) — regole di sviluppo / stack tecnologico (canonico per questo ecosistema)
- [CHAT_HANDOFF.md](CHAT_HANDOFF.md) — cronologia di sviluppo / note di passaggio

## Progetti correlati

Esiste un'architettura obiettivo che combina `open-web-server` con
`poem-cosmo-tauri`/`open-runo`, PostgreSQL, `aruaru-db` e questo
repository, pensata per evitare la perdita in rete di oggetti a
pagamento e dati finanziari/titoli in giochi online 3D (trasporto a
quadrupla ridondanza e scritture DB a quadrupla ridondanza, rivisto il
2026-07-11). open-raid-z vi partecipa come base di ridondanza del
disco, e le sue caratteristiche simil-ZFS (checksum, copy-on-write,
snapshot) hanno rilevanza reale e documentata anche per l'affidabilità
di lettura/scrittura dei database (PostgreSQL/aruaru-db) (per il quadro
completo con fonti vedi il `README.md`/`CLAUDE.md` di
[open-web-server](https://github.com/aon-co-jp/open-web-server)).

## Licenza

MPL-2.0.
