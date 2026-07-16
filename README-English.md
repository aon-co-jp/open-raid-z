# open-raid-z (English)

**A Rust implementation of a real, mountable RAID-Z/Z2/Z3 storage pool.**
It reimplements ZFS/OpenZFS's design ideas — parity-distributed striping,
self-healing checksums, copy-on-write, snapshots/clones — **from scratch in
Rust, with zero dependency on OpenZFS itself**. The `orzctl` CLI creates
pools and **actually mounts them** on Windows (WinFsp) and Linux/macOS/
Android (FUSE).

> [Root README](README.md) / [日本語](README-Japan.md) /
> [中文](README-Chinese.md) / [한국어](README-Korea.md) / [Español](README-Spain.md) /
> [Français](README-France.md) / [Deutsch](README-Germany.md) / [Italiano](README-Italy.md) /
> [Русский](README-Russia.md) / [العربية](README-Arabic.md)

## Important caveat

open-raid-z uses its **own on-disk format** (a ZFS-style CoW/striping
layout) and is **not on-disk compatible with real ZFS** (no uberblock, no
ZIL, etc.). Migrating from existing ZFS/NTFS/ext4/other RAID always means
"① read from the existing format → ② plain file-copy into an open-raid-z
pool." See [MIGRATION.md](MIGRATION.md) for the full procedure.

## Workspace layout (3 crates + supporting components)

| Component | Role / status |
|---|---|
| `open_raid_z_core` | Core library: RAID levels (`Raid0`/`Raid1`/`Raid5`/`Raid6`≡`Z2`/`Z3`, see the `RaidLevel` enum in `vdev.rs`), sha2 checksums, copy-on-write, snapshots/clones, ACL emulation, FAT32/exFAT interop (`foreign_fs` feature, read+write), real mounting (WinFsp on Windows, FUSE on Linux/macOS/Android), and the `orzctl` CLI binary |
| `zfs_accel_hlsl` | GPU-accelerates the Galois-field parity math for RAID-Z/Z2/Z3 via HLSL shaders + D3D12/DirectML. With the `gpu_accel` feature disabled it falls back to a pure-Rust CPU implementation only (useful for CI without WinFsp/dxc) |
| `open_runo_installer_core` | OS-independent disk-detection / zpool-advisor / preview logic, deliberately split out as a Tauri-independent crate so it isn't caught by Tauri's edition2024 constraints |
| `open_runo_installer` (Tauri GUI) | A Tauri 2 + TypeScript desktop app that uses `installer_core`. **This is the one place in the whole ecosystem that depends directly on the Tauri package** (separate from the web-ecosystem repos' policy of reimplementing Tauri from scratch) |
| `wdk_driver/orzflt` | A minimal Windows kernel-mode driver skeleton (WDF/KMDF 1.35). Only load/unload has been build-verified; **actual load testing is intentionally deferred to an isolated VM** — early-stage |
| `third_party/fuser-0.17.0-android-patch` | A patched fork of the `fuser` crate that enables pure-Rust builds for Android. Cross-compiles to arm64-v8a via `cargo ndk`; not yet verified on a real device |

## `orzctl` command line

```sh
# create a Z2 pool across 6 disks
orzctl create --level z2 --chunk-size 4096 --stripes 100000 --dataset tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# actually mount it (stays in the foreground)
orzctl mount --level z2 --chunk-size 4096 --stripes 100000 --mountpoint /mnt/tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# read/write an existing FAT32/exFAT volume (migration helper)
orzctl foreign ls /dev/sdb1
orzctl foreign --format exfat cat /dev/sdc1 /video.mp4 ./video.mp4
```

Supported RAID levels: `Raid0` / `Raid1` (mirror) / `Raid5` / `Raid6`
(same as `Z2`) / `Z2` / `Z3`. RAID10 is provided separately as a bundle of
`Raid1` mirror groups (`raid10.rs`).

## Build & test (measured)

```sh
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features
```

This is a CPU-fallback build requiring neither the WinFsp SDK, `dxc`, nor
the Windows SDK. Measured on 2026-07-11:

| Crate | passed | failed |
|---|---|---|
| `open_raid_z_core` (`--no-default-features`) | 101 | 0 |
| `zfs_accel_hlsl` (`--no-default-features`, CPU fallback) | 32 | 0 |
| `open_runo_installer_core` | 30 | 0 |
| **Total** | **163** | **0** |

The `default` feature set (`winfsp_backend` + `gpu_accel`, real mounting
and real GPU compute) requires a Windows machine with the WinFsp SDK and
`dxc` and must be verified separately.

## Documentation

- [MIGRATION.md](MIGRATION.md) — migrating from ZFS/NTFS/ext4/other RAID
- [PORTING.md](PORTING.md) — one-file guide to adopting this in another project
- [CLAUDE.md](CLAUDE.md) — dev rules / tech stack (canonical for this ecosystem)
- [CHAT_HANDOFF.md](CHAT_HANDOFF.md) — development history / handoff notes

## Related Projects

There is a target architecture combining `open-web-server` with
`poem-cosmo-tauri`/`open-runo`, PostgreSQL, `aruaru-db`, and this
repository, designed to prevent network loss of paid items and
financial/securities data in 3D online games (quadruple-redundant
transport and quadruple-redundant DB writes, revised 2026-07-11).
open-raid-z participates as the disk-redundancy foundation, and its
ZFS-like checksum/copy-on-write/snapshot characteristics have real,
documented relevance to database (PostgreSQL/aruaru-db) read/write
reliability as well (see
[open-web-server](https://github.com/aon-co-jp/open-web-server)'s
`README.md`/`CLAUDE.md` for the full picture with sources).

## License

MPL-2.0.
