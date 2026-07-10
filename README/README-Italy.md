# open-raid-z

Un progetto sperimentale di file system per Windows/Linux, quasi compatibile con NTFS/exFAT, che offre funzionalità in stile ZFS (checksum autoriparanti, pool di archiviazione, copy-on-write, snapshot/cloni) insieme a RAID0/1/5/6/10/Z2/Z3. La logica principale è un unico programma condiviso indipendente dal sistema operativo (`open_raid_z_core`); la build Windows (WinFsp) e quella Linux (FUSE) differiscono solo per il sottile livello di montaggio soprastante (distribuite con i nomi `open-raid-z-win`/`open-raid-z-linux`).

Lingua: [日本語](README-Japan.md) | [UK English](README-UK-English.md) | [US English](README-US-English.md) | **Italiano** | [Français](README-France.md) | [Deutsch](README-Germany.md) | [Русский](README-Russia.md) | [Українська](README-Ukraine.md) | [العربية](README-Arabic.md) | [فارسی](<README-Iran(Persian).md>)

## Un messaggio per Microsoft e Apple

Stiamo sviluppando questo file system sperimentale per portare su Windows funzionalità complete in stile ZFS (checksum autoriparanti, RAID6/RAID-Z2, snapshot e altro). Uno dei nostri obiettivi a lungo termine è che questo file system possa un giorno essere selezionabile come destinazione di installazione ufficiale e disco di avvio su Windows e macOS.

Comprendiamo che ciò richiede la collaborazione di ciascun fornitore di sistema operativo — firma/certificazione dei driver di avvio, supporto ufficiale nell'installer e così via. Se aveste interesse in questo impegno, accoglieremmo con grande favore un vostro contatto e la vostra collaborazione. Si tratta di un progetto piccolo e indipendente, ma desideriamo sinceramente veder realizzata questa tecnologia.

## Convenzione di denominazione

Gli identificatori definiti da questo progetto — nomi di directory, nomi di crate, nomi di pacchetti npm, nomi delle feature Cargo, id/classi HTML/CSS, ecc. — usano in modo coerente **il trattino basso (`_`) invece del trattino (`-`)** (es. `open_raid_z_core`, `zfs_accel_hlsl`, `open_runo_installer`, `open_runo_installer_core`, e le feature Cargo `winfsp_backend`/`gpu_accel`). I nomi che in precedenza usavano il trattino, come `openzfs-winfsp-bridge`, sono stati rinominati per coerenza all'interno del progetto.

Sono esclusi i seguenti casi, poiché seguono specifiche esterne o convenzioni dell'ecosistema e non la denominazione scelta da questo progetto:

- Il nome del repository stesso (`open-raid-z`; è il nome reale del repository su GitHub e non può essere cambiato)
- Gli attributi personalizzati HTML5 `data-*` (`data-i18n`; il trattino è richiesto dalla specifica)
- I nomi dei pacchetti npm esterni (es. `@tauri-apps/api`, i nomi reali dei pacchetti pubblicati)
- I nomi delle proprietà CSS (es. `font-family`; è la sintassi stessa del linguaggio CSS)
- I termini composti inglesi che contengono realmente un trattino, come Reed-Solomon o copy-on-write

## Componenti

| Componente | Ruolo |
|---|---|
| `open_raid_z_core` | vdev RAID-Z/RAID0-10, pool di archiviazione, livello di compatibilità ACL NTFS/attributi exFAT, montaggio reale (Windows = WinFsp `mount.rs` / Linux = FUSE `fuse_mount.rs`; tutto tranne il livello di montaggio specifico per OS è pienamente condiviso) |
| `zfs_accel_hlsl` | Offload del calcolo della parità su acceleratori hardware GPU/NPU (DirectX 12 Compute + DirectML) |
| `open_runo_installer_core` | Logica indipendente dal sistema operativo per il rilevamento dei dischi, il consulente di configurazione in stile Copilot e l'anteprima di inizializzazione zpool (nessuna dipendenza da Tauri; `cargo test` funziona anche su Linux/macOS) |
| `open_runo_installer` | L'installer Tauri vero e proprio (un sottile livello UI che richiama `open_runo_installer_core`): rilevamento hardware, wizard di inizializzazione zpool, interfaccia del consulente di configurazione in stile Copilot |

## Funzionalità principali

- **Intera serie RAID**: RAID0 / RAID1 (mirror) / RAID5 / RAID6 / RAID10 (mirror in striping) / RAID-Z2 / RAID-Z3
- **Partizionamento e riutilizzo del disco**: dividere un disco fisico e usare metà come membro di un mirror mentre l'altra metà entra in un array RAID6/Z2 separato
- **Checksum autoriparanti, copy-on-write, snapshot/cloni**: emulano l'approccio di ZFS. `Pool::scrub` può rilevare e riparare in un'unica passata la corruzione silenziosa sull'intero pool, tramite la stessa API sia con backend RAID-Z che RAID10
- **Compatibilità NTFS**: traduzione ACL (NFSv4 ⇔ NTFS), mappatura UID/GID ⇔ SID (mappatura deterministica basata su RID per domini SAM locali/AD)
- **Compatibilità exFAT**: conversione di attributi file e timestamp, supporto per file/volumi superiori a 4GB
- **Accelerazione hardware GPU/NPU**: la generazione della parità RAID-Z1/Z2/Z3 viene inviata tramite DirectX 12 Compute + DirectML (ripiego automatico su CPU se non è presente hardware). Implementa anche uno schema che converte la moltiplicazione per coefficiente GF(2^8) in una matrice di bit GF(2), riducendola a un'unica dispatch GEMM di DirectML (`zfs_accel_hlsl::dml_gemm`), verificata corretta su hardware GPU reale (non ancora verificata su hardware NPU reale). Questo stesso meccanismo è collegato al calcolo di ricostruzione eseguito da scrub/resilver quando viene rilevata una corruzione (cioè il controllo di parità). Sono inoltre previsti percorsi shader dedicati alle NPU (`raidnpu_*.hlsl`), in vista di future verifiche/ottimizzazioni su hardware NPU reale
- **Accelerazione Vulkan Compute per le piattaforme non Windows**: DirectX/DirectML è un'API esclusiva di Windows, quindi è stata aggiunta un'implementazione tramite Vulkan Compute basata sul crate `ash`, pensata per Linux/Mac/Android (`zfs_accel_hlsl::vulkan_compute`, feature `vulkan`). La generazione della parità XOR di RAID-Z1 è stata verificata funzionare correttamente su hardware GPU reale (NVIDIA GeForce GT 730, Vulkan 1.2)
- **Ponte di lettura/scrittura per formati esterni (`foreign_fs`)**: distinto dal formato di pool proprio di open-raid-z, consente di leggere e scrivere volumi FAT32/FAT16 già esistenti creati da altri sistemi operativi (chiavette USB, microSD, schede CF, ecc.), e di leggere volumi exFAT (al momento in sola lettura, per un limite del crate a monte). Operabile tramite `orzctl foreign` (`ls`/`cat`/`put`)
- **Pannello "Stato di compatibilità" nell'installer**: apribile e richiudibile con un pulsante, mostra lo stato di supporto del sistema operativo corrente, ogni GPU/NPU rilevata (con riconoscimento del fornitore: Intel/AMD/NVIDIA/Qualcomm, con supporto a dispositivi multipli) e i tipi di supporto di archiviazione rilevati (HDD/SSD/NVMe/USB/SD/CF)
- **Applicazione dello zpool su disco reale**: la procedura guidata di inizializzazione zpool dell'installer dispone ora di un comando (`init_zpool_apply`) che si applica ai dischi fisici reali (`\\.\PhysicalDriveN`), non solo alle anteprime su immagine temporanea. Protetto da un flag di conferma esplicito per la cancellazione dei dati esistenti
- **Consulente di configurazione in stile Copilot**: consiglia un livello RAID in base alla disposizione dei dischi, all'acceleratore e al numero di core CPU (prima versione euristica; è presente anche uno scheletro di rilevamento LLM locale). La logica risiede in `open_runo_installer_core`, indipendente da Tauri, e può essere verificata con `cargo test` anche su Linux/macOS
- **Montaggio reale via WinFsp (Windows)**: può essere effettivamente montato come lettera di unità Windows. Ogni dataset del pool appare come un proprio file, con supporto a offset arbitrari in byte per lettura/scrittura e a creazione/eliminazione/rinomina/append/troncamento dei file (resta uno spazio dei nomi piatto nella radice — le sottodirectory non sono ancora supportate). Verificato su hardware reale: lettura, scrittura, creazione, eliminazione, rinomina, append e troncamento di file tramite un'unità effettivamente montata.
- **Montaggio reale via FUSE (Linux)**: lo stesso `Pool` si monta direttamente anche su Linux (`fuse_mount.rs`), con le stesse funzionalità della build Windows (creazione/eliminazione/rinomina/append/troncamento). Verificato end-to-end su WSL2 Ubuntu 26.04, montato ed esercitato tramite normali chiamate `std::fs`. Essendo basato su inode, non condivide la limitazione nota della build WinFsp per cui un altro handle aperto può continuare a fare riferimento a un nome obsoleto dopo una rinomina. Il crate `fuser` ha anche una feature `macfuse-4-compat`, quindi lo stesso design potrebbe in futuro estendersi a macOS (come volume dati, non come disco di avvio).
- **Supporto multilingua**: l'installer (OpenRaidZ Installer) usa l'inglese come lingua predefinita con un selettore di lingua (9 lingue: inglese, giapponese, italiano, francese, tedesco, russo, ucraino, arabo, persiano) nell'interfaccia, modificabile anche dopo l'installazione. La visualizzazione ibrida (lingua principale + una seconda lingua selezionabile, giapponese per impostazione predefinita) è attiva di default, mostrando entrambe le lingue affiancate
- **Strumento di migrazione dei dati esistenti (modulo `migrate`, sperimentale)**: copia un albero di directory NTFS (o simile) esistente nel pool. Non scrive mai sulla sorgente, quindi può essere eseguito **mentre Windows resta in esecuzione**. Non può però convertire **sul posto il disco di sistema attualmente in uso (C: ecc.)** in formato RAID senza interruzioni (un sistema operativo non può far riscrivere da un software in esecuzione su di esso il volume che sta attivamente usando — è un vincolo intrinseco, non una funzionalità mancante). È rigorosamente uno strumento che "copia altrove (nel pool)". Al momento è solo una funzione di libreria, senza CLI/GUI; le sottodirectory vengono appiattite su un livello tramite un carattere separatore.
- **Persistenza dei metadati (`Pool::save`/`Pool::open`)**: l'elenco dei dataset, le allocazioni delle stripe, gli snapshot e altre informazioni di gestione possono ora essere salvati e ripristinati da un'area riservata (superblocco) all'interno del pool. In precedenza questo meccanismo non esisteva: i byte di dati grezzi sopravvivevano sul disco, ma il registro di quale file si trovasse dove andava perso non appena il processo (il montaggio) terminava. Sia la build Windows (WinFsp) sia quella Linux (FUSE) ora salvano automaticamente a ogni operazione di modifica, ed è stato verificato su hardware reale che i file sopravvivono realmente a uno smontaggio e rimontaggio effettivi.

## Capacità e limiti di dimensione dei file

- La dimensione logica di un dataset (file) è tracciata in modo coerente come `u64`, quindi non esiste un limite artificiale come il confine dei 4GB di FAT32 (il limite teorico è 2^64 byte). File di grandi dimensioni come video o immagini vanno bene, purché rientrino nei vincoli reali descritti sotto.
- Il limite reale è la **capacità libera del pool** — la somma della capacità utilizzabile dei dischi collegati, meno l'overhead di ridondanza di ciascun livello RAID. Ad esempio, con RAID-Z2 (doppia parità), il limite effettivo è all'incirca la capacità combinata dei dischi dati.
- Una singola chiamata di lettura/scrittura WinFsp è limitata a circa 4GiB (`u32`) dall'API Windows stessa, ma è lo stesso vincolo che ha qualsiasi file system reale — il sistema operativo/l'applicazione suddivide automaticamente i trasferimenti più grandi in più chiamate, quindi non è un limite pratico.
- A causa del copy-on-write, ogni scrittura (creazione, append o sovrascrittura) richiede sempre almeno una stripe libera disponibile nel pool (la stessa idea dello `slop space` di ZFS). Un'ulteriore stripe è inoltre riservata in modo permanente per l'archiviazione dei metadati. Riempire il pool al 100% della capacità significa che anche sovrascrivere dati esistenti fallirà. In pratica, lasciare sempre libera qualche percentuale del pool.

## Limitazioni attuali (fase prototipo)

- Il montaggio WinFsp supporta solo uno spazio dei nomi piatto nella radice. Le sottodirectory non sono supportate (create/delete/rename per singolo file sono invece supportate).
- Le letture/scritture passano attraverso `Pool::read_unaligned`/`Pool::write_unaligned_growing` (un livello read-modify-write) e supportano offset/lunghezze arbitrarie in byte; una scrittura che supera la dimensione attuale fa crescere automaticamente il file (vedi "Capacità e limiti di dimensione dei file" sopra per capacità e considerazioni sul PATH).
- `Pool` supporta sia `RaidZVdev` che `Raid10Vdev`, ma l'integrazione di RAID10 con l'API dataset è ancora superficiale in alcuni punti.
- Il codice del montaggio reale WinFsp (`mount.rs`) non può essere compilato con una toolchain Rust precedente alla 1.85, perché il crate `winfsp` richiede la feature Cargo `edition2024` (vedi Build e test più sotto).
- `mount.rs` e l'implementazione GPU di `zfs_accel_hlsl` (feature `gpu`) dipendono dal crate `windows`, il cui contenuto è completamente vuoto a meno che il target di compilazione non sia effettivamente Windows. Di conseguenza questo codice può essere compilato e testato solo su una macchina Windows reale (o compilando in cross per un target Windows); su Linux/macOS si compila solo disabilitandoli con `--no-default-features`.
- Rinominare (`rename`) un file mentre un altro handle aperto lo punta ancora può lasciare quell'altro handle non funzionante per le operazioni successive (`FileHandle` mantiene il nome direttamente per design — vedi la documentazione di `Pool::rename_dataset` per i dettagli).

## Build e test

```powershell
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features   # senza montaggio WinFsp/accelerazione GPU (sola logica CPU; non servono né dxc né l'SDK WinFsp)
cargo test                         # predefinito (include montaggio reale WinFsp e accelerazione GPU/NPU; richiede WinFsp + dxc)
```

`--no-default-features` disabilita entrambe le feature `winfsp_backend` e `gpu_accel`, permettendo di verificare la logica principale — RAID0/1/5/6/10/Z2/Z3, checksum autoriparanti, CoW, snapshot/cloni, resilver, ecc. — in modo indipendente dal sistema operativo (funziona anche su Linux/macOS). Non servono WinFsp, il DirectX Shader Compiler (dxc), né hardware GPU/NPU.

Compilare con le feature predefinite (`winfsp_backend` + `gpu_accel`) richiede:

- Il runtime WinFsp (https://winfsp.dev/) installato sul sistema (gli header SDK usati in fase di build sono forniti automaticamente in bundle, quindi non è necessaria l'installazione separata del componente per sviluppatori).
- `dxc` (il DirectX Shader Compiler, incluso nel Windows SDK o nel Vulkan SDK) nel `PATH` (usato per compilare in fase di build gli shader HLSL di parità RAID-Z/Z2).
- **Rust 1.85 o successivo** (la versione in cui `edition2024`, richiesta dal crate `winfsp`, è stata stabilizzata; con toolchain più vecchie fallisce già l'analisi del manifest `Cargo.toml`).

È anche possibile disabilitare singolarmente WinFsp o dxc (es. `--no-default-features --features gpu_accel` per solo GPU, senza WinFsp).

**Nota per eseguire realmente i test `winfsp_backend` (montaggio reale)**: il crate `winfsp` carica dinamicamente la DLL di WinFsp (`winfsp-x64.dll`) tramite `LoadLibraryW`, che cerca solo nel percorso di ricerca DLL standard (la cartella dell'eseguibile, `System32` e `PATH`). Negli ambienti in cui l'installer di WinFsp non si è aggiunto al `PATH`, la build va a buon fine (non servono gli header dell'SDK WinFsp) ma l'esecuzione **fallisce sempre a runtime** (errore `WIN32(1285)` = `ERROR_DELAY_LOAD_FAILED`). Aggiungere la cartella `bin` di WinFsp al `PATH` solo per l'esecuzione dei test:

```powershell
$env:PATH = "C:\Program Files (x86)\WinFsp\bin;$env:PATH"
cargo test --features winfsp_backend,gpu_accel
```

Senza questo, `mount_pool` restituisce un `Err`, e il test lo tratta come un problema dipendente dall'ambiente, stampando un messaggio di skip via `eprintln` e uscendo subito. **Senza `--nocapture`, questo skip appare comunque come `ok`, indistinguibile da un montaggio+lettura/scrittura effettivamente riuscito.** Usare sempre `--nocapture` quando si verificano i test di montaggio reale, e controllare visivamente che non compaia alcun messaggio di skip.

### Build e test della build Linux (FUSE)

```bash
# Su Ubuntu/Debian: servono build-essential, pkg-config e libfuse3-dev.
sudo apt-get install -y build-essential pkg-config libfuse3-dev

cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features --features fuse_backend
```

La feature `fuse_backend` abilita il crate `fuser` (un binding reale a `libfuse3` di Linux). È indipendente da `winfsp_backend`/`gpu_accel` e non può essere abilitata su target non Linux, poiché `fuser` stesso non è nemmeno una dipendenza lì (si trova sotto `target.'cfg(target_os = "linux")'.dependencies` in `Cargo.toml`). Il test di integrazione con montaggio reale (`tests/fuse_mount.rs`) è stato verificato su WSL2 Ubuntu 26.04: creazione, scrittura, lettura, rinomina, troncamento, eliminazione, il roundtrip di un file più grande che attraversa più stripe e la sopravvivenza dei metadati a uno smontaggio e rimontaggio reali. Se si è solo su Windows, WSL2 (`wsl --install`) è il modo consigliato per compilare/testare il target Linux.

È inclusa anche una piccola CLI `orzctl` per creare e montare un pool direttamente dalla riga di comando:

```bash
cargo build --no-default-features --features fuse_backend --bin orzctl
./target/debug/orzctl create --level z2 --chunk-size 4096 --stripes 1000 --dataset tank /path/to/disk0 /path/to/disk1 ...
./target/debug/orzctl mount  --level z2 --chunk-size 4096 --stripes 1000 --mountpoint /mnt/tank /path/to/disk0 /path/to/disk1 ...
```

Per il montaggio automatico all'avvio, registrare
[`contrib/systemd/open-raid-z-pool.service.example`](../open_runo_zfs_source/open_raid_z_core/contrib/systemd/open-raid-z-pool.service.example)
come unità systemd (verificato su una VM VirtualBox: un pool creato su 4
dispositivi a blocchi realmente separati si monta automaticamente anche
dopo un riavvio reale).

### Installer (`open_runo_installer` / `open_runo_installer_core`)

```powershell
# Livello logico (nessuna dipendenza da Tauri; funziona anche su Linux/macOS)
cd open_runo_zfs_source/open_runo_installer_core
cargo test                    # solo fallback CPU (predefinito)
cargo test --features gpu     # include il dispatch reale GPU/NPU (richiede una macchina Windows reale + dxc)

# Frontend (TypeScript, indipendente dal sistema operativo)
cd open_runo_zfs_source/open_runo_installer
npm install
npx tsc --noEmit               # solo controllo dei tipi
npx vite build                 # build effettiva

# L'app Tauri vera e propria (richiede una macchina Windows reale, oppure un Rust sufficientemente recente più le dipendenze desktop di Linux)
cd open_runo_zfs_source/open_runo_installer/src-tauri
cargo tauri dev / cargo tauri build
```

`open_runo_installer_core` (rilevamento dischi, consulente di configurazione in stile Copilot, anteprima di inizializzazione zpool) è un crate indipendente senza dipendenza da Tauri, quindi la sua logica può essere verificata così com'è anche in ambienti privi di ciò che serve a Tauri stesso per compilare (una WebView, GTK, ecc., più una toolchain Rust sufficientemente recente). Solo l'effettiva enumerazione dei dischi (`\\.\PhysicalDriveN`) usa un'API esclusiva di Windows, ed è isolata dietro `#[cfg(windows)]`; tutto il resto (consulente di configurazione e calcoli dell'anteprima zpool) è indipendente dal sistema operativo, e tutti i suoi 26 test risultano superati.

## Roadmap: interoperabilità multi-OS e formati esistenti

L'obiettivo del progetto è rendere lo stesso open-raid-z leggibile e scrivibile su Windows/Mac/Linux/Android/iOS/iPad, e farlo interoperare con i formati già esistenti di altri sistemi operativi (NTFS/exFAT/FAT32/ext4/APFS, ecc.). La fattibilità attuale, le priorità e i vincoli tecnici — in particolare il fatto che su iOS/iPad Apple non consente a terze parti di implementare un RAID a livello di dispositivo a blocchi, per cui lì il supporto sarà probabilmente limitato alla sola navigazione tramite una File Provider Extension — sono documentati in [`MULTIPLATFORM_ROADMAP.md`](open_runo_zfs_source/open_raid_z_core/contrib/systemd/MULTIPLATFORM_ROADMAP.md). L'accelerazione GPU/NPU adotterà progressivamente l'API nativa di ciascun sistema operativo dove DirectX non è disponibile (ad es. Metal Performance Shaders su Mac, NNAPI su Android). Si sta inoltre valutando, come obiettivo futuro, l'interoperabilità con formati RAID di terze parti (ad es. mdadm su Linux, Storage Spaces su Windows).

## Licenza

MPL-2.0
