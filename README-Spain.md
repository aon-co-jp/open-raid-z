# open-raid-z (Español)

**Una implementación en Rust de un pool de almacenamiento RAID-Z/Z2/Z3
real y montable.** Reimplementa las ideas de diseño de ZFS/OpenZFS
—striping con paridad distribuida, checksums autorreparables,
copy-on-write, snapshots/clones— **desde cero en Rust, sin ninguna
dependencia de OpenZFS**. La CLI `orzctl` crea pools y los **monta de
verdad** en Windows (WinFsp) y Linux/macOS/Android (FUSE).

> [README raíz](README.md) / [日本語](README-Japan.md) / [English](README-English.md) /
> [中文](README-Chinese.md) / [한국어](README-Korea.md) / [Français](README-France.md) /
> [Deutsch](README-Germany.md) / [Italiano](README-Italy.md) / [Русский](README-Russia.md) /
> [العربية](README-Arabic.md)

## Advertencia importante

open-raid-z usa **su propio formato en disco** (un diseño CoW/striping
al estilo ZFS) y **no es compatible a nivel de disco con ZFS real** (sin
uberblock, sin ZIL, etc.). Migrar desde ZFS/NTFS/ext4/otros RAID
existentes siempre significa "① leer del formato existente → ② copiar
archivos normalmente a un pool de open-raid-z". Ver [MIGRATION.md](MIGRATION.md).

## Estructura del workspace (3 crates + componentes de soporte)

| Componente | Rol / estado |
|---|---|
| `open_raid_z_core` | Librería principal: niveles RAID (`Raid0`/`Raid1`/`Raid5`/`Raid6`≡`Z2`/`Z3`, enum `RaidLevel` en `vdev.rs`), checksums sha2, copy-on-write, snapshots/clones, emulación de ACL, interoperabilidad FAT32/exFAT (`foreign_fs`, lectura+escritura), montaje real (WinFsp en Windows, FUSE en Linux/macOS/Android) y el binario CLI `orzctl` |
| `zfs_accel_hlsl` | Acelera por GPU el cálculo de paridad en cuerpo de Galois para RAID-Z/Z2/Z3 mediante shaders HLSL + D3D12/DirectML. Con la feature `gpu_accel` desactivada, cae a una implementación CPU en Rust puro (útil para CI sin WinFsp/dxc) |
| `open_runo_installer_core` | Lógica independiente del SO para detección de discos, asesor de configuración zpool y vista previa; separada deliberadamente como crate independiente de Tauri para no verse afectada por los requisitos de edition2024 de Tauri |
| `open_runo_installer` (GUI Tauri) | Una app de escritorio Tauri 2 + TypeScript que usa `installer_core`. **Es el único lugar de todo el ecosistema que depende directamente del paquete Tauri** (aparte de la política de los repos del ecosistema web de reimplementar Tauri desde cero) |
| `wdk_driver/orzflt` | Un esqueleto mínimo de driver en modo kernel de Windows (WDF/KMDF 1.35). Solo se ha verificado la compilación de carga/descarga; **las pruebas reales de carga se dejan intencionalmente para una VM aislada** — etapa temprana |
| `third_party/fuser-0.17.0-android-patch` | Un fork parcheado del crate `fuser` que permite compilaciones puramente en Rust para Android. Compila de forma cruzada a arm64-v8a vía `cargo ndk`; aún no verificado en un dispositivo real |

## Línea de comandos `orzctl`

```sh
# crear un pool Z2 con 6 discos
orzctl create --level z2 --chunk-size 4096 --stripes 100000 --dataset tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# montarlo de verdad (permanece en primer plano)
orzctl mount --level z2 --chunk-size 4096 --stripes 100000 --mountpoint /mnt/tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# leer/escribir un volumen FAT32/exFAT existente (ayuda para migración)
orzctl foreign ls /dev/sdb1
orzctl foreign --format exfat cat /dev/sdc1 /video.mp4 ./video.mp4
```

Niveles RAID soportados: `Raid0` / `Raid1` (espejo) / `Raid5` / `Raid6`
(igual que `Z2`) / `Z2` / `Z3`. RAID10 se ofrece por separado como un
conjunto de grupos espejo `Raid1` (`raid10.rs`).

## Compilación y pruebas (medido)

```sh
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features
```

Es una compilación de respaldo por CPU que no requiere el SDK de WinFsp,
`dxc` ni el SDK de Windows. Medido el 2026-07-11:

| Crate | pasaron | fallaron |
|---|---|---|
| `open_raid_z_core` (`--no-default-features`) | 101 | 0 |
| `zfs_accel_hlsl` (`--no-default-features`, respaldo CPU) | 32 | 0 |
| `open_runo_installer_core` | 30 | 0 |
| **Total** | **163** | **0** |

El conjunto de features `default` (`winfsp_backend` + `gpu_accel`, montaje
real + cómputo GPU real) requiere una máquina Windows con el SDK de
WinFsp y `dxc`, y debe verificarse por separado.

## Documentación

- [MIGRATION.md](MIGRATION.md) — migrar desde ZFS/NTFS/ext4/otros RAID
- [PORTING.md](PORTING.md) — guía de una sola página para adoptarlo en otro proyecto
- [CLAUDE.md](CLAUDE.md) — reglas de desarrollo / stack tecnológico (canónico para este ecosistema)
- [CHAT_HANDOFF.md](CHAT_HANDOFF.md) — historial de desarrollo / notas de traspaso

## Proyectos relacionados

Existe una arquitectura objetivo que combina `open-web-server` con
`poem-cosmo-tauri`/`open-runo`, PostgreSQL, `aruaru-db` y este
repositorio, diseñada para evitar la pérdida en red de artículos de pago
y datos financieros/bursátiles en juegos 3D online (transporte
cuádruple-redundante y escritura de BD cuádruple-redundante, revisado el
2026-07-11). open-raid-z participa como base de redundancia de disco, y
sus características tipo ZFS (checksums, copy-on-write, snapshots)
tienen relevancia real y documentada para la fiabilidad de lectura/
escritura de bases de datos (PostgreSQL/aruaru-db) también (ver el
`README.md`/`CLAUDE.md` de
[open-web-server](https://github.com/aon-co-jp/open-web-server) para el
panorama completo con fuentes).

## Licencia

MPL-2.0.
