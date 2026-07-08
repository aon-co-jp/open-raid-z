# open-raid-z

Un projet de système de fichiers expérimental pour Windows, quasi compatible avec NTFS/exFAT, offrant des fonctionnalités de type ZFS (sommes de contrôle auto-réparatrices, pools de stockage, copy-on-write, instantanés/clones) ainsi que RAID0/1/5/6/10/Z2/Z3.

Langue : [日本語](README-Japan.md) | [UK English](README-UK-English.md) | [US English](README-US-English.md) | [Italiano](README-Italy.md) | **Français** | [Deutsch](README-Germany.md) | [Русский](README-Russia.md) | [Українська](README-Ukraine.md) | [العربية](README-Arabic.md) | [فارسی](<README-Iran(Persian).md>)

## Convention de nommage

Les identifiants définis par ce projet lui-même — noms de répertoires, noms de crates, noms de paquets npm, noms des fonctionnalités (features) Cargo, id/classes HTML/CSS, etc. — utilisent de façon cohérente **le tiret bas (`_`) plutôt que le trait d'union (`-`)** (ex. `open_zfs_winfsp_bridge`, `zfs_accel_hlsl`, `open_runo_installer`, `open_runo_installer_core`, et les fonctionnalités Cargo `winfsp_backend`/`gpu_accel`). Les noms auparavant écrits avec des traits d'union, comme `openzfs-winfsp-bridge`, ont été renommés pour assurer la cohérence au sein du projet.

Sont exclus de cette règle les éléments suivants, car ils suivent des spécifications externes ou des conventions d'écosystème plutôt que le choix de nommage propre à ce projet :

- Le nom du dépôt lui-même (`open-raid-z` ; c'est le nom réel du dépôt GitHub et il ne peut pas être modifié)
- Les attributs personnalisés HTML5 `data-*` (`data-i18n` ; le trait d'union est imposé par la spécification)
- Les noms de paquets npm externes (ex. `@tauri-apps/api`, les noms réels des paquets publiés)
- Les noms de propriétés CSS (ex. `font-family` ; c'est la syntaxe même du langage CSS)
- Les termes composés anglais qui contiennent réellement un trait d'union, comme Reed-Solomon ou copy-on-write

## Composants

| Composant | Rôle |
|---|---|
| `open_zfs_winfsp_bridge` | vdev RAID-Z/RAID0-10, pool de stockage, couche de compatibilité ACL NTFS/attributs exFAT, montage réel via WinFsp |
| `zfs_accel_hlsl` | Délestage du calcul de parité vers des accélérateurs matériels GPU/NPU (DirectX 12 Compute + DirectML) |
| `open_runo_installer_core` | Logique indépendante du système d'exploitation pour la détection des disques, le conseiller de configuration façon Copilot et l'aperçu d'initialisation zpool (aucune dépendance à Tauri ; `cargo test` fonctionne aussi sous Linux/macOS) |
| `open_runo_installer` | L'installateur Tauri lui-même (une fine couche d'interface qui appelle `open_runo_installer_core`) : détection matérielle, assistant d'initialisation zpool, interface du conseiller de configuration façon Copilot |

## Fonctionnalités principales

- **Toute la série RAID** : RAID0 / RAID1 (miroir) / RAID5 / RAID6 / RAID10 (miroirs agrégés) / RAID-Z2 / RAID-Z3
- **Partitionnement et réutilisation de disque** : diviser un disque physique et utiliser une moitié comme membre d'un miroir tandis que l'autre rejoint une matrice RAID6/Z2 distincte
- **Sommes de contrôle auto-réparatrices, copy-on-write, instantanés/clones** : imitent l'approche de ZFS. `Pool::scrub` peut détecter et réparer en une seule passe la corruption silencieuse sur l'ensemble du pool, via la même API pour les backends RAID-Z comme RAID10
- **Compatibilité NTFS** : traduction des ACL (NFSv4 ⇔ NTFS), correspondance UID/GID ⇔ SID (correspondance déterministe basée sur le RID pour les domaines SAM locaux/AD)
- **Compatibilité exFAT** : conversion des attributs de fichiers et des horodatages, prise en charge des fichiers/volumes de plus de 4 Go
- **Accélération matérielle GPU/NPU** : le calcul de parité RAID-Z1/Z2 est réellement envoyé via DirectX 12 Compute + DirectML (repli automatique sur le CPU en l'absence de matériel)
- **Conseiller de configuration façon Copilot** : recommande un niveau RAID selon la disposition des disques, l'accélérateur et le nombre de cœurs CPU (première version heuristique ; une ébauche de détection de LLM local est également en place). La logique réside dans `open_runo_installer_core`, indépendant de Tauri, et peut aussi être vérifiée avec `cargo test` sous Linux/macOS
- **Montage réel via WinFsp (prototype)** : peut réellement être monté en tant que lettre de lecteur Windows. Chaque jeu de données du pool apparaît comme son propre fichier, avec prise en charge d'offsets et de longueurs arbitraires en octets pour la lecture et l'écriture (la hiérarchie de répertoires et create/delete/rename ne sont pas encore prises en charge — l'espace de noms reste plat)
- **Prise en charge multilingue** : l'installateur utilise le japonais par défaut avec un sélecteur de langue dans l'interface, modifiable même après l'installation

## Limitations actuelles (stade prototype)

- Le montage WinFsp ne prend en charge qu'un espace de noms plat (chaque jeu de données du pool apparaît comme un fichier à la racine). Pas encore de hiérarchie de répertoires ni de create/delete/rename par fichier.
- Les lectures/écritures passent par `Pool::read_unaligned`/`Pool::write_unaligned` (une couche read-modify-write), ce qui permet des offsets et longueurs arbitraires en octets. Les requêtes dépassant la capacité allouée d'un jeu de données (définie via `grow_dataset`) échouent toujours (aucune extension automatique implicite).
- `Pool` prend en charge à la fois `RaidZVdev` et `Raid10Vdev`, mais l'intégration de RAID10 avec l'API de jeu de données reste superficielle à certains endroits.
- Le code du montage réel WinFsp (`mount.rs`) ne peut pas être compilé avec une chaîne d'outils Rust antérieure à la 1.85, car le crate `winfsp` requiert la fonctionnalité Cargo `edition2024` (voir Compilation et tests ci-dessous).
- `mount.rs` et l'implémentation GPU de `zfs_accel_hlsl` (fonctionnalité `gpu`) dépendent du crate `windows`, dont le contenu est entièrement vide sauf si la cible de compilation est réellement Windows. Ce code ne peut donc être compilé et testé que sur une véritable machine Windows (ou en compilation croisée vers une cible Windows) ; sous Linux/macOS, il ne compile qu'une fois ces fonctionnalités désactivées via `--no-default-features`.

## Compilation et tests

```powershell
cd open_runo_zfs_source/open_zfs_winfsp_bridge
cargo test --no-default-features   # sans montage WinFsp ni accélération GPU (logique CPU pure ; ni dxc ni le SDK WinFsp ne sont nécessaires)
cargo test                         # par défaut (inclut le montage réel WinFsp et l'accélération GPU/NPU ; nécessite WinFsp + dxc)
```

`--no-default-features` désactive à la fois les fonctionnalités `winfsp_backend` et `gpu_accel`, ce qui permet de vérifier la logique principale — RAID0/1/5/6/10/Z2/Z3, sommes de contrôle auto-réparatrices, CoW, instantanés/clones, resilver, etc. — de façon indépendante du système d'exploitation (fonctionne aussi sous Linux/macOS). Ni WinFsp, ni le DirectX Shader Compiler (dxc), ni du matériel GPU/NPU ne sont nécessaires.

La compilation avec les fonctionnalités par défaut (`winfsp_backend` + `gpu_accel`) nécessite :

- Le runtime WinFsp (https://winfsp.dev/) installé sur le système (les en-têtes SDK utilisés lors de la compilation sont fournis automatiquement, aucune installation séparée du composant développeur n'est donc nécessaire).
- `dxc` (le DirectX Shader Compiler, fourni avec le Windows SDK ou le Vulkan SDK) dans le `PATH` (utilisé pour compiler les shaders HLSL de parité RAID-Z/Z2 lors de la compilation).
- **Rust 1.85 ou supérieur** (la version dans laquelle `edition2024`, requise par le crate `winfsp`, a été stabilisée ; avec des chaînes d'outils plus anciennes, l'analyse du manifeste `Cargo.toml` échoue déjà).

Il est également possible de désactiver WinFsp ou dxc séparément (ex. `--no-default-features --features gpu_accel` pour le GPU seul, sans WinFsp).

### Installateur (`open_runo_installer` / `open_runo_installer_core`)

```powershell
# Couche logique (aucune dépendance à Tauri ; fonctionne aussi sous Linux/macOS)
cd open_runo_zfs_source/open_runo_installer_core
cargo test                    # repli CPU uniquement (par défaut)
cargo test --features gpu     # inclut le dispatch GPU/NPU réel (nécessite une véritable machine Windows + dxc)

# Frontend (TypeScript, indépendant du système d'exploitation)
cd open_runo_zfs_source/open_runo_installer
npm install
npx tsc --noEmit               # vérification des types uniquement
npx vite build                 # compilation réelle

# L'application Tauri elle-même (nécessite une véritable machine Windows, ou un Rust suffisamment récent plus les dépendances de bureau Linux)
cd open_runo_zfs_source/open_runo_installer/src-tauri
cargo tauri dev / cargo tauri build
```

`open_runo_installer_core` (détection des disques, conseiller de configuration façon Copilot, aperçu d'initialisation zpool) est un crate indépendant sans dépendance à Tauri : sa logique peut donc être vérifiée telle quelle même dans des environnements dépourvus de ce dont Tauri lui-même a besoin pour compiler (une WebView, GTK, etc., ainsi qu'une chaîne d'outils Rust suffisamment récente). Seule l'énumération réelle des disques (`\\.\PhysicalDriveN`) utilise une API propre à Windows, isolée derrière `#[cfg(windows)]` ; tout le reste (conseiller de configuration et calculs de l'aperçu zpool) est indépendant du système d'exploitation, et l'ensemble de ses 26 tests est confirmé comme réussi.

## Licence

MPL-2.0
