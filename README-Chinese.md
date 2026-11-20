# open-raid-z（中文简体版）

**用 Rust 实现的、可实际挂载的 RAID-Z/Z2/Z3 存储池。**
在**完全不依赖 OpenZFS 本体**的前提下，用 Rust 从零实现了 ZFS/OpenZFS 的
设计理念——奇偶校验分散条带化、校验和自愈、写时复制（CoW）、
快照/克隆。命令行工具 `orzctl` 用来创建存储池，并**真正挂载**到
Windows（WinFsp）与 Linux/macOS/Android（FUSE）。

> [根 README](README.md) / [日本語](README-Japan.md) / [English](README-English.md) /
> [한국어](README-Korea.md) / [Español](README-Spain.md) / [Français](README-France.md) /
> [Deutsch](README-Germany.md) / [Italiano](README-Italy.md) / [Русский](README-Russia.md) /
> [العربية](README-Arabic.md)

## 重要前提

open-raid-z 使用**自有的磁盘格式**（ZFS 风格的 CoW/条带化布局），
**与真正的 ZFS 磁盘结构（uberblock、ZIL 等）不兼容**。从现有
ZFS/NTFS/ext4/其他 RAID 迁移时，流程始终是「①从原格式读出 →
②普通文件拷贝到 open-raid-z 存储池」。详见 [MIGRATION.md](MIGRATION.md)。

## 组成（3 个 crate + 辅助组件）

| 组件 | 作用/现状 |
|---|---|
| `open_raid_z_core` | 核心库：RAID 级别（`Raid0`/`Raid1`/`Raid5`/`Raid6`≡`Z2`/`Z3`，见 `vdev.rs` 的 `RaidLevel`）、sha2 校验和、写时复制、快照/克隆、ACL 模拟、FAT32/exFAT 互操作（`foreign_fs`，支持读写）以及只读的 ext2/ext4 访问（同一 feature）、真实挂载（Windows 用 WinFsp，Linux/macOS/Android 用 FUSE）、以及 `orzctl` CLI |
| `zfs_accel_hlsl` | 通过 HLSL 着色器 + D3D12/DirectML 对 RAID-Z/Z2/Z3 的伽罗瓦域奇偶校验计算做 GPU 加速；关闭 `gpu_accel` 特性时退化为纯 Rust CPU 实现（适合无 WinFsp/dxc 的 CI 环境） |
| `open_runo_installer_core` | 磁盘检测、zpool 配置建议、预览等与操作系统无关的逻辑；特意拆成不依赖 Tauri 的独立 crate，避免受 Tauri 本体 edition2024 要求牵连 |
| `open_runo_installer`（Tauri GUI） | 使用上述 `installer_core` 的 Tauri 2 + TypeScript 桌面应用。**这是整个生态中唯一直接依赖 Tauri 包的地方**（与 Web 生态各仓库「自研重现 Tauri」的方针不同） |
| `wdk_driver/orzflt` | Windows 内核态驱动的最小骨架（WDF/KMDF 1.35）。仅验证了加载/卸载可编译，**实际加载测试有意保留到隔离虚拟机中进行**，仍处于早期阶段 |
| `third_party/fuser-0.17.0-android-patch` | 让 `fuser` crate 支持 Android 纯 Rust 构建的补丁分支；已通过 `cargo ndk` 交叉编译到 arm64-v8a，尚未在真机上验证 |

## `orzctl` 命令行

```sh
# 用 6 块磁盘创建 Z2 存储池
orzctl create --level z2 --chunk-size 4096 --stripes 100000 --dataset tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# 实际挂载（前台驻留）
orzctl mount --level z2 --chunk-size 4096 --stripes 100000 --mountpoint /mnt/tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# 读写现有的 FAT32/exFAT 卷（迁移辅助）
orzctl foreign ls /dev/sdb1
orzctl foreign --format exfat cat /dev/sdc1 /video.mp4 ./video.mp4

# 只读访问现有的 ext2/ext4 卷
orzctl foreign --format ext4 ls  /dev/sdd1 /home
orzctl foreign --format ext4 cat /dev/sdd1 /etc/hostname
```

支持的 RAID 级别：`Raid0` / `Raid1`（镜像） / `Raid5` / `Raid6`（等同于
`Z2`） / `Z2` / `Z3`。RAID10 另行以多个 `Raid1` 镜像组捆绑提供
（`raid10.rs`）。

## 构建与测试（实测数据）

```sh
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features
```

这是无需 WinFsp SDK、`dxc`、Windows SDK 的 CPU 回退构建。
2026-07-11 实测：

| Crate | 通过 | 失败 |
|---|---|---|
| `open_raid_z_core`（`--no-default-features`） | 101 | 0 |
| `zfs_accel_hlsl`（`--no-default-features`，CPU 回退） | 32 | 0 |
| `open_runo_installer_core` | 30 | 0 |
| **合计** | **163** | **0** |

`default` 特性集（`winfsp_backend` + `gpu_accel`，真实挂载 + 真实 GPU
计算）需要 Windows 实机、WinFsp SDK 和 `dxc`，需另行验证。

## 文档

- [MIGRATION.md](MIGRATION.md) — 从 ZFS/NTFS/ext4/其他 RAID 迁移
- [PORTING.md](PORTING.md) — 一份文件搞定的移植/引入指南
- [CLAUDE.md](CLAUDE.md) — 开发规则/技术栈（本生态系统的正本）
- [CHAT_HANDOFF.md](CHAT_HANDOFF.md) — 开发历程/交接记录

## 许可证

## 相关项目

存在一个将 `open-web-server` 与 `poem-cosmo-tauri`/`open-runo`、
PostgreSQL、`aruaru-db`、本仓库组合起来的目标架构,旨在防止 3D 网络游戏的
付费道具及金融/证券数据在网络上丢失(通信层四重化与数据库写入四重化,
2026-07-11 修订)。open-raid-z 作为该架构的磁盘冗余基础参与,其实现的
类 ZFS 校验和/写时复制/快照特性与 DATABASE(PostgreSQL/aruaru-db)的
读写可靠性也有实际的关联(完整内容及出处参见
[open-web-server](https://github.com/aon-co-jp/open-web-server) 的
`README.md`/`CLAUDE.md`)。

MPL-2.0。
