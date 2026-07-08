//! zpool初期化ウィザードのバックエンドロジック。
//!
//! 【安全設計】実際の物理ディスクへ直接書き込むのは事故が大きすぎるため、
//! 本ウィザードはまず一時ファイル(スクラッチイメージ)上でプールを構築して
//! 動作を確認する「プレビュー」モードのみを提供する。実ディスクへの適用は、
//! VHDXアタッチ等で得られる`\\.\PhysicalDriveN`パスを
//! `openzfs_winfsp_bridge::block_device::FileBackedDevice::open`へそのまま
//! 渡せる設計になっている(`block_device.rs`のドキュメント参照)ため、将来
//! UI側に「実ディスクへ適用」ボタンを追加するだけで対応できる。

use openzfs_winfsp_bridge::block_device::FileBackedDevice;
use openzfs_winfsp_bridge::pool::Pool;
use openzfs_winfsp_bridge::raid10::Raid10Vdev;
use openzfs_winfsp_bridge::vdev::{RaidLevel, RaidZVdev};
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
            "openruno_installer_preview_{}_{}.img",
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

    pool.create_dataset(&req.dataset_name)
        .map_err(|e| format!("データセットの作成に失敗しました: {e}"))?;
    pool.grow_dataset(&req.dataset_name, total_stripes * (CHUNK_SIZE as u64 * num_data_disks))
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
}

#[derive(Debug, Serialize)]
pub struct Raid10InitResult {
    pub accelerator: String,
    pub num_groups: usize,
    /// プレビューとして実際に1ストライプぶん書き込み・読み出しを行い、
    /// 内容が一致したことを確認できたかどうか。
    pub round_trip_verified: bool,
}

/// RAID10(ストライプ+ミラー)のプレビュー。
///
/// 【現状の制約】[`Pool`]はまだ`RaidZVdev`専用のため、RAID10は
/// `Pool`を経由しない単体の[`Raid10Vdev`]として動作確認する
/// (`raid10.rs`のモジュールドキュメント参照)。データセット容量計算などの
/// `Pool`機能とはまだ統合されていない。
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
            "openruno_installer_raid10_preview_{}_{}.img",
            std::process::id(),
            i
        ));
        let dev = FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * STRIPES_PER_DISK)
            .map_err(|e| format!("スクラッチイメージの作成に失敗しました: {e}"))?;
        scratch_paths.push(path);
        devices.push(dev);
    }

    let accelerator = zfs_accel_hlsl::detect_best_accelerator()
        .map(|a| format!("{:?}: {}", a.kind, a.adapter_description))
        .unwrap_or_else(|e| format!("検出失敗: {e}"));

    let mut vdev = Raid10Vdev::new(devices, mirror_width, CHUNK_SIZE)
        .map_err(|e| format!("RAID10 vdevの構築に失敗しました: {e}"))?;
    let num_groups = vdev.num_groups();

    let sample: Vec<u8> = (0..CHUNK_SIZE).map(|i| (i % 256) as u8).collect();
    vdev.write_stripe(0, &sample)
        .map_err(|e| format!("プレビュー書き込みに失敗しました: {e}"))?;
    let read_back = vdev
        .read_stripe(0)
        .map_err(|e| format!("プレビュー読み出しに失敗しました: {e}"))?;
    let round_trip_verified = read_back == sample;

    for path in scratch_paths {
        std::fs::remove_file(&path).ok();
    }

    Ok(Raid10InitResult {
        accelerator,
        num_groups,
        round_trip_verified,
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
        // ミラーはデータディスク1台ぶんの容量しかない(残り3台は複製)。
        assert_eq!(result.dataset_size_bytes, (STRIPES_PER_DISK * CHUNK_SIZE as u64));
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
    fn raid10_preview_round_trips_across_mirror_groups() {
        let result = init_raid10_preview(Raid10InitRequest {
            disk_count: 4,
            mirror_width: 2,
        })
        .unwrap();
        assert_eq!(result.num_groups, 2);
        assert!(result.round_trip_verified);
    }

    #[test]
    fn raid10_preview_rejects_disk_count_not_multiple_of_mirror_width() {
        let err = init_raid10_preview(Raid10InitRequest {
            disk_count: 3,
            mirror_width: 2,
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
}
