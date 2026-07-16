# open-raid-z

An experimental filesystem project for Windows/Linux that stays near-compatible with NTFS/exFAT while providing ZFS-style features (self-healing checksums, storage pools, copy-on-write, snapshots/clones) alongside RAID0/1/5/6/10/Z2/Z3. The core logic is a single OS-independent shared program (`open_raid_z_core`); the Windows build (WinFsp) and Linux build (FUSE) differ only in the thin mount layer on top of it (distributed under the names `open-raid-z-win`/`open-raid-z-linux`).

Language: [日本語](README-Japan.md) | **UK English** | [US English](README-US-English.md) | [Italiano](README-Italy.md) | [Français](README-France.md) | [Deutsch](README-Germany.md) | [Русский](README-Russia.md) | [Українська](README-Ukraine.md) | [العربية](README-Arabic.md) | [فارسی](<README-Iran(Persian).md>)

## A note to Microsoft and Apple

We are building this experimental filesystem to bring full ZFS-style capabilities (self-healing checksums, RAID6/RAID-Z2, snapshots, and more) to Windows. One of our long-term goals is for this filesystem to eventually be selectable as an official installation target and boot drive on Windows and macOS.

We understand this requires cooperation from each OS vendor — boot-start driver signing/certification, official installer support, and so on. If you have any interest in this effort, we would greatly welcome your outreach and collaboration. This is a small, independent project, but we are genuinely committed to seeing this technology through.

## Naming convention

Identifiers defined by this project itself — directory names, crate names, npm package names, Cargo feature names, HTML/CSS ids/classes, and so on — are consistently **underscore-separated (`_`) rather than hyphen-separated (`-`)** (e.g. `open_raid_z_core`, `zfs_accel_hlsl`, `open_runo_installer`, `open_runo_installer_core`, and the Cargo features `winfsp_backend`/`gpu_accel`). Names that used to be hyphenated, such as `openzfs-winfsp-bridge`, were renamed for consistency within this project.

The following are exempt, because they follow external specifications or ecosystem conventions rather than this project's own naming choices:

- The repository name itself (`open-raid-z`; this is the actual GitHub repository name and cannot be changed)
- HTML5 `data-*` custom attributes (`data-i18n`; the hyphen is mandated by the spec)
- External npm package names (e.g. `@tauri-apps/api`, the actual published package names)
- CSS property names (e.g. `font-family`; this is the CSS language syntax itself)
- English compound terms that genuinely contain a hyphen, such as Reed-Solomon or copy-on-write

## Components

| Component | Role |
|---|---|
| `open_raid_z_core` | RAID-Z/RAID0-10 vdevs, storage pool, NTFS ACL/exFAT attribute compatibility layer, real mount (Windows = WinFsp `mount.rs` / Linux = FUSE `fuse_mount.rs`; everything but the per-OS mount layer is fully shared) |
| `zfs_accel_hlsl` | Parity computation offload to GPU/NPU hardware accelerators (DirectX 12 Compute + DirectML) |
| `open_runo_installer_core` | OS-independent logic for disk detection, the Copilot-style configuration advisor, and zpool-init preview (no Tauri dependency; `cargo test` also works on Linux/macOS) |
| `open_runo_installer` | The Tauri installer itself (a thin UI layer that calls into `open_runo_installer_core`): hardware detection, zpool-init wizard, Copilot-style configuration advisor UI |

## Key features

- **Full RAID series**: RAID0 / RAID1 (mirror) / RAID5 / RAID6 / RAID10 (striped mirrors) / RAID-Z2 / RAID-Z3
- **Disk partitioning and reuse**: split one physical disk and use one half as a mirror member while the other half joins a separate RAID6/Z2 array
- **Self-healing checksums, copy-on-write, snapshots/clones**: emulating ZFS's approach. `Pool::scrub` can detect and heal silent corruption across the whole pool in one pass, via the same API on both RAID-Z-family and RAID10 backends
- **NTFS compatibility**: ACL translation (NFSv4 ⇔ NTFS), UID/GID ⇔ SID mapping (deterministic RID-based mapping for local SAM/AD domains)
- **exFAT compatibility**: file attribute and timestamp conversion, support for files/volumes beyond 4GB
- **GPU/NPU hardware acceleration**: RAID-Z1/Z2/Z3 parity generation is dispatched via DirectX 12 Compute + DirectML (falls back to CPU automatically when no hardware is present). Also implements a scheme that converts GF(2^8) coefficient multiplication into a GF(2) bit matrix and reduces it to a single DirectML GEMM dispatch (`zfs_accel_hlsl::dml_gemm`), verified correct on real GPU hardware (not yet verified on real NPU hardware). The same mechanism is wired into the reconstruction computation that scrub/resilver runs when corruption is detected (i.e. parity checking). Dedicated NPU shader paths (`raidnpu_*.hlsl`) are also provided, in preparation for future verification/optimisation on real NPU hardware
- **Vulkan Compute acceleration (non-Windows platforms)**: DirectX/DirectML is a Windows-only API, so a Vulkan Compute implementation via the `ash` crate has been added to cover Linux/Mac/Android (`zfs_accel_hlsl::vulkan_compute`, the `vulkan` feature). RAID-Z1 XOR parity generation has been verified to work correctly on real GPU hardware (NVIDIA GeForce GT 730, Vulkan 1.2)
- **Foreign-format read/write bridge (`foreign_fs`)**: separate from open-raid-z's own pool format, this can read and write existing FAT32/FAT16 and exFAT volumes created by other operating systems (USB sticks, microSD, CF cards, and so on) — exFAT write support now works too, via the `hadris-fat` crate. Operable via `orzctl foreign` (`ls`/`cat`/`put`)
- **Installer "Compatibility Status" panel**: a panel, toggled open and closed via a button, showing the current OS's support status, every detected GPU/NPU (with vendor detection for Intel/AMD/NVIDIA/Qualcomm, supporting multiple devices), and the detected storage media types (HDD/SSD/NVMe/USB/SD/CF)
- **Applying zpool to real disks**: the installer's zpool init wizard now has a command (`init_zpool_apply`) that applies to actual physical disks (`\\.\PhysicalDriveN`), not just scratch-image previews. Safety-gated behind an explicit confirmation flag for erasing existing data
- **Copilot-style configuration advisor**: recommends a RAID level from disk layout, accelerator, and CPU core count (heuristic first pass; a local-LLM detection skeleton is also in place). The logic lives in `open_runo_installer_core`, independent of Tauri, and can be verified with `cargo test` on Linux/macOS too
- **Real WinFsp mount (Windows)**: can actually be mounted as a Windows drive letter. Every dataset in the pool shows up as its own file, with arbitrary byte-offset reads/writes and file create/delete/rename/append/truncate all supported (still a flat namespace at the root — subdirectories are not supported yet). Verified on real hardware: reading, writing, creating, deleting, renaming, appending, and truncating files through an actual mounted drive.
- **Real FUSE mount (Linux)**: the same `Pool` mounts directly on Linux too (`fuse_mount.rs`), with the same functionality as the Windows build (create/delete/rename/append/truncate). Verified end-to-end on WSL2 Ubuntu 26.04 — mounted and exercised via plain `std::fs` calls. Because it's inode-based, it doesn't share the WinFsp build's known limitation where another open handle can keep referencing a stale name after a rename. The `fuser` crate also has a `macfuse-4-compat` feature, so the same design could plausibly extend to macOS (as a data volume, not a boot disk) down the road.
- **Multilingual support**: the installer (OpenRaidZ Installer) defaults to English with a 9-language UI switcher (English, Japanese, Italian, French, German, Russian, Ukrainian, Arabic, Persian), changeable after installation too. Hybrid display (the primary language plus a selectable second language, Japanese by default) is enabled by default, showing both side by side
- **Existing-data migration tool (the `migrate` module, experimental)**: copies an existing NTFS (or similar) directory tree into the pool. It never writes to the source, so it can run **while Windows keeps running**. It cannot, however, convert the **currently running system drive (C: etc.) in place** into RAID format without downtime (an OS cannot have the volume it is actively using rewritten by software running on that same OS — this is a hard constraint, not a missing feature). It is strictly a "copy to another location (the pool)" tool. Currently a library function only, with no CLI/GUI yet; subdirectories are flattened to one level using a separator character.
- **Metadata persistence (`Pool::save`/`Pool::open`)**: dataset lists, stripe allocations, snapshots, and other bookkeeping can now be saved to and restored from a reserved area (superblock) inside the pool. Previously there was no such mechanism — the raw data bytes survived on disk, but the record of which file lived where was lost the moment the process (mount) exited. Both the Windows (WinFsp) and Linux (FUSE) builds now auto-save on every mutating operation, and it's been verified on real hardware that files genuinely survive a real unmount + remount.

## Capacity & file-size limits

- A dataset (file)'s logical size is tracked consistently as a `u64`, so there is no artificial limit like FAT32's 4GB boundary (the theoretical ceiling is 2^64 bytes). Large files such as video or images are fine as long as they fit within the actual constraints below.
- The real limit is the **pool's free capacity** — the sum of the connected disks' usable capacity, minus each RAID level's redundancy overhead. For example, with RAID-Z2 (double parity), the effective limit is roughly the combined capacity of the data disks.
- A single WinFsp read/write call is capped at about 4GiB (`u32`) by the Windows API itself, but this is the same constraint any real filesystem has — the OS/application automatically splits larger transfers into multiple calls, so it is not a practical limit.
- Because of copy-on-write, every write (create, append, or overwrite alike) always needs at least one free stripe available in the pool (the same idea as ZFS's `slop space`). One additional stripe is also permanently reserved for metadata storage. Filling the pool to 100% capacity means even overwriting existing data will fail. In practice, always leave a few percent of the pool free.

## Current limitations (prototype stage)

- The WinFsp mount only supports a flat namespace at the root. Subdirectories are not supported (per-file create/delete/rename are supported).
- Reads/writes go through `Pool::read_unaligned`/`Pool::write_unaligned_growing` (a read-modify-write layer) and support arbitrary byte offsets/lengths; a write that exceeds the current size automatically grows the file (see "Capacity & file-size limits" above for capacity and PATH considerations).
- `Pool` supports both `RaidZVdev` and `Raid10Vdev`, but RAID10's integration with the dataset API is still shallow in places.
- The real WinFsp mount code (`mount.rs`) cannot be built on a Rust toolchain older than 1.85, because the `winfsp` crate requires the `edition2024` Cargo feature (see Build & test below).
- `mount.rs` and `zfs_accel_hlsl`'s GPU implementation (the `gpu` feature) depend on the `windows` crate, whose contents are entirely empty unless the compilation target is actually Windows. Consequently this code can only be built and tested on a real Windows machine (or when cross-compiling to a Windows target); on Linux/macOS it only builds once these are disabled via `--no-default-features`.
- Renaming (`rename`) a file while another open handle still points at it can leave that other handle broken for subsequent operations (`FileHandle` holds the name directly by design — see the `Pool::rename_dataset` documentation for details).

## Build & test

```powershell
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features   # without WinFsp/GPU acceleration (pure CPU logic; neither dxc nor the WinFsp SDK is needed)
cargo test                         # default (includes the real WinFsp mount and GPU/NPU acceleration; requires WinFsp + dxc)
```

`--no-default-features` disables both the `winfsp_backend` and `gpu_accel` features, letting you verify the core logic — RAID0/1/5/6/10/Z2/Z3, self-healing checksums, CoW, snapshots/clones, resilver, and so on — in an OS-independent way (Linux/macOS work fine). Neither WinFsp, the DirectX Shader Compiler (dxc), nor GPU/NPU hardware is required.

Building with the default features (`winfsp_backend` + `gpu_accel`) requires:

- The WinFsp runtime (https://winfsp.dev/) installed on the system (the SDK headers used at build time are vendored automatically, so no separate developer-component install is required).
- `dxc` (the DirectX Shader Compiler, bundled with the Windows SDK or the Vulkan SDK) on `PATH` (used to compile the RAID-Z/Z2 parity HLSL shaders at build time).
- **Rust 1.85 or later** (the version in which `edition2024`, required by the `winfsp` crate, was stabilised; older toolchains fail even to parse the `Cargo.toml` manifest).

Either WinFsp or dxc can also be disabled individually (e.g. `--no-default-features --features gpu_accel` for GPU only, without WinFsp).

**Note when actually running the `winfsp_backend` tests (real mount)**: the `winfsp` crate dynamically loads the WinFsp DLL (`winfsp-x64.dll`) via `LoadLibraryW`, which only looks at the standard DLL search path (the executable's own folder, `System32`, and `PATH`). In environments where the WinFsp installer hasn't added itself to `PATH`, the build succeeds fine (no WinFsp SDK headers are needed) but running it **always fails at runtime** (error `WIN32(1285)` = `ERROR_DELAY_LOAD_FAILED`). Add WinFsp's `bin` directory to `PATH` just for the test run:

```powershell
$env:PATH = "C:\Program Files (x86)\WinFsp\bin;$env:PATH"
cargo test --features winfsp_backend,gpu_accel
```

Without this, `mount_pool` returns an `Err`, and the test treats it as an environment-dependent issue, printing a skip message via `eprintln` and returning early. **Without `--nocapture`, this skip still just shows as `ok`, indistinguishable from an actual successful mount+read/write.** Always pass `--nocapture` when checking real-mount tests, and confirm visually that no skip message appears.

### Building & testing the Linux build (FUSE)

```bash
# On Ubuntu/Debian: build-essential, pkg-config, and libfuse3-dev are required.
sudo apt-get install -y build-essential pkg-config libfuse3-dev

cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features --features fuse_backend
```

The `fuse_backend` feature enables the `fuser` crate (a real binding to Linux's `libfuse3`). It's independent of `winfsp_backend`/`gpu_accel`, and can't be enabled on non-Linux targets since `fuser` itself isn't even a dependency there (it lives under `target.'cfg(target_os = "linux")'.dependencies` in `Cargo.toml`). The real-mount integration test (`tests/fuse_mount.rs`) has been verified on WSL2 Ubuntu 26.04 — create, write, read, rename, truncate, delete, a round trip of a larger file spanning multiple stripes, and metadata surviving a real unmount + remount. If you're on Windows only, WSL2 (`wsl --install`) is the recommended way to build/test the Linux target.

A small `orzctl` CLI is also included for creating and mounting a pool directly from the command line:

```bash
cargo build --no-default-features --features fuse_backend --bin orzctl
./target/debug/orzctl create --level z2 --chunk-size 4096 --stripes 1000 --dataset tank /path/to/disk0 /path/to/disk1 ...
./target/debug/orzctl mount  --level z2 --chunk-size 4096 --stripes 1000 --mountpoint /mnt/tank /path/to/disk0 /path/to/disk1 ...
```

To auto-mount at boot, register
[`contrib/systemd/open-raid-z-pool.service.example`](../open_runo_zfs_source/open_raid_z_core/contrib/systemd/open-raid-z-pool.service.example)
as a systemd unit (verified on a VirtualBox VM: a pool created across 4
genuinely separate block devices auto-mounts across a real reboot).

### Installer (`open_runo_installer` / `open_runo_installer_core`)

```powershell
# Logic layer (no Tauri dependency; also runs on Linux/macOS)
cd open_runo_zfs_source/open_runo_installer_core
cargo test                    # CPU fallback only (default)
cargo test --features gpu     # includes real GPU/NPU dispatch (requires a real Windows machine + dxc)

# Frontend (TypeScript, OS-independent)
cd open_runo_zfs_source/open_runo_installer
npm install
npx tsc --noEmit               # type-check only
npx vite build                 # actually build it

# The Tauri app itself (requires a real Windows machine, or a sufficiently new Rust plus Linux desktop dependencies)
cd open_runo_zfs_source/open_runo_installer/src-tauri
cargo tauri dev / cargo tauri build
```

`open_runo_installer_core` (disk detection, the Copilot-style configuration advisor, zpool-init preview) is an independent crate with no Tauri dependency, so its logic can be verified as-is even in environments that lack what Tauri itself needs to build (a WebView, GTK, and so on, plus a sufficiently recent Rust toolchain). Only the actual disk enumeration (`\\.\PhysicalDriveN`) uses a Windows-only API, and is isolated behind `#[cfg(windows)]`; everything else (the configuration advisor and zpool-init preview calculations) is OS-independent, and all 26 of its tests are confirmed to pass.

## Multi-OS & cross-format interoperability roadmap

This project's longer-term aim is for open-raid-z itself to be readable and writable across Windows/Mac/Linux/Android/iOS/iPad, and to interoperate with existing other-OS formats (NTFS/exFAT/FAT32/ext4/APFS, and so on). The current feasibility, priorities, and technical constraints of that effort — most notably that Apple does not permit third-party block-device RAID on iOS/iPad, so support there will likely be limited to browsing via a File Provider Extension — are recorded in [`MULTIPLATFORM_ROADMAP.md`](open_runo_zfs_source/open_raid_z_core/contrib/systemd/MULTIPLATFORM_ROADMAP.md). GPU/NPU acceleration will progressively adopt each OS's native API where DirectX isn't available (for example, Metal Performance Shaders on Mac, NNAPI on Android). Interoperability with third-party RAID formats (e.g. Linux mdadm, Windows Storage Spaces) is also being considered as a future scope item.

## Migrating from an existing setup

For steps to migrate data from an existing ZFS/NTFS/ext4/other RAID setup into `open-raid-z`, see [MIGRATION.md](../MIGRATION.md).


## Licence

MPL-2.0
