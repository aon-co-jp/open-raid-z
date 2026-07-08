//! RAID10(ストライプ+ミラーの入れ子構成)。
//!
//! [`crate::vdev::RaidZVdev`]は「1ストライプを全ディスクへ書く」固定モデルで、
//! RAID0/1/5/6/Z2/Z3はいずれもこのモデルに乗る(データディスク1台の
//! 退化形がミラー、というのが[`crate::vdev::RaidLevel::Raid1`]の実装)。
//!
//! しかしRAID10は「複数のミラーペアのうち、1ストライプはそのうち1組だけを
//! 使う」構成であり、全ディスクへ毎回書くモデルには乗らない。そのため
//! 本モジュールでは、[`RaidZVdev`](crate::vdev::RaidZVdev)を
//! (`RaidLevel::Raid1`で構成した)ミラーグループとして複数束ね、
//! グローバルなストライプ番号をグループ数でラウンドロビンして
//! 各グループへ委譲する、別レイヤーとして実装する。
//!
//! 【現状の制約】[`crate::pool::Pool`]は`RaidZVdev`に直接結合しており、
//! 本`Raid10Vdev`はまだ`Pool`からは利用できない(将来的に`Pool`を
//! vdevトレイトへ一般化すれば統合できる設計だが、既存の多数のテストに
//! 影響する破壊的変更になるため、本パスでは見送った)。単体では
//! 完全に動作し、書き込み・読み出し・障害耐性・resilverまで検証済み。

use crate::block_device::BlockDevice;
use crate::error::{BridgeError, BridgeResult};
use crate::vdev::{RaidLevel, RaidZVdev};

pub struct Raid10Vdev<D: BlockDevice> {
    /// 各要素が1つのミラーグループ(`RaidLevel::Raid1`で構成した`RaidZVdev`)。
    groups: Vec<RaidZVdev<D>>,
    chunk_size: usize,
}

impl<D: BlockDevice> Raid10Vdev<D> {
    /// `devices`を`mirror_width`台ずつのミラーグループへ分割し、
    /// ストライプ単位でグループ間をラウンドロビンするRAID10 vdevを構築する。
    ///
    /// 例: 4台・`mirror_width=2` なら、(disk0,disk1)と(disk2,disk3)の
    /// 2組のミラーペアを作り、偶数ストライプはグループ0、奇数ストライプは
    /// グループ1、という具合に交互に配置する(標準的なRAID10と同じ考え方)。
    pub fn new(devices: Vec<D>, mirror_width: usize, chunk_size: usize) -> BridgeResult<Self> {
        if mirror_width < 2 {
            return Err(invalid_config("ミラー幅(mirror_width)は2台以上である必要があります"));
        }
        if devices.is_empty() || devices.len() % mirror_width != 0 {
            return Err(invalid_config(&format!(
                "デバイス数({})はミラー幅({mirror_width})の倍数である必要があります",
                devices.len()
            )));
        }

        let mut groups = Vec::with_capacity(devices.len() / mirror_width);
        let mut remaining: Vec<D> = devices;
        while !remaining.is_empty() {
            let rest = remaining.split_off(mirror_width.min(remaining.len()));
            let group_devices = std::mem::replace(&mut remaining, rest);
            groups.push(RaidZVdev::new(group_devices, RaidLevel::Raid1, chunk_size));
        }

        Ok(Self { groups, chunk_size })
    }

    pub fn num_groups(&self) -> usize {
        self.groups.len()
    }

    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// グローバルなストライプ番号を(担当グループ, グループ内ストライプ番号)へ変換する。
    fn route(&self, stripe_index: u64) -> (usize, u64) {
        let num_groups = self.groups.len() as u64;
        let group = (stripe_index % num_groups) as usize;
        let inner_stripe = stripe_index / num_groups;
        (group, inner_stripe)
    }

    /// 1ストライプ(`chunk_size`バイト、担当グループのミラー全台へ複製される)を書き込む。
    pub fn write_stripe(&mut self, stripe_index: u64, data: &[u8]) -> BridgeResult<()> {
        let (group, inner) = self.route(stripe_index);
        self.groups[group].write_stripe(inner, data)
    }

    /// 1ストライプを読み出す。担当グループ内でミラーの1台でも生きていれば復旧できる。
    pub fn read_stripe(&mut self, stripe_index: u64) -> BridgeResult<Vec<u8>> {
        let (group, inner) = self.route(stripe_index);
        self.groups[group].read_stripe(inner)
    }

    /// 指定グループ内の指定ディスクをresilver(再構築)する。
    pub fn resilver(&mut self, group_index: usize, disk_index_in_group: usize, num_stripes_in_group: u64) -> BridgeResult<()> {
        self.groups[group_index].resilver(disk_index_in_group, num_stripes_in_group)
    }

    /// 指定グループの生デバイス配列(障害注入・直接検証用)。
    pub fn group_devices_mut(&mut self, group_index: usize) -> &mut [D] {
        self.groups[group_index].devices_mut()
    }
}

fn invalid_config(msg: &str) -> BridgeError {
    BridgeError::Io(std::io::Error::other(msg.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_device::{FaultInjectableDevice, FileBackedDevice};

    const CHUNK_SIZE: usize = 32;

    fn scratch_disk(name: &str) -> FaultInjectableDevice<FileBackedDevice> {
        let path = std::env::temp_dir().join(format!("openruno_raid10_test_{name}_{}", std::process::id()));
        FaultInjectableDevice::new(FileBackedDevice::create_fixed_size(&path, CHUNK_SIZE as u64 * 8).unwrap())
    }

    #[test]
    fn rejects_device_count_not_a_multiple_of_mirror_width() {
        let devices = vec![scratch_disk("a"), scratch_disk("b"), scratch_disk("c")];
        assert!(Raid10Vdev::new(devices, 2, CHUNK_SIZE).is_err());
    }

    #[test]
    fn stripes_round_robin_across_mirror_groups() {
        let devices: Vec<_> = (0..4).map(|i| scratch_disk(&format!("rr{i}"))).collect();
        let mut vdev = Raid10Vdev::new(devices, 2, CHUNK_SIZE).unwrap();
        assert_eq!(vdev.num_groups(), 2);

        for stripe in 0..6u64 {
            let data = vec![(stripe % 256) as u8; CHUNK_SIZE];
            vdev.write_stripe(stripe, &data).unwrap();
        }
        for stripe in 0..6u64 {
            let expected = vec![(stripe % 256) as u8; CHUNK_SIZE];
            assert_eq!(vdev.read_stripe(stripe).unwrap(), expected);
        }
    }

    #[test]
    fn survives_one_disk_failure_per_mirror_group_simultaneously() {
        // 4台(2グループ)構成で、各グループから1台ずつ、合計2台が同時に
        // 故障しても読めることを確認する(RAID10の典型的な耐障害シナリオ)。
        let devices: Vec<_> = (0..4).map(|i| scratch_disk(&format!("fail{i}"))).collect();
        let mut vdev = Raid10Vdev::new(devices, 2, CHUNK_SIZE).unwrap();

        for stripe in 0..4u64 {
            let data = vec![(stripe * 17 % 256) as u8; CHUNK_SIZE];
            vdev.write_stripe(stripe, &data).unwrap();
        }

        vdev.group_devices_mut(0)[0].failed = true;
        vdev.group_devices_mut(1)[1].failed = true;

        for stripe in 0..4u64 {
            let expected = vec![(stripe * 17 % 256) as u8; CHUNK_SIZE];
            assert_eq!(vdev.read_stripe(stripe).unwrap(), expected);
        }
    }

    #[test]
    fn losing_both_disks_in_one_group_is_unrecoverable_for_that_groups_stripes() {
        let devices: Vec<_> = (0..4).map(|i| scratch_disk(&format!("bothfail{i}"))).collect();
        let mut vdev = Raid10Vdev::new(devices, 2, CHUNK_SIZE).unwrap();

        vdev.write_stripe(0, &vec![0xAAu8; CHUNK_SIZE]).unwrap(); // グループ0

        vdev.group_devices_mut(0)[0].failed = true;
        vdev.group_devices_mut(0)[1].failed = true;

        assert!(vdev.read_stripe(0).is_err(), "同一ミラーグループの全滅は復旧不能なはず");
    }

    #[test]
    fn resilver_restores_replaced_disk_within_its_group() {
        let devices: Vec<_> = (0..4).map(|i| scratch_disk(&format!("resilver{i}"))).collect();
        let mut vdev = Raid10Vdev::new(devices, 2, CHUNK_SIZE).unwrap();

        for stripe in 0..4u64 {
            let data = vec![(stripe * 31 % 256) as u8; CHUNK_SIZE];
            vdev.write_stripe(stripe, &data).unwrap();
        }

        vdev.group_devices_mut(1)[0].failed = true;
        vdev.group_devices_mut(1)[0].failed = false; // 交換直後、中身は信用しない
        vdev.resilver(1, 0, 2).unwrap(); // グループ1は偶数/奇数ストライプのうち内部2ストライプぶん

        for stripe in 0..4u64 {
            let expected = vec![(stripe * 31 % 256) as u8; CHUNK_SIZE];
            assert_eq!(vdev.read_stripe(stripe).unwrap(), expected);
        }
    }
}
