//! zpool初期化ウィザードのバックエンドロジック。
//!
//! 【安全設計】実際の物理ディスクへ直接書き込むのは事故が大きすぎるため、
//! 本ウィザードはまず一時ファイル(スクラッチイメージ)上でプールを構築して
//! 動作を確認する「プレビュー」モードのみを提供する。実ディスクへの適用は、
//! VHDXアタッチ等で得られる`\\.\PhysicalDriveN`パスを
//! `open_raid_z_core::block_device::FileBackedDevice::open`へそのまま
//! 渡せる設計になっている(`block_device.rs`のドキュメント参照)ため、将来
//! UI側に「実ディスクへ適用」ボタンを追加するだけで対応できる。

use open_raid_z_core::block_device::FileBackedDevice;
use open_raid_z_core::pool::Pool;
use open_raid_z_core::raid10::Raid10Vdev;
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};
use serde::{Deserialize, Serialize};

const CHUNK_SIZE: usize = 4096;
const STRIPES_PER_DISK: u64 = 16;

#[derive(Debug, Deserialize)]
pub struct ZpoolInitRequest {
    /// 選択されたディスクの本数(実ディスクには書き込まず、本数と同じ数の
    /// スクラッチイメージでプールを構築してプレビューする)。
    pub disk_count: u32,
    pub level: String,
    pub dataset_name: String,
}

#[derive(Debug, Serialize)]
pub struct ZpoolInitResult {
    pub accelerator: String,
    pub total_stripes: u64,
    pub used_stripes: u64,
    pub free_stripes: u64,
    pub dataset_size_bytes: u64,
}

fn parse_level(level: &str) -> Result<RaidLevel, String> {
    match level {
        "Raid0" => Ok(RaidLevel::Raid0),
        "Raid1" => Ok(RaidLevel::Raid1),
        "Raid5" => Ok(RaidLevel::Raid5),
        "Raid6" => Ok(RaidLevel::Raid6),
        "Z2" => Ok(RaidLevel::Z2),
        "Z3" => Ok(RaidLevel::Z3),
        other => Err(format!(
            "未対応のRAIDレベルです: {other}(Raid0/Raid1/Raid5/Raid6/Z2/Z3のいずれかを指定してください)"
        )),
    }
}

/// スクラッチイメージ上でプールを初期化し、指定した名前のデータセットを
/// プール全体の容量ぶん確保した上で、その結果を返す
/// (実ディスクを一切変更しないプレビュー実行)。
pub fn init_zpool_preview(req: ZpoolInitRequest) -> Result<ZpoolInitResult, String> {
    let level = parse_level(&req.level)?;
    let parity_count = level.parity_count(req.disk_count as usize);
    if req.disk_count as usize <= parity_count {
        return Err(format!(
            "{:?}にはデータディスクが最低1台必要です(合計{}台以上を選択してください)",
            level,
            parity_count + 1
        ));
    }

    let mut scratch_paths = Vec::with_capacity(req.disk_count as usize);
    let mut devices = Vec::with_capacity(req.disk_count as usize);
    for i in 0..req.disk_count {
        let path = std::env::temp_dir().join(format!(
            "open_runo_installer_preview_{}_{}.img",
            std::process::id(),
            i
        ));
        let dev = FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * STRIPES_PER_DISK)
            .map_err(|e| format!("スクラッチイメージの作成に失敗しました: {e}"))?;
        scratch_paths.push(path);
        devices.push(dev);
    }

    let accel_device = zfs_accel_hlsl::detect_best_accelerator().ok();
    let accelerator = accel_device
        .as_ref()
        .map(|a| format!("{:?}: {}", a.kind, a.adapter_description))
        .unwrap_or_else(|| "検出失敗".to_string());

    let num_data_disks = req.disk_count as u64 - parity_count as u64;
    let total_stripes = num_data_disks * STRIPES_PER_DISK;

    let mut vdev = RaidZVdev::new(devices, level, CHUNK_SIZE);
    if let Some(accel) = accel_device {
        vdev = vdev.with_accelerator(accel);
    }
    let mut pool = Pool::new(vdev, total_stripes);
    // `Pool::new`はメタデータ(スーパーブロック)用に1ストライプを予約するため、
    // 実際にデータセットへ割り当てられる容量は`total_stripes`より1少ない
    // (`pool.usage().free_stripes`が予約後の実容量)。
    let usable_stripes = pool.usage().free_stripes;

    pool.create_dataset(&req.dataset_name)
        .map_err(|e| format!("データセットの作成に失敗しました: {e}"))?;
    pool.grow_dataset(&req.dataset_name, usable_stripes * (CHUNK_SIZE as u64 * num_data_disks))
        .map_err(|e| format!("データセットの容量確保に失敗しました: {e}"))?;

    let usage = pool.usage();
    let dataset_size_bytes = pool
        .dataset_size(&req.dataset_name)
        .map_err(|e| format!("データセットサイズの取得に失敗しました: {e}"))?;

    for path in scratch_paths {
        std::fs::remove_file(&path).ok();
    }

    Ok(ZpoolInitResult {
        accelerator,
        total_stripes: usage.total_stripes,
        used_stripes: usage.used_stripes,
        free_stripes: usage.free_stripes,
        dataset_size_bytes,
    })
}

#[derive(Debug, Deserialize)]
pub struct Raid10InitRequest {
    pub disk_count: u32,
    /// 1ミラーグループあたりの台数(通常は2)。
    pub mirror_width: u32,
    pub dataset_name: String,
}

#[derive(Debug, Serialize)]
pub struct Raid10InitResult {
    pub accelerator: String,
    pub num_groups: usize,
    pub total_stripes: u64,
    pub used_stripes: u64,
    pub free_stripes: u64,
    pub dataset_size_bytes: u64,
}

/// RAID10(ストライプ+ミラー)のプレビュー。
///
/// [`Raid10Vdev`]は[`open_raid_z_core::vdev::Vdev`]トレイトを実装して
/// いるため、[`Pool`]は`RaidZVdev`と全く同じデータセットAPI
/// (`create_dataset`/`grow_dataset`/`write`/`read`)でRAID10も扱える
/// (`raid10.rs`のモジュールドキュメント参照)。
pub fn init_raid10_preview(req: Raid10InitRequest) -> Result<Raid10InitResult, String> {
    let mirror_width = req.mirror_width as usize;
    if mirror_width < 2 {
        return Err("ミラー幅(mirror_width)は2台以上を指定してください".to_string());
    }
    if req.disk_count == 0 || req.disk_count as usize % mirror_width != 0 {
        return Err(format!(
            "ディスク台数({})はミラー幅({mirror_width})の倍数である必要があります",
            req.disk_count
        ));
    }

    let mut scratch_paths = Vec::with_capacity(req.disk_count as usize);
    let mut devices = Vec::with_capacity(req.disk_count as usize);
    for i in 0..req.disk_count {
        let path = std::env::temp_dir().join(format!(
            "open_runo_installer_raid10_preview_{}_{}.img",
            std::process::id(),
            i
        ));
        let dev = FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * STRIPES_PER_DISK)
            .map_err(|e| format!("スクラッチイメージの作成に失敗しました: {e}"))?;
        scratch_paths.push(path);
        devices.push(dev);
    }

    let accel_device = zfs_accel_hlsl::detect_best_accelerator().ok();
    let accelerator = accel_device
        .as_ref()
        .map(|a| format!("{:?}: {}", a.kind, a.adapter_description))
        .unwrap_or_else(|| "検出失敗".to_string());

    let vdev = Raid10Vdev::new(devices, mirror_width, CHUNK_SIZE)
        .map_err(|e| format!("RAID10 vdevの構築に失敗しました: {e}"))?;
    let num_groups = vdev.num_groups();
    let total_stripes = num_groups as u64 * STRIPES_PER_DISK;

    let mut pool = Pool::new(vdev, total_stripes);
    // `pool.usage().free_stripes`は`Pool::new`のスーパーブロック予約(1ストライプ)
    // を差し引いた実容量。さらにCoW書き込みには常に1ストライプ以上の空きが
    // 必要なため、そこからもう1ストライプ差し引いた分だけをデータセットへ
    // 割り当てる(丸ごと割り当てない)。
    let dataset_stripes = pool.usage().free_stripes.saturating_sub(1).max(1);

    pool.create_dataset(&req.dataset_name)
        .map_err(|e| format!("データセットの作成に失敗しました: {e}"))?;
    pool.grow_dataset(&req.dataset_name, dataset_stripes * CHUNK_SIZE as u64)
        .map_err(|e| format!("データセットの容量確保に失敗しました: {e}"))?;

    let usage = pool.usage();
    let dataset_size_bytes = pool
        .dataset_size(&req.dataset_name)
        .map_err(|e| format!("データセットサイズの取得に失敗しました: {e}"))?;

    for path in scratch_paths {
        std::fs::remove_file(&path).ok();
    }

    Ok(Raid10InitResult {
        accelerator,
        num_groups,
        total_stripes: usage.total_stripes,
        used_stripes: usage.used_stripes,
        free_stripes: usage.free_stripes,
        dataset_size_bytes,
    })
}

#[derive(Debug, Deserialize)]
pub struct DiskSelection {
    /// `hardware::list_physical_disks`が返す`\\.\PhysicalDriveN`形式のパス。
    pub path: String,
    /// 同関数が返すディスクサイズ(バイト)。実サイズはここでのみ受け取り、
    /// 本モジュール側では再問い合わせしない(呼び出し元の一覧結果を信頼する)。
    pub size_bytes: u64,
}

#[derive(Debug, Deserialize)]
pub struct ZpoolApplyRequest {
    pub disks: Vec<DiskSelection>,
    pub level: String,
    pub dataset_name: String,
    /// UI側の「このディスクの既存データは全て消去されます」チェックボックスに
    /// 対応するフラグ。false(未確認)の場合は何も開かず・書き込まずエラーで
    /// 拒否する。
    pub confirm_data_loss: bool,
}

/// 実ディスクへzpoolを初期化する(プレビューと異なり、選択したディスクの
/// 既存データは完全に上書きされる)。
///
/// `disks`には`hardware::list_physical_disks`で列挙した実在パスを渡すこと。
/// `confirm_data_loss`がfalseの場合、ディスクを一切開かずに拒否する。
pub fn init_zpool_apply(req: ZpoolApplyRequest) -> Result<ZpoolInitResult, String> {
    if !req.confirm_data_loss {
        return Err(
            "実ディスクへの適用には確認が必要です(選択したディスクの既存データは全て消去されます)"
                .to_string(),
        );
    }

    let disk_count = req.disks.len();
    if disk_count == 0 {
        return Err("ディスクが選択されていません".to_string());
    }

    let level = parse_level(&req.level)?;
    let parity_count = level.parity_count(disk_count);
    if disk_count <= parity_count {
        return Err(format!(
            "{:?}にはデータディスクが最低1台必要です(合計{}台以上を選択してください)",
            level,
            parity_count + 1
        ));
    }

    let min_size_bytes = req
        .disks
        .iter()
        .map(|d| d.size_bytes)
        .min()
        .unwrap_or(0);
    let stripes_per_disk = min_size_bytes / CHUNK_SIZE as u64;
    if stripes_per_disk == 0 {
        return Err("選択したディスクの容量が小さすぎます".to_string());
    }

    let mut devices = Vec::with_capacity(disk_count);
    for disk in &req.disks {
        let dev = FileBackedDevice::open(&disk.path).map_err(|e| {
            format!(
                "ディスク{}を開けませんでした(管理者権限で実行しているか確認してください): {e}",
                disk.path
            )
        })?;
        devices.push(dev);
    }

    let accel_device = zfs_accel_hlsl::detect_best_accelerator().ok();
    let accelerator = accel_device
        .as_ref()
        .map(|a| format!("{:?}: {}", a.kind, a.adapter_description))
        .unwrap_or_else(|| "検出失敗".to_string());

    let num_data_disks = disk_count as u64 - parity_count as u64;
    let total_stripes = num_data_disks * stripes_per_disk;

    let mut vdev = RaidZVdev::new(devices, level, CHUNK_SIZE);
    if let Some(accel) = accel_device {
        vdev = vdev.with_accelerator(accel);
    }
    let mut pool = Pool::new(vdev, total_stripes);
    // `init_zpool_preview`と同様、スーパーブロック予約後の実容量を使う。
    let usable_stripes = pool.usage().free_stripes;

    pool.create_dataset(&req.dataset_name)
        .map_err(|e| format!("データセットの作成に失敗しました: {e}"))?;
    pool.grow_dataset(&req.dataset_name, usable_stripes * (CHUNK_SIZE as u64 * num_data_disks))
        .map_err(|e| format!("データセットの容量確保に失敗しました: {e}"))?;

    let usage = pool.usage();
    let dataset_size_bytes = pool
        .dataset_size(&req.dataset_name)
        .map_err(|e| format!("データセットサイズの取得に失敗しました: {e}"))?;

    Ok(ZpoolInitResult {
        accelerator,
        total_stripes: usage.total_stripes,
        used_stripes: usage.used_stripes,
        free_stripes: usage.free_stripes,
        dataset_size_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn z2_preview_succeeds_with_enough_disks() {
        let result = init_zpool_preview(ZpoolInitRequest {
            disk_count: 4,
            level: "Z2".to_string(),
            dataset_name: "tank".to_string(),
        })
        .unwrap();

        assert_eq!(result.total_stripes, result.used_stripes);
        assert_eq!(result.free_stripes, 0);
        assert!(result.dataset_size_bytes > 0);
    }

    #[test]
    fn z3_preview_rejects_too_few_disks() {
        let err = init_zpool_preview(ZpoolInitRequest {
            disk_count: 3,
            level: "Z3".to_string(),
            dataset_name: "tank".to_string(),
        })
        .unwrap_err();

        assert!(err.contains("最低1台必要"));
    }

    #[test]
    fn raid0_preview_allows_all_disks_as_data() {
        let result = init_zpool_preview(ZpoolInitRequest {
            disk_count: 4,
            level: "Raid0".to_string(),
            dataset_name: "tank".to_string(),
        })
        .unwrap();
        assert!(result.dataset_size_bytes > 0);
    }

    #[test]
    fn raid1_preview_mirrors_across_all_disks() {
        let result = init_zpool_preview(ZpoolInitRequest {
            disk_count: 4,
            level: "Raid1".to_string(),
            dataset_name: "tank".to_string(),
        })
        .unwrap();
        // ミラーはデータディスク1台ぶんの容量しかない(残り3台は複製)上に、
        // `Pool::new`がスーパーブロック用に1ストライプ予約するため、実際に
        // 使える容量は`STRIPES_PER_DISK - 1`ストライプぶん。
        assert_eq!(result.dataset_size_bytes, (STRIPES_PER_DISK - 1) * CHUNK_SIZE as u64);
    }

    #[test]
    fn raid5_preview_succeeds_with_enough_disks() {
        let result = init_zpool_preview(ZpoolInitRequest {
            disk_count: 4,
            level: "Raid5".to_string(),
            dataset_name: "tank".to_string(),
        })
        .unwrap();
        assert!(result.dataset_size_bytes > 0);
    }

    #[test]
    fn raid6_preview_succeeds_with_enough_disks() {
        let result = init_zpool_preview(ZpoolInitRequest {
            disk_count: 4,
            level: "Raid6".to_string(),
            dataset_name: "tank".to_string(),
        })
        .unwrap();
        assert!(result.dataset_size_bytes > 0);
    }

    #[test]
    fn raid10_preview_creates_dataset_via_pool() {
        let result = init_raid10_preview(Raid10InitRequest {
            disk_count: 4,
            mirror_width: 2,
            dataset_name: "tank".to_string(),
        })
        .unwrap();
        assert_eq!(result.num_groups, 2);
        assert!(result.dataset_size_bytes > 0);
        // CoW用に1ストライプぶんの空きを残しているはず。
        assert_eq!(result.free_stripes, 1);
    }

    #[test]
    fn raid10_preview_rejects_disk_count_not_multiple_of_mirror_width() {
        let err = init_raid10_preview(Raid10InitRequest {
            disk_count: 3,
            mirror_width: 2,
            dataset_name: "tank".to_string(),
        })
        .unwrap_err();
        assert!(err.contains("倍数"));
    }

    #[test]
    fn unknown_level_is_rejected() {
        let err = init_zpool_preview(ZpoolInitRequest {
            disk_count: 4,
            level: "Z1".to_string(),
            dataset_name: "tank".to_string(),
        })
        .unwrap_err();

        assert!(err.contains("未対応のRAIDレベル"));
    }

    fn make_fake_disk(name: &str, stripes: u64) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "open_runo_installer_apply_test_{}_{name}.img",
            std::process::id()
        ));
        FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * stripes).unwrap();
        path
    }

    #[test]
    fn apply_rejects_without_confirmation() {
        let err = init_zpool_apply(ZpoolApplyRequest {
            disks: vec![DiskSelection {
                path: "irrelevant".to_string(),
                size_bytes: CHUNK_SIZE as u64 * 16,
            }],
            level: "Raid0".to_string(),
            dataset_name: "tank".to_string(),
            confirm_data_loss: false,
        })
        .unwrap_err();
        assert!(err.contains("確認が必要"));
    }

    #[test]
    fn apply_writes_to_selected_disk_paths() {
        let disk_a = make_fake_disk("a", 16);
        let disk_b = make_fake_disk("b", 16);

        let result = init_zpool_apply(ZpoolApplyRequest {
            disks: vec![
                DiskSelection {
                    path: disk_a.to_string_lossy().into_owned(),
                    size_bytes: CHUNK_SIZE as u64 * 16,
                },
                DiskSelection {
                    path: disk_b.to_string_lossy().into_owned(),
                    size_bytes: CHUNK_SIZE as u64 * 16,
                },
            ],
            level: "Raid1".to_string(),
            dataset_name: "tank".to_string(),
            confirm_data_loss: true,
        })
        .unwrap();

        assert!(result.dataset_size_bytes > 0);

        std::fs::remove_file(&disk_a).ok();
        std::fs::remove_file(&disk_b).ok();
    }

    #[test]
    fn apply_rejects_unreadable_disk_path() {
        let err = init_zpool_apply(ZpoolApplyRequest {
            disks: vec![
                DiskSelection {
                    path: "\\\\.\\ThisDeviceDoesNotExist12345".to_string(),
                    size_bytes: CHUNK_SIZE as u64 * 16,
                },
                DiskSelection {
                    path: "\\\\.\\ThisDeviceDoesNotExist67890".to_string(),
                    size_bytes: CHUNK_SIZE as u64 * 16,
                },
            ],
            level: "Raid1".to_string(),
            dataset_name: "tank".to_string(),
            confirm_data_loss: true,
        })
        .unwrap_err();
        assert!(err.contains("開けませんでした"));
    }

    #[test]
    fn apply_rejects_disks_too_small_for_a_single_chunk() {
        let disk_a = make_fake_disk("small_a", 0);
        let disk_b = make_fake_disk("small_b", 0);

        let err = init_zpool_apply(ZpoolApplyRequest {
            disks: vec![
                DiskSelection {
                    path: disk_a.to_string_lossy().into_owned(),
                    size_bytes: 0,
                },
                DiskSelection {
                    path: disk_b.to_string_lossy().into_owned(),
                    size_bytes: 0,
                },
            ],
            level: "Raid1".to_string(),
            dataset_name: "tank".to_string(),
            confirm_data_loss: true,
        })
        .unwrap_err();
        assert!(err.contains("容量が小さすぎます"));

        std::fs::remove_file(&disk_a).ok();
        std::fs::remove_file(&disk_b).ok();
    }
}
