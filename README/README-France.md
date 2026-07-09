# open-raid-z

Un projet de système de fichiers expérimental pour Windows/Linux, quasi compatible avec NTFS/exFAT, offrant des fonctionnalités de type ZFS (sommes de contrôle auto-réparatrices, pools de stockage, copy-on-write, instantanés/clones) ainsi que RAID0/1/5/6/10/Z2/Z3. La logique principale est un unique programme partagé indépendant du système d'exploitation (`open_raid_z_core`) ; la version Windows (WinFsp) et la version Linux (FUSE) ne diffèrent que par la fine couche de montage placée par-dessus (distribuées sous les noms `open-raid-z-win`/`open-raid-z-linux`).

Langue : [日本語](README-Japan.md) | [UK English](README-UK-English.md) | [US English](README-US-English.md) | [Italiano](README-Italy.md) | **Français** | [Deutsch](README-Germany.md) | [Русский](README-Russia.md) | [Українська](README-Ukraine.md) | [العربية](README-Arabic.md) | [فارسی](<README-Iran(Persian).md>)

## Un message à l'attention de Microsoft et d'Apple

Nous développons ce système de fichiers expérimental afin d'apporter à Windows des fonctionnalités complètes de type ZFS (sommes de contrôle auto-réparatrices, RAID6/RAID-Z2, instantanés, et plus encore). L'un de nos objectifs à long terme est que ce système de fichiers puisse un jour être sélectionnable comme destination d'installation officielle et disque de démarrage sous Windows et macOS.

Nous comprenons que cela nécessite la coopération de chaque éditeur de système d'exploitation — signature/certification des pilotes de démarrage, prise en charge officielle par l'installateur, etc. Si vous portez un intérêt à cette démarche, nous accueillerons avec grand plaisir votre contact et votre collaboration. Il s'agit d'un projet modeste et indépendant, mais nous tenons sincèrement à voir cette technologie aboutir.

## Convention de nommage

Les identifiants définis par ce projet lui-même — noms de répertoires, noms de crates, noms de paquets npm, noms des fonctionnalités (features) Cargo, id/classes HTML/CSS, etc. — utilisent de façon cohérente **le tiret bas (`_`) plutôt que le trait d'union (`-`)** (ex. `open_raid_z_core`, `zfs_accel_hlsl`, `open_runo_installer`, `open_runo_installer_core`, et les fonctionnalités Cargo `winfsp_backend`/`gpu_accel`). Les noms auparavant écrits avec des traits d'union, comme `openzfs-winfsp-bridge`, ont été renommés pour assurer la cohérence au sein du projet.

Sont exclus de cette règle les éléments suivants, car ils suivent des spécifications externes ou des conventions d'écosystème plutôt que le choix de nommage propre à ce projet :

- Le nom du dépôt lui-même (`open-raid-z` ; c'est le nom réel du dépôt GitHub et il ne peut pas être modifié)
- Les attributs personnalisés HTML5 `data-*` (`data-i18n` ; le trait d'union est imposé par la spécification)
- Les noms de paquets npm externes (ex. `@tauri-apps/api`, les noms réels des paquets publiés)
- Les noms de propriétés CSS (ex. `font-family` ; c'est la syntaxe même du langage CSS)
- Les termes composés anglais qui contiennent réellement un trait d'union, comme Reed-Solomon ou copy-on-write

## Composants

| Composant | Rôle |
|---|---|
| `open_raid_z_core` | vdev RAID-Z/RAID0-10, pool de stockage, couche de compatibilité ACL NTFS/attributs exFAT, montage réel (Windows = WinFsp `mount.rs` / Linux = FUSE `fuse_mount.rs` ; tout sauf la couche de montage propre à chaque OS est entièrement partagé) |
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
- **Montage réel via WinFsp (Windows)** : peut réellement être monté en tant que lettre de lecteur Windows. Chaque jeu de données du pool apparaît comme son propre fichier, avec prise en charge d'offsets arbitraires en octets pour la lecture/écriture ainsi que la création/suppression/renommage/ajout/troncature de fichiers (l'espace de noms reste plat à la racine — les sous-répertoires ne sont pas encore pris en charge). Vérifié sur du matériel réel : lecture, écriture, création, suppression, renommage, ajout et troncature de fichiers via un lecteur réellement monté.
- **Montage réel via FUSE (Linux)** : le même `Pool` se monte aussi directement sous Linux (`fuse_mount.rs`), avec les mêmes fonctionnalités que la version Windows (création/suppression/renommage/ajout/troncature). Vérifié de bout en bout sous WSL2 Ubuntu 26.04 — monté et exercé via de simples appels `std::fs`. Étant basé sur les inodes, il ne partage pas la limitation connue de la version WinFsp où un autre descripteur ouvert peut continuer à référencer un nom obsolète après un renommage. Le crate `fuser` dispose aussi d'une fonctionnalité `macfuse-4-compat`, ce qui laisse penser que la même conception pourrait s'étendre à macOS (comme volume de données, pas comme disque de démarrage).
- **Prise en charge multilingue** : l'installateur utilise le japonais par défaut avec un sélecteur de langue dans l'interface, modifiable même après l'installation
- **Outil de migration des données existantes (module `migrate`, expérimental)** : copie une arborescence NTFS (ou similaire) existante dans le pool. Il n'écrit jamais dans la source, il peut donc s'exécuter **pendant que Windows continue de fonctionner**. Il ne peut cependant pas convertir **sur place le disque système actuellement utilisé (C:, etc.)** au format RAID sans interruption (un système d'exploitation ne peut pas laisser réécrire par un logiciel qui s'exécute dessus le volume qu'il utilise activement — c'est une contrainte fondamentale, pas une fonctionnalité manquante). C'est strictement un outil qui « copie ailleurs (dans le pool) ». Il s'agit pour l'instant uniquement d'une fonction de bibliothèque, sans CLI/GUI ; les sous-répertoires sont aplatis sur un seul niveau à l'aide d'un caractère séparateur.

## Capacité et limites de taille de fichier

- La taille logique d'un jeu de données (fichier) est suivie de façon cohérente en `u64`, il n'existe donc pas de limite artificielle comme la barrière des 4 Go de FAT32 (le plafond théorique est de 2^64 octets). Les fichiers volumineux comme la vidéo ou les images conviennent tant qu'ils respectent les contraintes réelles ci-dessous.
- La limite réelle est la **capacité libre du pool** — la somme de la capacité utilisable des disques connectés, moins la surcharge de redondance de chaque niveau RAID. Par exemple, avec RAID-Z2 (double parité), la limite effective correspond à peu près à la capacité combinée des disques de données.
- Un seul appel de lecture/écriture WinFsp est plafonné à environ 4 Gio (`u32`) par l'API Windows elle-même, mais c'est la même contrainte que pour tout système de fichiers réel — le système d'exploitation/l'application fractionne automatiquement les transferts plus importants en plusieurs appels, ce n'est donc pas une limite pratique.
- En raison du copy-on-write, chaque écriture (création, ajout ou écrasement) nécessite toujours au moins une bande libre disponible dans le pool (la même idée que le `slop space` de ZFS). Remplir le pool à 100 % de sa capacité signifie que même l'écrasement de données existantes échouera. En pratique, toujours laisser quelques pourcents du pool libres.

## Limitations actuelles (stade prototype)

- Le montage WinFsp ne prend en charge qu'un espace de noms plat à la racine. Les sous-répertoires ne sont pas pris en charge (create/delete/rename par fichier le sont).
- Les lectures/écritures passent par `Pool::read_unaligned`/`Pool::write_unaligned_growing` (une couche read-modify-write) et prennent en charge des offsets/longueurs arbitraires en octets ; une écriture qui dépasse la taille actuelle agrandit automatiquement le fichier (voir « Capacité et limites de taille de fichier » ci-dessus pour la capacité et les considérations liées au PATH).
- `Pool` prend en charge à la fois `RaidZVdev` et `Raid10Vdev`, mais l'intégration de RAID10 avec l'API de jeu de données reste superficielle à certains endroits.
- Le code du montage réel WinFsp (`mount.rs`) ne peut pas être compilé avec une chaîne d'outils Rust antérieure à la 1.85, car le crate `winfsp` requiert la fonctionnalité Cargo `edition2024` (voir Compilation et tests ci-dessous).
- `mount.rs` et l'implémentation GPU de `zfs_accel_hlsl` (fonctionnalité `gpu`) dépendent du crate `windows`, dont le contenu est entièrement vide sauf si la cible de compilation est réellement Windows. Ce code ne peut donc être compilé et testé que sur une véritable machine Windows (ou en compilation croisée vers une cible Windows) ; sous Linux/macOS, il ne compile qu'une fois ces fonctionnalités désactivées via `--no-default-features`.
- Renommer (`rename`) un fichier alors qu'un autre descripteur ouvert le pointe encore peut rendre cet autre descripteur défaillant pour les opérations suivantes (`FileHandle` conserve le nom directement par conception — voir la documentation de `Pool::rename_dataset` pour plus de détails).

## Compilation et tests

```powershell
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features   # sans montage WinFsp ni accélération GPU (logique CPU pure ; ni dxc ni le SDK WinFsp ne sont nécessaires)
cargo test                         # par défaut (inclut le montage réel WinFsp et l'accélération GPU/NPU ; nécessite WinFsp + dxc)
```

`--no-default-features` désactive à la fois les fonctionnalités `winfsp_backend` et `gpu_accel`, ce qui permet de vérifier la logique principale — RAID0/1/5/6/10/Z2/Z3, sommes de contrôle auto-réparatrices, CoW, instantanés/clones, resilver, etc. — de façon indépendante du système d'exploitation (fonctionne aussi sous Linux/macOS). Ni WinFsp, ni le DirectX Shader Compiler (dxc), ni du matériel GPU/NPU ne sont nécessaires.

La compilation avec les fonctionnalités par défaut (`winfsp_backend` + `gpu_accel`) nécessite :

- Le runtime WinFsp (https://winfsp.dev/) installé sur le système (les en-têtes SDK utilisés lors de la compilation sont fournis automatiquement, aucune installation séparée du composant développeur n'est donc nécessaire).
- `dxc` (le DirectX Shader Compiler, fourni avec le Windows SDK ou le Vulkan SDK) dans le `PATH` (utilisé pour compiler les shaders HLSL de parité RAID-Z/Z2 lors de la compilation).
- **Rust 1.85 ou supérieur** (la version dans laquelle `edition2024`, requise par le crate `winfsp`, a été stabilisée ; avec des chaînes d'outils plus anciennes, l'analyse du manifeste `Cargo.toml` échoue déjà).

Il est également possible de désactiver WinFsp ou dxc séparément (ex. `--no-default-features --features gpu_accel` pour le GPU seul, sans WinFsp).

**Remarque pour exécuter réellement les tests `winfsp_backend` (montage réel)** : le crate `winfsp` charge dynamiquement la DLL WinFsp (`winfsp-x64.dll`) via `LoadLibraryW`, qui ne recherche que dans le chemin de recherche DLL standard (le dossier de l'exécutable, `System32` et `PATH`). Dans les environnements où l'installateur WinFsp ne s'est pas ajouté au `PATH`, la compilation réussit sans problème (aucun en-tête du SDK WinFsp n'est nécessaire), mais l'exécution **échoue toujours au runtime** (erreur `WIN32(1285)` = `ERROR_DELAY_LOAD_FAILED`). Ajoutez le répertoire `bin` de WinFsp au `PATH` uniquement pour l'exécution des tests :

```powershell
$env:PATH = "C:\Program Files (x86)\WinFsp\bin;$env:PATH"
cargo test --features winfsp_backend,gpu_accel
```

Sans cela, `mount_pool` renvoie une `Err`, et le test la traite comme un problème dépendant de l'environnement, affichant un message de saut via `eprintln` puis se terminant tôt. **Sans `--nocapture`, ce saut s'affiche quand même comme `ok`, indiscernable d'un montage+lecture/écriture réellement réussi.** Toujours passer `--nocapture` lors de la vérification des tests de montage réel, et vérifier visuellement qu'aucun message de saut n'apparaît.

### Compilation et tests de la version Linux (FUSE)

```bash
# Sous Ubuntu/Debian : build-essential, pkg-config et libfuse3-dev sont nécessaires.
sudo apt-get install -y build-essential pkg-config libfuse3-dev

cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features --features fuse_backend
```

La fonctionnalité `fuse_backend` active le crate `fuser` (une véritable liaison vers `libfuse3` de Linux). Elle est indépendante de `winfsp_backend`/`gpu_accel` et ne peut pas être activée sur des cibles non Linux, puisque `fuser` lui-même n'y est même pas une dépendance (il se trouve sous `target.'cfg(target_os = "linux")'.dependencies` dans `Cargo.toml`). Le test d'intégration à montage réel (`tests/fuse_mount.rs`) a été vérifié sous WSL2 Ubuntu 26.04 : création, écriture, lecture, renommage, troncature, suppression, et l'aller-retour d'un fichier plus volumineux s'étendant sur plusieurs bandes. Si vous êtes uniquement sous Windows, WSL2 (`wsl --install`) est le moyen recommandé de compiler/tester la cible Linux.

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
