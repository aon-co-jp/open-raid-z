# open-raid-z

Un progetto sperimentale di file system per Windows, quasi compatibile con NTFS/exFAT, che offre funzionalità in stile ZFS (checksum autoriparanti, pool di archiviazione, copy-on-write, snapshot/cloni) insieme a RAID0/1/5/6/10/Z2/Z3.

Lingua: [日本語](README-Japan.md) | [UK English](README-UK-English.md) | [US English](README-US-English.md) | **Italiano** | [Français](README-France.md) | [Deutsch](README-Germany.md) | [Русский](README-Russia.md) | [Українська](README-Ukraine.md) | [العربية](README-Arabic.md) | [فارسی](<README-Iran(Persian).md>)

## Componenti

| Componente | Ruolo |
|---|---|
| `openzfs-winfsp-bridge` | vdev RAID-Z/RAID0-10, pool di archiviazione, livello di compatibilità ACL NTFS/attributi exFAT, montaggio reale via WinFsp |
| `zfs-accel-hlsl` | Offload del calcolo della parità su acceleratori hardware GPU/NPU (DirectX 12 Compute + DirectML) |
| `openruno-installer` | Installer Tauri: rilevamento hardware, wizard di inizializzazione zpool, consulente di configurazione in stile Copilot |

## Funzionalità principali

- **Intera serie RAID**: RAID0 / RAID1 (mirror) / RAID5 / RAID6 / RAID10 (mirror in striping) / RAID-Z2 / RAID-Z3
- **Partizionamento e riutilizzo del disco**: dividere un disco fisico e usare metà come membro di un mirror mentre l'altra metà entra in un array RAID6/Z2 separato
- **Checksum autoriparanti, copy-on-write, snapshot/cloni**: emulano l'approccio di ZFS
- **Compatibilità NTFS**: traduzione ACL (NFSv4 ⇔ NTFS), mappatura UID/GID ⇔ SID (mappatura deterministica basata su RID per domini SAM locali/AD)
- **Compatibilità exFAT**: conversione di attributi file e timestamp, supporto per file/volumi superiori a 4GB
- **Accelerazione hardware GPU/NPU**: il calcolo della parità RAID-Z1/Z2 viene effettivamente inviato tramite DirectX 12 Compute + DirectML (ripiego automatico su CPU se non è presente hardware)
- **Consulente di configurazione in stile Copilot**: consiglia un livello RAID in base alla disposizione dei dischi, all'acceleratore e al numero di core CPU (prima versione euristica; è presente anche uno scheletro di rilevamento LLM locale)
- **Montaggio reale via WinFsp (prototipo)**: può essere effettivamente montato come lettera di unità Windows (attualmente una build minima a file singolo)
- **Supporto multilingua**: l'installer usa il giapponese come lingua predefinita con un selettore di lingua nell'interfaccia, modificabile anche dopo l'installazione

## Limitazioni attuali (fase prototipo)

- Il montaggio WinFsp supporta solo uno spazio dei nomi piatto (un unico file fisso `\pool.dat` nella radice). Nessuna gerarchia di directory o file multipli per ora.
- Le letture/scritture devono essere allineate al confine di chunk del dataset.
- `Pool` supporta sia `RaidZVdev` che `Raid10Vdev`, ma l'integrazione di RAID10 con l'API dataset è ancora superficiale in alcuni punti.

## Build e test

```powershell
cd open-runo-zfs-source/openzfs-winfsp-bridge
cargo test --no-default-features        # senza il montaggio WinFsp
cargo test --features winfsp-backend    # con il montaggio reale WinFsp (richiede il runtime WinFsp)
```

Il runtime WinFsp (https://winfsp.dev/) deve essere installato sul sistema (gli header SDK usati in fase di build sono forniti automaticamente in bundle, quindi non è necessaria l'installazione separata del componente per sviluppatori).

## Licenza

MPL-2.0
