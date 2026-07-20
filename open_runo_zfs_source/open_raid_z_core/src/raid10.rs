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
//! [`crate::vdev::Vdev`]トレイトを実装しているため、[`crate::pool::Pool`]
//! からも`RaidZVdev`と同じように利用できる(`Pool<Raid10Vdev<D>>`)。
//! `Vdev::num_data_disks`は常に1を返す点に注意(1回の`write_stripe`は
//! 担当グループ1組ぶん=`chunk_size`バイトのみを扱うため。集約的な
//! 並列度を知りたい場合は[`Raid10Vdev::num_groups`]を使うこと)。

use crate::block_device::BlockDevice;
use crate::error::{BridgeError, BridgeResult};
use crate::vdev::{RaidLevel, RaidZVdev, Vdev};

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
        if devices.is_empty() || !devices.len().is_multiple_of(mirror_width) {
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
    pub fn resilver(
        &mut self,
        group_index: usize,
        disk_index_in_group: usize,
        num_stripes_in_group: u64,
    ) -> BridgeResult<()> {
        self.groups[group_index].resilver(disk_index_in_group, num_stripes_in_group)
    }

    /// 指定グループの生デバイス配列(障害注入・直接検証用)。
    pub fn group_devices_mut(&mut self, group_index: usize) -> &mut [D] {
        self.groups[group_index].devices_mut()
    }

    /// `RaidZVdev::scrub`のRAID10版。全ミラーグループを横断して
    /// チェックサム不一致(サイレント破損)を検知・自己修復する
    /// (ZFSの`zpool scrub`に相当)。
    ///
    /// `total_stripes`は[`Self::route`]が扱うのと同じ**グローバルな**
    /// ストライプ数(`Pool::usage().total_stripes`相当)。内部で
    /// ラウンドロビン配置に基づき各グループが担当する実際のストライプ数
    /// (`total_stripes`がグループ数で割り切れない場合、余りの分だけ
    /// 若い番号のグループが1つ多く担当する)へ変換してから、各グループの
    /// `RaidZVdev::scrub`へ委譲する。
    pub fn scrub(&mut self, total_stripes: u64) -> BridgeResult<crate::vdev::ScrubReport> {
        let num_groups = self.groups.len() as u64;
        let mut report = crate::vdev::ScrubReport::default();
        for (group_index, group) in self.groups.iter_mut().enumerate() {
            let group_index = group_index as u64;
            // group_indexが担当するグローバルストライプは
            // group_index, group_index+num_groups, group_index+2*num_groups, ...
            // なので、その個数はtotal_stripesをnum_groupsで割った商に、
            // 余りがgroup_indexより大きければ+1したもの。
            let base = total_stripes / num_groups;
            let remainder = total_stripes % num_groups;
            let stripes_in_group = base + if group_index < remainder { 1 } else { 0 };

            let group_report = group.scrub(stripes_in_group)?;
            report.stripes_scanned += group_report.stripes_scanned;
            report.corruptions_healed += group_report.corruptions_healed;
        }
        Ok(report)
    }
}

fn invalid_config(msg: &str) -> BridgeError {
    BridgeError::InvalidConfig(msg.to_string())
}

impl<D: BlockDevice> Vdev for Raid10Vdev<D> {
    /// 常に1(1回の`write_stripe`は担当グループ1組ぶん=`chunk_size`バイトのみを扱うため)。
    fn num_data_disks(&self) -> usize {
        1
    }

    fn chunk_size(&self) -> usize {
        Raid10Vdev::chunk_size(self)
    }

    fn write_stripe(&mut self, stripe_index: u64, data: &[u8]) -> BridgeResult<()> {
        Raid10Vdev::write_stripe(self, stripe_index, data)
    }

    fn read_stripe(&mut self, stripe_index: u64) -> BridgeResult<Vec<u8>> {
        Raid10Vdev::read_stripe(self, stripe_index)
    }

    fn scrub(&mut self, num_stripes: u64) -> BridgeResult<crate::vdev::ScrubReport> {
        Raid10Vdev::scrub(self, num_stripes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_device::{BlockDevice, FaultInjectableDevice, FileBackedDevice};

    const CHUNK_SIZE: usize = 32;

    fn scratch_disk(name: &str) -> FaultInjectableDevice<FileBackedDevice> {
        let path = std::env::temp_dir().join(format!("open_runo_raid10_test_{name}_{}", std::process::id()));
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

        vdev.write_stripe(0, &[0xAAu8; CHUNK_SIZE]).unwrap(); // グループ0

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

    /// 直接ディスクの中身だけを壊す(`failed`フラグは立てない、ビットロットの
    /// シミュレーション。`tests/checksum_self_healing.rs`と同じ手法)。
    fn corrupt_group_disk_directly<D: BlockDevice>(
        vdev: &mut Raid10Vdev<D>,
        group_index: usize,
        disk_index_in_group: usize,
        inner_stripe: u64,
    ) {
        let offset = inner_stripe * CHUNK_SIZE as u64;
        let disk = &mut vdev.group_devices_mut(group_index)[disk_index_in_group];
        let mut garbage = disk.read_at(offset, CHUNK_SIZE).unwrap();
        for b in garbage.iter_mut() {
            *b ^= 0xFF;
        }
        disk.write_at(offset, &garbage).unwrap();
    }

    #[test]
    fn scrub_detects_and_heals_silent_corruption_within_a_group() {
        let devices: Vec<_> = (0..4).map(|i| scratch_disk(&format!("scrub{i}"))).collect();
        let mut vdev = Raid10Vdev::new(devices, 2, CHUNK_SIZE).unwrap();
        let total_stripes = 6u64;

        for stripe in 0..total_stripes {
            let data = vec![(stripe * 13 % 256) as u8; CHUNK_SIZE];
            vdev.write_stripe(stripe, &data).unwrap();
        }

        // グローバルストライプ0(グループ0の内部ストライプ0)を、グループ0の
        // 2台目のミラーメンバーだけビットロットさせる。
        corrupt_group_disk_directly(&mut vdev, 0, 1, 0);

        let report = vdev.scrub(total_stripes).expect("scrubに失敗");
        assert_eq!(report.stripes_scanned, total_stripes);
        assert_eq!(report.corruptions_healed, 1);

        for stripe in 0..total_stripes {
            let expected = vec![(stripe * 13 % 256) as u8; CHUNK_SIZE];
            assert_eq!(vdev.read_stripe(stripe).unwrap(), expected, "stripe {stripe}");
        }
    }

    #[test]
    fn scrub_correctly_splits_uneven_stripe_counts_across_groups() {
        // 3グループ(6台, mirror_width=2)・グローバルストライプ7つ(3の倍数
        // ではない)という構成で、余りの分配とscrub範囲が正しいことを検証する。
        let devices: Vec<_> = (0..6).map(|i| scratch_disk(&format!("uneven{i}"))).collect();
        let mut vdev = Raid10Vdev::new(devices, 2, CHUNK_SIZE).unwrap();
        assert_eq!(vdev.num_groups(), 3);

        let total_stripes = 7u64;
        for stripe in 0..total_stripes {
            let data = vec![(stripe * 19 % 256) as u8; CHUNK_SIZE];
            vdev.write_stripe(stripe, &data).unwrap();
        }

        // route(6) = (6 % 3, 6 / 3) = (グループ0, 内部ストライプ2)。
        // これはグループ0が担当する「余り分」の3番目のストライプであり、
        // 余りの分配ロジック(`Raid10Vdev::scrub`のremainder計算)が誤っていると
        // スキャン範囲から漏れて検知されない。
        corrupt_group_disk_directly(&mut vdev, 0, 0, 2);

        let report = vdev.scrub(total_stripes).expect("scrubに失敗");
        assert_eq!(
            report.stripes_scanned, total_stripes,
            "グローバルストライプ全体がちょうど1回ずつスキャンされるはず"
        );
        assert_eq!(report.corruptions_healed, 1);

        for stripe in 0..total_stripes {
            let expected = vec![(stripe * 19 % 256) as u8; CHUNK_SIZE];
            assert_eq!(vdev.read_stripe(stripe).unwrap(), expected, "stripe {stripe}");
        }
    }
}
