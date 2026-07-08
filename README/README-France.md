# open-raid-z

Un projet de système de fichiers expérimental pour Windows, quasi compatible avec NTFS/exFAT, offrant des fonctionnalités de type ZFS (sommes de contrôle auto-réparatrices, pools de stockage, copy-on-write, instantanés/clones) ainsi que RAID0/1/5/6/10/Z2/Z3.

Langue : [日本語](README-Japan.md) | [UK English](README-UK-English.md) | [US English](README-US-English.md) | [Italiano](README-Italy.md) | **Français** | [Deutsch](README-Germany.md) | [Русский](README-Russia.md) | [Українська](README-Ukraine.md) | [العربية](README-Arabic.md) | [فارسی](<README-Iran(Persian).md>)

## Composants

| Composant | Rôle |
|---|---|
| `openzfs-winfsp-bridge` | vdev RAID-Z/RAID0-10, pool de stockage, couche de compatibilité ACL NTFS/attributs exFAT, montage réel via WinFsp |
| `zfs-accel-hlsl` | Délestage du calcul de parité vers des accélérateurs matériels GPU/NPU (DirectX 12 Compute + DirectML) |
| `openruno-installer` | Installateur Tauri : détection matérielle, assistant d'initialisation zpool, conseiller de configuration façon Copilot |

## Fonctionnalités principales

- **Toute la série RAID** : RAID0 / RAID1 (miroir) / RAID5 / RAID6 / RAID10 (miroirs agrégés) / RAID-Z2 / RAID-Z3
- **Partitionnement et réutilisation de disque** : diviser un disque physique et utiliser une moitié comme membre d'un miroir tandis que l'autre rejoint une matrice RAID6/Z2 distincte
- **Sommes de contrôle auto-réparatrices, copy-on-write, instantanés/clones** : imitent l'approche de ZFS
- **Compatibilité NTFS** : traduction des ACL (NFSv4 ⇔ NTFS), correspondance UID/GID ⇔ SID (correspondance déterministe basée sur le RID pour les domaines SAM locaux/AD)
- **Compatibilité exFAT** : conversion des attributs de fichiers et des horodatages, prise en charge des fichiers/volumes de plus de 4 Go
- **Accélération matérielle GPU/NPU** : le calcul de parité RAID-Z1/Z2 est réellement envoyé via DirectX 12 Compute + DirectML (repli automatique sur le CPU en l'absence de matériel)
- **Conseiller de configuration façon Copilot** : recommande un niveau RAID selon la disposition des disques, l'accélérateur et le nombre de cœurs CPU (première version heuristique ; une ébauche de détection de LLM local est également en place)
- **Montage réel via WinFsp (prototype)** : peut réellement être monté en tant que lettre de lecteur Windows (actuellement une version minimale à fichier unique)
- **Prise en charge multilingue** : l'installateur utilise le japonais par défaut avec un sélecteur de langue dans l'interface, modifiable même après l'installation

## Limitations actuelles (stade prototype)

- Le montage WinFsp ne prend en charge qu'un espace de noms plat (un seul fichier fixe `\pool.dat` à la racine). Pas encore de hiérarchie de répertoires ni de fichiers multiples.
- Les lectures/écritures doivent être alignées sur la limite de bloc (chunk) du jeu de données.
- `Pool` prend en charge à la fois `RaidZVdev` et `Raid10Vdev`, mais l'intégration de RAID10 avec l'API de jeu de données reste superficielle à certains endroits.

## Compilation et tests

```powershell
cd open-runo-zfs-source/openzfs-winfsp-bridge
cargo test --no-default-features        # sans le montage WinFsp
cargo test --features winfsp-backend    # avec le montage réel WinFsp (nécessite le runtime WinFsp)
```

Le runtime WinFsp (https://winfsp.dev/) doit être installé sur le système (les en-têtes SDK utilisés lors de la compilation sont fournis automatiquement, aucune installation séparée du composant développeur n'est donc nécessaire).

## Licence

MPL-2.0
