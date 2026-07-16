# open-raid-z (Français)

**Une implémentation en Rust d'un pool de stockage RAID-Z/Z2/Z3 réel et
montable.** Elle réimplémente les concepts de conception de ZFS/OpenZFS —
striping avec parité distribuée, checksums auto-réparateurs,
copy-on-write, snapshots/clones — **entièrement en Rust, sans aucune
dépendance à OpenZFS lui-même**. L'outil CLI `orzctl` crée des pools et
les **monte réellement** sur Windows (WinFsp) et Linux/macOS/Android
(FUSE).

> [README racine](README.md) / [日本語](README-Japan.md) / [English](README-English.md) /
> [中文](README-Chinese.md) / [한국어](README-Korea.md) / [Español](README-Spain.md) /
> [Deutsch](README-Germany.md) / [Italiano](README-Italy.md) / [Русский](README-Russia.md) /
> [العربية](README-Arabic.md)

## Avertissement important

open-raid-z utilise **son propre format sur disque** (une mise en page
CoW/striping à la ZFS) et **n'est pas compatible sur disque avec le vrai
ZFS** (pas d'uberblock, pas de ZIL, etc.). Migrer depuis un ZFS/NTFS/
ext4/autre RAID existant signifie toujours « ① lire depuis le format
existant → ② copier les fichiers normalement dans un pool open-raid-z ».
Voir [MIGRATION.md](MIGRATION.md).

## Structure du workspace (3 crates + composants annexes)

| Composant | Rôle / statut |
|---|---|
| `open_raid_z_core` | Bibliothèque principale : niveaux RAID (`Raid0`/`Raid1`/`Raid5`/`Raid6`≡`Z2`/`Z3`, enum `RaidLevel` dans `vdev.rs`), checksums sha2, copy-on-write, snapshots/clones, émulation d'ACL, interopérabilité FAT32/exFAT (`foreign_fs`, lecture+écriture), montage réel (WinFsp sous Windows, FUSE sous Linux/macOS/Android) et le binaire CLI `orzctl` |
| `zfs_accel_hlsl` | Accélère par GPU le calcul de parité en corps de Galois pour RAID-Z/Z2/Z3 via des shaders HLSL + D3D12/DirectML. Avec la feature `gpu_accel` désactivée, retombe sur une implémentation CPU en Rust pur (utile en CI sans WinFsp/dxc) |
| `open_runo_installer_core` | Logique indépendante de l'OS pour la détection des disques, le conseil de configuration zpool et l'aperçu ; volontairement séparée en crate indépendante de Tauri pour ne pas être affectée par les contraintes edition2024 de Tauri |
| `open_runo_installer` (GUI Tauri) | Une application de bureau Tauri 2 + TypeScript utilisant `installer_core`. **C'est le seul endroit de tout l'écosystème qui dépend directement du paquet Tauri** (à part la politique des dépôts de l'écosystème web de réimplémenter Tauri de zéro) |
| `wdk_driver/orzflt` | Un squelette minimal de pilote en mode noyau Windows (WDF/KMDF 1.35). Seuls le chargement/déchargement ont été vérifiés à la compilation ; **les tests de chargement réels sont volontairement réservés à une VM isolée** — stade précoce |
| `third_party/fuser-0.17.0-android-patch` | Un fork patché du crate `fuser` permettant des builds purement Rust pour Android. Compile en croisé vers arm64-v8a via `cargo ndk` ; pas encore vérifié sur un appareil réel |

## Ligne de commande `orzctl`

```sh
# créer un pool Z2 avec 6 disques
orzctl create --level z2 --chunk-size 4096 --stripes 100000 --dataset tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# le monter réellement (reste au premier plan)
orzctl mount --level z2 --chunk-size 4096 --stripes 100000 --mountpoint /mnt/tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# lire/écrire un volume FAT32/exFAT existant (aide à la migration)
orzctl foreign ls /dev/sdb1
orzctl foreign --format exfat cat /dev/sdc1 /video.mp4 ./video.mp4
```

Niveaux RAID pris en charge : `Raid0` / `Raid1` (miroir) / `Raid5` /
`Raid6` (identique à `Z2`) / `Z2` / `Z3`. Le RAID10 est fourni séparément
comme un ensemble de groupes miroirs `Raid1` (`raid10.rs`).

## Compilation et tests (mesurés)

```sh
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features
```

C'est une build de repli CPU ne nécessitant ni le SDK WinFsp, ni `dxc`,
ni le SDK Windows. Mesuré le 2026-07-11 :

| Crate | réussis | échoués |
|---|---|---|
| `open_raid_z_core` (`--no-default-features`) | 101 | 0 |
| `zfs_accel_hlsl` (`--no-default-features`, repli CPU) | 32 | 0 |
| `open_runo_installer_core` | 30 | 0 |
| **Total** | **163** | **0** |

L'ensemble de features `default` (`winfsp_backend` + `gpu_accel`, montage
réel + calcul GPU réel) nécessite une machine Windows avec le SDK WinFsp
et `dxc`, et doit être vérifié séparément.

## Documentation

- [MIGRATION.md](MIGRATION.md) — migration depuis ZFS/NTFS/ext4/autre RAID
- [PORTING.md](PORTING.md) — guide en une page pour l'adopter dans un autre projet
- [CLAUDE.md](CLAUDE.md) — règles de développement / stack technique (référence de cet écosystème)
- [CHAT_HANDOFF.md](CHAT_HANDOFF.md) — historique de développement / notes de passation

## Projets liés

Il existe une architecture cible combinant `open-web-server` avec
`poem-cosmo-tauri`/`open-runo`, PostgreSQL, `aruaru-db` et ce dépôt,
conçue pour éviter la perte réseau des objets payants et des données
financières/boursières dans les jeux en ligne 3D (transport à quadruple
redondance et écritures BD à quadruple redondance, révisé le
2026-07-11). open-raid-z y participe comme fondation de redondance
disque, et ses caractéristiques de type ZFS (checksums, copy-on-write,
snapshots) ont une pertinence réelle et documentée pour la fiabilité de
lecture/écriture des bases de données (PostgreSQL/aruaru-db) également
(voir le `README.md`/`CLAUDE.md` de
[open-web-server](https://github.com/aon-co-jp/open-web-server) pour la
vue d'ensemble avec les sources).

## Licence

MPL-2.0.
