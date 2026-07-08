# open-raid-z

An experimental filesystem project for Windows that stays near-compatible with NTFS/exFAT while providing ZFS-style features (self-healing checksums, storage pools, copy-on-write, snapshots/clones) alongside RAID0/1/5/6/10/Z2/Z3.

Language: [日本語](README-Japan.md) | **UK English** | [US English](README-US-English.md) | [Italiano](README-Italy.md) | [Français](README-France.md) | [Deutsch](README-Germany.md) | [Русский](README-Russia.md) | [Українська](README-Ukraine.md) | [العربية](README-Arabic.md) | [فارسی](<README-Iran(Persian).md>)

## Components

| Component | Role |
|---|---|
| `openzfs-winfsp-bridge` | RAID-Z/RAID0-10 vdevs, storage pool, NTFS ACL/exFAT attribute compatibility layer, real WinFsp mount |
| `zfs-accel-hlsl` | Parity computation offload to GPU/NPU hardware accelerators (DirectX 12 Compute + DirectML) |
| `openruno-installer` | Tauri installer: hardware detection, zpool-init wizard, Copilot-style configuration advisor |

## Key features

- **Full RAID series**: RAID0 / RAID1 (mirror) / RAID5 / RAID6 / RAID10 (striped mirrors) / RAID-Z2 / RAID-Z3
- **Disk partitioning and reuse**: split one physical disk and use one half as a mirror member while the other half joins a separate RAID6/Z2 array
- **Self-healing checksums, copy-on-write, snapshots/clones**: emulating ZFS's approach
- **NTFS compatibility**: ACL translation (NFSv4 ⇔ NTFS), UID/GID ⇔ SID mapping (deterministic RID-based mapping for local SAM/AD domains)
- **exFAT compatibility**: file attribute and timestamp conversion, support for files/volumes beyond 4GB
- **GPU/NPU hardware acceleration**: RAID-Z1/Z2 parity computation is actually dispatched via DirectX 12 Compute + DirectML (falls back to CPU automatically when no hardware is present)
- **Copilot-style configuration advisor**: recommends a RAID level from disk layout, accelerator, and CPU core count (heuristic first pass; a local-LLM detection skeleton is also in place)
- **Real WinFsp mount (prototype)**: can actually be mounted as a Windows drive letter (currently a minimal single-file build)
- **Multilingual support**: the installer defaults to Japanese with a UI language switcher, changeable after installation too

## Current limitations (prototype stage)

- The WinFsp mount only supports a flat namespace (one fixed file `\pool.dat` at the root). No directory hierarchy or multiple files yet.
- Reads/writes must be aligned to the dataset's chunk boundary.
- `Pool` supports both `RaidZVdev` and `Raid10Vdev`, but RAID10's integration with the dataset API is still shallow in places.

## Build & test

```powershell
cd open-runo-zfs-source/openzfs-winfsp-bridge
cargo test --no-default-features        # without the WinFsp mount
cargo test --features winfsp-backend    # with the real WinFsp mount (requires the WinFsp runtime)
```

The WinFsp runtime (https://winfsp.dev/) must be installed on the system (the SDK headers used at build time are vendored automatically, so no separate developer-component install is required).

## Licence

MPL-2.0
