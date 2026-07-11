# open-raid-z (한국어)

**실제로 마운트 가능한 RAID-Z/Z2/Z3 스토리지 풀을 Rust로 구현한 프로젝트.**
OpenZFS 자체에 **전혀 의존하지 않고** ZFS/OpenZFS의 설계 사상(패리티
분산 스트라이핑, 체크섬 자가 복구, Copy-on-Write, 스냅샷/클론)을 처음부터
Rust로 다시 구현했습니다. CLI 도구 `orzctl`로 풀을 생성하고 Windows
(WinFsp)·Linux/macOS/Android(FUSE)에 **실제로 마운트**할 수 있습니다.

> [루트 README](README.md) / [日本語](README-Japan.md) / [English](README-English.md) /
> [中文](README-Chinese.md) / [Español](README-Spain.md) / [Français](README-France.md) /
> [Deutsch](README-Germany.md) / [Italiano](README-Italy.md) / [Русский](README-Russia.md) /
> [العربية](README-Arabic.md)

## 중요한 전제

open-raid-z는 **자체 온디스크 포맷**(ZFS 스타일의 CoW/스트라이핑
레이아웃)을 사용하며, 실제 ZFS의 온디스크 구조(uberblock, ZIL 등)와는
**호환되지 않습니다**. 기존 ZFS/NTFS/ext4/타사 RAID에서 마이그레이션할
때는 항상 "①기존 포맷에서 읽기 → ②open-raid-z 풀로 일반 파일 복사"
절차를 따릅니다. 자세한 내용은 [MIGRATION.md](MIGRATION.md) 참고.

## 구성 (3개 crate + 보조 컴포넌트)

| 컴포넌트 | 역할/현황 |
|---|---|
| `open_raid_z_core` | 핵심 라이브러리: RAID 레벨(`Raid0`/`Raid1`/`Raid5`/`Raid6`≡`Z2`/`Z3`, `vdev.rs`의 `RaidLevel` 열거형), sha2 체크섬, Copy-on-Write, 스냅샷/클론, ACL 에뮬레이션, FAT32/exFAT 상호운용(`foreign_fs`, 읽기+쓰기), 실제 마운트(Windows=WinFsp, Linux/macOS/Android=FUSE), `orzctl` CLI 바이너리 |
| `zfs_accel_hlsl` | RAID-Z/Z2/Z3의 갈루아체(GF) 패리티 계산을 HLSL 셰이더 + D3D12/DirectML로 GPU 가속. `gpu_accel` 기능 비활성화 시 순수 Rust CPU 폴백만 사용(WinFsp/dxc 없는 CI 환경용) |
| `open_runo_installer_core` | 디스크 감지·zpool 구성 조언·미리보기를 위한 OS 독립 로직. Tauri 본체의 edition2024 요구사항에 얽매이지 않도록 의도적으로 Tauri 비의존 crate로 분리 |
| `open_runo_installer` (Tauri GUI) | 위 `installer_core`를 사용하는 Tauri 2 + TypeScript 데스크톱 앱. **이 생태계 전체에서 유일하게 Tauri 패키지에 직접 의존하는 부분**(웹 생태계 저장소들의 "Tauri 자체 재구현" 방침과는 별개) |
| `wdk_driver/orzflt` | Windows 커널 모드 드라이버 최소 스켈레톤(WDF/KMDF 1.35). 로드/언로드만 빌드 검증됨, **실제 로드 테스트는 격리된 VM에서만 수행하는 방침**으로 아직 미실시(초기 단계) |
| `third_party/fuser-0.17.0-android-patch` | `fuser` crate가 Android 순수 Rust 빌드를 지원하도록 패치한 포크. `cargo ndk`로 arm64-v8a 크로스컴파일 성공, 실기기 검증은 미실시 |

## `orzctl` 명령줄

```sh
# 디스크 6개로 Z2 풀 생성
orzctl create --level z2 --chunk-size 4096 --stripes 100000 --dataset tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# 실제 마운트(포그라운드로 대기)
orzctl mount --level z2 --chunk-size 4096 --stripes 100000 --mountpoint /mnt/tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# 기존 FAT32/exFAT 볼륨 읽기/쓰기(마이그레이션 보조)
orzctl foreign ls /dev/sdb1
orzctl foreign --format exfat cat /dev/sdc1 /video.mp4 ./video.mp4
```

지원 RAID 레벨: `Raid0` / `Raid1`(미러) / `Raid5` / `Raid6`(`Z2`와 동일) /
`Z2` / `Z3`. RAID10은 `Raid1` 미러 그룹을 묶는 형태로 별도 제공
(`raid10.rs`).

## 빌드 및 테스트 (실측)

```sh
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features
```

WinFsp SDK·`dxc`·Windows SDK가 필요 없는 CPU 폴백 구성입니다.
2026-07-11 실측값:

| Crate | 통과 | 실패 |
|---|---|---|
| `open_raid_z_core` (`--no-default-features`) | 101 | 0 |
| `zfs_accel_hlsl` (`--no-default-features`, CPU 폴백) | 32 | 0 |
| `open_runo_installer_core` | 30 | 0 |
| **합계** | **163** | **0** |

`default` 기능 세트(`winfsp_backend` + `gpu_accel`, 실제 마운트 + 실제
GPU 연산)는 Windows 실기기와 WinFsp SDK, `dxc`가 필요하며 별도로
검증해야 합니다.

## 문서

- [MIGRATION.md](MIGRATION.md) — 기존 ZFS/NTFS/ext4/타사 RAID에서 마이그레이션
- [PORTING.md](PORTING.md) — 다른 프로젝트로의 이식 가이드(단일 파일)
- [CLAUDE.md](CLAUDE.md) — 개발 규칙/기술 스택(이 생태계의 정본)
- [CHAT_HANDOFF.md](CHAT_HANDOFF.md) — 개발 경위/인수인계 기록

## 라이선스

MPL-2.0.
