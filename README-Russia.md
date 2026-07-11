# open-raid-z (Русский)

**Реализация на Rust реального, монтируемого пула хранения RAID-Z/Z2/Z3.**
Заново, на Rust, реализованы идеи проектирования ZFS/OpenZFS —
распределённое чередование с чётностью, самовосстанавливающиеся
контрольные суммы, copy-on-write, снапшоты/клоны — **без какой-либо
зависимости от самого OpenZFS**. CLI-инструмент `orzctl` создаёт пулы и
**реально монтирует** их в Windows (WinFsp) и Linux/macOS/Android (FUSE).

> [Корневой README](README.md) / [日本語](README-Japan.md) / [English](README-English.md) /
> [中文](README-Chinese.md) / [한국어](README-Korea.md) / [Español](README-Spain.md) /
> [Français](README-France.md) / [Deutsch](README-Germany.md) / [Italiano](README-Italy.md) /
> [العربية](README-Arabic.md)

## Важная оговорка

open-raid-z использует **собственный формат на диске** (CoW/striping в
стиле ZFS) и **несовместим по формату диска с настоящим ZFS** (нет
uberblock, нет ZIL и т.д.). Миграция с существующего ZFS/NTFS/ext4/
других RAID всегда означает «① прочитать из существующего формата →
② обычным образом скопировать файлы в пул open-raid-z». См.
[MIGRATION.md](MIGRATION.md).

## Структура workspace (3 крейта + вспомогательные компоненты)

| Компонент | Роль / статус |
|---|---|
| `open_raid_z_core` | Основная библиотека: уровни RAID (`Raid0`/`Raid1`/`Raid5`/`Raid6`≡`Z2`/`Z3`, перечисление `RaidLevel` в `vdev.rs`), контрольные суммы sha2, copy-on-write, снапшоты/клоны, эмуляция ACL, взаимодействие с FAT32/exFAT (`foreign_fs`, чтение+запись), реальное монтирование (WinFsp в Windows, FUSE в Linux/macOS/Android) и бинарник CLI `orzctl` |
| `zfs_accel_hlsl` | Ускоряет на GPU вычисление чётности в поле Галуа для RAID-Z/Z2/Z3 через HLSL-шейдеры + D3D12/DirectML. При отключённой фиче `gpu_accel` откатывается на чистую реализацию на CPU (Rust), что удобно для CI без WinFsp/dxc |
| `open_runo_installer_core` | ОС-независимая логика обнаружения дисков, советов по конфигурации zpool и предпросмотра; намеренно выделена в отдельный крейт, не зависящий от Tauri, чтобы не попадать под ограничения edition2024 у Tauri |
| `open_runo_installer` (GUI на Tauri) | Десктопное приложение на Tauri 2 + TypeScript, использующее `installer_core`. **Единственное место во всей экосистеме, напрямую зависящее от пакета Tauri** (отдельно от политики репозиториев веб-экосистемы переписывать Tauri с нуля) |
| `wdk_driver/orzflt` | Минимальный скелет драйвера режима ядра Windows (WDF/KMDF 1.35). Проверена сборка только загрузки/выгрузки; **реальное тестирование загрузки намеренно отложено до изолированной VM** — ранняя стадия |
| `third_party/fuser-0.17.0-android-patch` | Патченный форк крейта `fuser`, позволяющий чисто Rust-сборки для Android. Кросс-компилируется в arm64-v8a через `cargo ndk`; ещё не проверено на реальном устройстве |

## Командная строка `orzctl`

```sh
# создать пул Z2 на 6 дисках
orzctl create --level z2 --chunk-size 4096 --stripes 100000 --dataset tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# реально смонтировать (остаётся на переднем плане)
orzctl mount --level z2 --chunk-size 4096 --stripes 100000 --mountpoint /mnt/tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# чтение/запись существующего тома FAT32/exFAT (помощь при миграции)
orzctl foreign ls /dev/sdb1
orzctl foreign --format exfat cat /dev/sdc1 /video.mp4 ./video.mp4
```

Поддерживаемые уровни RAID: `Raid0` / `Raid1` (зеркало) / `Raid5` /
`Raid6` (то же, что `Z2`) / `Z2` / `Z3`. RAID10 предоставляется отдельно
как набор зеркальных групп `Raid1` (`raid10.rs`).

## Сборка и тесты (измерено)

```sh
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features
```

Это сборка с откатом на CPU, не требующая ни SDK WinFsp, ни `dxc`, ни
Windows SDK. Измерено 2026-07-11:

| Крейт | пройдено | провалено |
|---|---|---|
| `open_raid_z_core` (`--no-default-features`) | 101 | 0 |
| `zfs_accel_hlsl` (`--no-default-features`, откат на CPU) | 32 | 0 |
| `open_runo_installer_core` | 30 | 0 |
| **Итого** | **163** | **0** |

Набор фич `default` (`winfsp_backend` + `gpu_accel`, реальное монтирование
+ реальные вычисления на GPU) требует машину с Windows, SDK WinFsp и
`dxc`, и должен проверяться отдельно.

## Документация

- [MIGRATION.md](MIGRATION.md) — миграция с ZFS/NTFS/ext4/других RAID
- [PORTING.md](PORTING.md) — руководство на одной странице для внедрения в другой проект
- [CLAUDE.md](CLAUDE.md) — правила разработки / технологический стек (эталон для этой экосистемы)
- [CHAT_HANDOFF.md](CHAT_HANDOFF.md) — история разработки / заметки о передаче

## Лицензия

MPL-2.0.
