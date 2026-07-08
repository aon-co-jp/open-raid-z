//! ZFSのストレージプール(複数ディスクを1つの記憶領域にまとめ、その中から
//! 必要な容量を切り出して複数のファイルシステム(データセット)を作成できる)
//! を模した層。
//!
//! [`crate::vdev::RaidZVdev`]が提供する総ストライプ数を「プールの容量」として
//! 扱い、その中から複数の名前付きデータセットへストライプ単位で動的に
//! 容量を割り当てる。個々のディスクの容量に縛られず、プール全体の空き容量の
//! 範囲で複数のファイルシステムを柔軟に運用できる、というZFSの特徴を模している。
//!
//! 【現状の制約】
//! - 1プール = 1vdevのみ対応(複数vdevをまたぐプールの容量拡張は未実装)
//! - ストライプ境界単位での粗い割り当て(ZFSのメタスラブ/SPAほど細かい
//!   バイト単位のアロケータではない)
//!
//! ## 任意オフセットの読み書き([`Pool::read_unaligned`] / [`Pool::write_unaligned`])
//!
//! [`Pool::read`] / [`Pool::write`]はストライプ境界(`chunk_size ×
//! num_data_disks`)に一致するオフセット・長さしか受け付けない
//! (WinFspマウント層(`mount.rs`)がこの制約を抱えたままになっている
//! 主因)。[`Pool::read_unaligned`] / [`Pool::write_unaligned`]は、
//! 要求範囲を含む最小のストライプ境界範囲へ内部的に切り上げてから
//! 既存の`read`/`write`へ委譲する read-modify-write 層であり、
//! バイト単位の任意オフセット・任意長の読み書きを提供する。
//! 書き込みは対象範囲全体を一度読み出してから書き戻すため、境界を
//! はみ出さない未変更部分のバイトは保持される。内部で使う`write`が
//! CoWで実装されているため、read-modify-write全体としてもCoW特性
//! (書き込み失敗時に既存データが無傷)を保つ。
//!
//! ## コピーオンライト(CoW)
//!
//! `Dataset.stripes`は「論理ストライプ番号 -> 物理ストライプ番号」の間接参照
//! テーブルであり、これがそのままCoWの実装基盤になる。[`Pool::write`]は
//! 既存の物理ストライプを上書きするのではなく、
//!
//! 1. 新しいデータを**空き**物理ストライプへ書き込み、
//! 2. 書き込みが成功して初めて、論理ストライプ番号が指す物理ストライプ番号を
//!    新しい方へ差し替え、
//! 3. (スナップショット等の他参照が無ければ)古い物理ストライプを空き領域へ
//!    返却する
//!
//! という順序で行う。書き込み中に電源断等でクラッシュしても、ステップ2の
//! 参照切り替えが完了するまでは古いデータが指されたままなので、
//! データが破壊されることはない(ZFSのCoWと同じ考え方)。
//!
//! ## スナップショット・クローン
//!
//! CoWの間接参照テーブルのおかげで、スナップショットは「ある時点の
//! `stripes`配列を複製して保持する」だけで作成できる(実データはコピーせず、
//! 物理ストライプ番号の一覧、たかだか数バイト〜数十バイトのコピーで済む)。
//! 各物理ストライプには参照カウント(`ref_counts`)を持たせ、
//! データセット・スナップショット・クローンのいずれかから参照されている間は
//! 空き領域へ返却しない。これにより、スナップショット作成後に元データセット
//! 側でCoW書き込みが起きても、スナップショットが指す古いストライプは
//! (元データセットからの参照が外れても)生き残り続ける。
//!
//! クローンはスナップショットの`stripes`配列をコピーして新しい書き込み可能な
//! データセットとして登録するだけで作成できる(この時点ではブロックを一切
//! 複製しない)。クローンへの書き込みはCoW経由で新しいストライプへ分岐する
//! ため、スナップショット・クローンいずれのデータも壊れない。

use crate::error::{BridgeError, BridgeResult};
use crate::vdev::Vdev;
use std::collections::HashMap;

pub struct Pool<V: Vdev> {
    vdev: V,
    total_stripes: u64,
    /// 空きストライプのインデックス集合(スタックとして扱う: popで払い出す)
    free_stripes: Vec<u64>,
    /// 割当済み(=free_stripesに無い)物理ストライプの参照カウント。
    /// データセット・スナップショット・クローンから参照されるたびに+1、
    /// 参照が外れるたびに-1し、0になったら空き領域へ返却する。
    ref_counts: HashMap<u64, u32>,
    datasets: HashMap<String, Dataset>,
    /// キーは`"データセット名@スナップショット名"`。
    snapshots: HashMap<String, Snapshot>,
}

#[derive(Debug, Clone, Default)]
struct Dataset {
    /// このデータセットが保持する物理ストライプのインデックス列(論理順)
    stripes: Vec<u64>,
}

#[derive(Debug, Clone, Default)]
struct Snapshot {
    /// スナップショット作成時点の物理ストライプのインデックス列(論理順・不変)
    stripes: Vec<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolUsage {
    pub total_stripes: u64,
    pub used_stripes: u64,
    pub free_stripes: u64,
}

fn not_found(name: &str) -> BridgeError {
    BridgeError::DatasetNotFound(name.to_string())
}

fn snapshot_not_found(key: &str) -> BridgeError {
    BridgeError::SnapshotNotFound(key.to_string())
}

impl<V: Vdev> Pool<V> {
    /// `total_stripes`個のストライプ(=`vdev`の総容量)を持つプールを作成する。
    pub fn new(vdev: V, total_stripes: u64) -> Self {
        let free_stripes = (0..total_stripes).rev().collect();
        Self {
            vdev,
            total_stripes,
            free_stripes,
            ref_counts: HashMap::new(),
            datasets: HashMap::new(),
            snapshots: HashMap::new(),
        }
    }

    pub fn usage(&self) -> PoolUsage {
        PoolUsage {
            total_stripes: self.total_stripes,
            used_stripes: self.total_stripes - self.free_stripes.len() as u64,
            free_stripes: self.free_stripes.len() as u64,
        }
    }

    fn chunk_bytes(&self) -> u64 {
        (self.vdev.num_data_disks() * self.vdev.chunk_size()) as u64
    }

    /// 空きストライプを1つ払い出し、参照カウント1で確保済みにする。
    fn claim_stripe(&mut self) -> BridgeResult<u64> {
        let stripe = self
            .free_stripes
            .pop()
            .ok_or_else(|| BridgeError::CapacityExceeded("プールに空きストライプがありません".to_string()))?;
        self.ref_counts.insert(stripe, 1);
        Ok(stripe)
    }

    /// 既に確保済みのストライプへ新たな参照を追加する(参照カウント+1)。
    /// スナップショット・クローン作成時に使う。
    fn retain_stripe(&mut self, stripe: u64) {
        *self.ref_counts.entry(stripe).or_insert(0) += 1;
    }

    /// ストライプへの参照を1つ手放す(参照カウント-1)。0になったら
    /// 空き領域へ返却する。
    fn release_stripe(&mut self, stripe: u64) {
        if let Some(count) = self.ref_counts.get_mut(&stripe) {
            *count -= 1;
            if *count == 0 {
                self.ref_counts.remove(&stripe);
                self.free_stripes.push(stripe);
            }
        }
    }

    pub fn create_dataset(&mut self, name: &str) -> BridgeResult<()> {
        if self.datasets.contains_key(name) {
            return Err(BridgeError::AlreadyExists(format!("データセット'{name}'")));
        }
        self.datasets.insert(name.to_string(), Dataset::default());
        Ok(())
    }

    /// データセットを破棄し、他から参照されていないストライプをプールへ
    /// 返却する(スナップショット・クローンから参照中のストライプは
    /// 参照カウントが残るため、誤って解放されることはない)。
    pub fn destroy_dataset(&mut self, name: &str) -> BridgeResult<()> {
        let dataset = self.datasets.remove(name).ok_or_else(|| not_found(name))?;
        for stripe in dataset.stripes {
            self.release_stripe(stripe);
        }
        Ok(())
    }

    pub fn dataset_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.datasets.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn dataset_size(&self, name: &str) -> BridgeResult<u64> {
        let ds = self.datasets.get(name).ok_or_else(|| not_found(name))?;
        Ok(ds.stripes.len() as u64 * self.chunk_bytes())
    }

    /// データセットの割当容量を`additional_bytes`ぶん拡張する
    /// (プールに十分な空きがある場合のみ)。ZFSの「必要な分だけ動的に
    /// 確保する」プール容量管理を模す。
    pub fn grow_dataset(&mut self, name: &str, additional_bytes: u64) -> BridgeResult<()> {
        if !self.datasets.contains_key(name) {
            return Err(not_found(name));
        }
        let chunk_bytes = self.chunk_bytes();
        let additional_stripes = additional_bytes.div_ceil(chunk_bytes);

        if additional_stripes > self.free_stripes.len() as u64 {
            return Err(BridgeError::CapacityExceeded(format!(
                "プールの空き容量が不足しています(必要{additional_stripes}ストライプ、空き{}ストライプ)",
                self.free_stripes.len()
            )));
        }

        let mut newly_allocated = Vec::with_capacity(additional_stripes as usize);
        for _ in 0..additional_stripes {
            newly_allocated.push(self.claim_stripe()?);
        }

        self.datasets.get_mut(name).unwrap().stripes.extend(newly_allocated);
        Ok(())
    }

    /// データセットへストライプ境界単位でコピーオンライト書き込みを行う。
    ///
    /// 論理ストライプ位置ごとに新しい空き物理ストライプへデータを書き、
    /// 成功した場合のみそのストライプの参照先を新しい物理ストライプへ
    /// 切り替える。既存の物理ストライプの中身は(参照が切り替わるまで)
    /// 一切変更しない。
    pub fn write(&mut self, name: &str, logical_offset: u64, data: &[u8]) -> BridgeResult<()> {
        let chunk_bytes = self.chunk_bytes();
        assert_eq!(
            logical_offset % chunk_bytes,
            0,
            "現状の実装ではストライプ境界への書き込みのみサポート"
        );
        assert_eq!(
            data.len() as u64 % chunk_bytes,
            0,
            "書き込みサイズはストライプ境界の倍数である必要があります"
        );

        let start = (logical_offset / chunk_bytes) as usize;
        let count = (data.len() as u64 / chunk_bytes) as usize;

        {
            let ds = self.datasets.get(name).ok_or_else(|| not_found(name))?;
            if start + count > ds.stripes.len() {
                return Err(BridgeError::CapacityExceeded(format!(
                    "データセット'{name}'の割当容量を超える書き込みです(grow_datasetが必要)"
                )));
            }
        }

        for i in 0..count {
            let logical_idx = start + i;
            let chunk_start = i * chunk_bytes as usize;
            let chunk = &data[chunk_start..chunk_start + chunk_bytes as usize];

            // 1. まず空き物理ストライプへ新データを書く(既存データには一切触れない)
            let new_phys = self.claim_stripe()?;
            if let Err(e) = self.vdev.write_stripe(new_phys, chunk) {
                // 書き込みに失敗した場合は確保したストライプを解放し、
                // 参照テーブルには一切触れない(=古いデータは無傷のまま)
                self.release_stripe(new_phys);
                return Err(e);
            }

            // 2. 書き込み成功後にのみ、参照(論理->物理)を新ストライプへ切り替える
            let old_phys = {
                let ds = self.datasets.get_mut(name).unwrap();
                let old = ds.stripes[logical_idx];
                ds.stripes[logical_idx] = new_phys;
                old
            };

            // 3. 旧物理ストライプへの、このデータセットからの参照を手放す。
            //    スナップショット・クローンからまだ参照されていれば
            //    参照カウントが残るため、実際には解放されず生き残る。
            self.release_stripe(old_phys);
        }
        Ok(())
    }

    fn snapshot_key(dataset_name: &str, snapshot_name: &str) -> String {
        format!("{dataset_name}@{snapshot_name}")
    }

    /// データセットの現在の状態を指すスナップショットを作成する。
    ///
    /// 実データを一切コピーしない(物理ストライプ番号の一覧を複製して
    /// 参照カウントを増やすだけ)ため、データセットのサイズに関わらず
    /// 一瞬で完了し、消費容量もほぼ0(ZFSのスナップショットと同じ特性)。
    pub fn create_snapshot(&mut self, dataset_name: &str, snapshot_name: &str) -> BridgeResult<()> {
        let key = Self::snapshot_key(dataset_name, snapshot_name);
        if self.snapshots.contains_key(&key) {
            return Err(BridgeError::AlreadyExists(format!("スナップショット'{key}'")));
        }
        let stripes = self
            .datasets
            .get(dataset_name)
            .ok_or_else(|| not_found(dataset_name))?
            .stripes
            .clone();

        for &stripe in &stripes {
            self.retain_stripe(stripe);
        }
        self.snapshots.insert(key, Snapshot { stripes });
        Ok(())
    }

    /// スナップショットを破棄し、他から参照されていないストライプを
    /// プールへ返却する。
    pub fn destroy_snapshot(&mut self, dataset_name: &str, snapshot_name: &str) -> BridgeResult<()> {
        let key = Self::snapshot_key(dataset_name, snapshot_name);
        let snapshot = self.snapshots.remove(&key).ok_or_else(|| snapshot_not_found(&key))?;
        for stripe in snapshot.stripes {
            self.release_stripe(stripe);
        }
        Ok(())
    }

    pub fn snapshot_names(&self, dataset_name: &str) -> Vec<String> {
        let prefix = format!("{dataset_name}@");
        let mut names: Vec<String> = self
            .snapshots
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .map(|k| k[prefix.len()..].to_string())
            .collect();
        names.sort();
        names
    }

    pub fn snapshot_size(&self, dataset_name: &str, snapshot_name: &str) -> BridgeResult<u64> {
        let key = Self::snapshot_key(dataset_name, snapshot_name);
        let snapshot = self.snapshots.get(&key).ok_or_else(|| snapshot_not_found(&key))?;
        Ok(snapshot.stripes.len() as u64 * self.chunk_bytes())
    }

    /// スナップショットからストライプ境界単位で読み込む(スナップショットは
    /// 不変なので書き込みは提供しない。書き込みが必要な場合は
    /// [`Self::create_clone`]でクローンを作ること)。
    pub fn read_snapshot(
        &mut self,
        dataset_name: &str,
        snapshot_name: &str,
        logical_offset: u64,
        len: u64,
    ) -> BridgeResult<Vec<u8>> {
        let chunk_bytes = self.chunk_bytes();
        assert_eq!(logical_offset % chunk_bytes, 0, "ストライプ境界のみサポート");
        assert_eq!(len % chunk_bytes, 0, "ストライプ境界の倍数のみサポート");

        let key = Self::snapshot_key(dataset_name, snapshot_name);
        let start = (logical_offset / chunk_bytes) as usize;
        let count = (len / chunk_bytes) as usize;

        let physical_stripes: Vec<u64> = {
            let snapshot = self.snapshots.get(&key).ok_or_else(|| snapshot_not_found(&key))?;
            if start + count > snapshot.stripes.len() {
                return Err(BridgeError::CapacityExceeded(
                    "スナップショットの容量を超える読み込みです".to_string(),
                ));
            }
            snapshot.stripes[start..start + count].to_vec()
        };

        let mut out = Vec::with_capacity(len as usize);
        for phys_stripe in physical_stripes {
            out.extend_from_slice(&self.vdev.read_stripe(phys_stripe)?);
        }
        Ok(out)
    }

    /// スナップショットから書き込み可能なクローン(新しいデータセット)を作成する。
    ///
    /// スナップショットと同様、実データは一切コピーしない。作成直後は
    /// クローンとスナップショットは全く同じ物理ストライプを共有しており、
    /// クローンへの書き込みはCoW経由で新しいストライプへ分岐するため、
    /// 元のスナップショット・データセットには一切影響しない。
    pub fn create_clone(
        &mut self,
        dataset_name: &str,
        snapshot_name: &str,
        new_dataset_name: &str,
    ) -> BridgeResult<()> {
        if self.datasets.contains_key(new_dataset_name) {
            return Err(BridgeError::AlreadyExists(format!("データセット'{new_dataset_name}'")));
        }
        let key = Self::snapshot_key(dataset_name, snapshot_name);
        let stripes = self
            .snapshots
            .get(&key)
            .ok_or_else(|| snapshot_not_found(&key))?
            .stripes
            .clone();

        for &stripe in &stripes {
            self.retain_stripe(stripe);
        }
        self.datasets.insert(new_dataset_name.to_string(), Dataset { stripes });
        Ok(())
    }

    /// 物理ストライプ番号を直接指定して読み出す(CoWの検証・デバッグ用。
    /// 通常のデータアクセスは`read`/`write`をデータセット経由で使うこと)。
    pub fn read_physical_stripe(&mut self, physical_stripe: u64) -> BridgeResult<Vec<u8>> {
        self.vdev.read_stripe(physical_stripe)
    }

    /// データセットの論理ストライプ番号が指す物理ストライプ番号を返す
    /// (CoWの検証・デバッグ用)。
    pub fn physical_stripe_for(&self, name: &str, logical_stripe: u64) -> BridgeResult<u64> {
        let ds = self.datasets.get(name).ok_or_else(|| not_found(name))?;
        ds.stripes
            .get(logical_stripe as usize)
            .copied()
            .ok_or_else(|| {
                BridgeError::CapacityExceeded(format!(
                    "データセット'{name}'の論理ストライプ{logical_stripe}は未割当です"
                ))
            })
    }

    /// データセットからストライプ境界単位で読み込む。
    pub fn read(&mut self, name: &str, logical_offset: u64, len: u64) -> BridgeResult<Vec<u8>> {
        let chunk_bytes = self.chunk_bytes();
        assert_eq!(
            logical_offset % chunk_bytes,
            0,
            "現状の実装ではストライプ境界への読み込みのみサポート"
        );
        assert_eq!(
            len % chunk_bytes,
            0,
            "読み込みサイズはストライプ境界の倍数である必要があります"
        );

        let start = (logical_offset / chunk_bytes) as usize;
        let count = (len / chunk_bytes) as usize;

        let physical_stripes: Vec<u64> = {
            let ds = self.datasets.get(name).ok_or_else(|| not_found(name))?;
            if start + count > ds.stripes.len() {
                return Err(BridgeError::CapacityExceeded(format!(
                    "データセット'{name}'の割当容量を超える読み込みです"
                )));
            }
            ds.stripes[start..start + count].to_vec()
        };

        let mut out = Vec::with_capacity(len as usize);
        for phys_stripe in physical_stripes {
            out.extend_from_slice(&self.vdev.read_stripe(phys_stripe)?);
        }
        Ok(out)
    }

    /// [`Self::read`]と同じ内容を、ストライプ境界に揃っていない任意の
    /// `offset`/`len`で読み出す。
    ///
    /// 要求範囲を含む最小のストライプ境界範囲を計算して[`Self::read`]で
    /// まとめて読み出し、実際に要求された部分だけを切り出して返す
    /// (境界を跨ぐ場合は複数ストライプにまたがって読み出す)。
    pub fn read_unaligned(&mut self, name: &str, offset: u64, len: u64) -> BridgeResult<Vec<u8>> {
        if len == 0 {
            return Ok(Vec::new());
        }
        let chunk_bytes = self.chunk_bytes();
        let (aligned_offset, aligned_len) = Self::align_range(chunk_bytes, offset, len);

        let buffer = self.read(name, aligned_offset, aligned_len)?;
        let start = (offset - aligned_offset) as usize;
        Ok(buffer[start..start + len as usize].to_vec())
    }

    /// [`Self::write`]と同じ内容を、ストライプ境界に揃っていない任意の
    /// `offset`/`data.len()`で書き込む(read-modify-write)。
    ///
    /// 1. 要求範囲を含む最小のストライプ境界範囲を[`Self::read`]で読み出す
    ///    (境界からはみ出す部分の既存バイトを保持するため)。
    /// 2. 読み出したバッファの該当部分を`data`で上書きする。
    /// 3. バッファ全体を[`Self::write`]でストライプ境界単位で書き戻す
    ///    ([`Self::write`]自体がCoWで実装されているため、この関数全体としても
    ///    「書き込み失敗時は既存データが無傷」というCoW特性を保つ)。
    ///
    /// 対象範囲がデータセットの割当容量([`Self::grow_dataset`]で確保済みの
    /// 範囲)を超える場合は、[`Self::read`]/[`Self::write`]と同様にエラーを
    /// 返す(暗黙の自動拡張は行わない)。
    pub fn write_unaligned(&mut self, name: &str, offset: u64, data: &[u8]) -> BridgeResult<()> {
        if data.is_empty() {
            return Ok(());
        }
        let chunk_bytes = self.chunk_bytes();
        let (aligned_offset, aligned_len) = Self::align_range(chunk_bytes, offset, data.len() as u64);

        let mut buffer = self.read(name, aligned_offset, aligned_len)?;
        let start = (offset - aligned_offset) as usize;
        buffer[start..start + data.len()].copy_from_slice(data);

        self.write(name, aligned_offset, &buffer)
    }

    /// `[offset, offset + len)`を含む最小のストライプ境界範囲
    /// `[aligned_offset, aligned_offset + aligned_len)`を計算する。
    fn align_range(chunk_bytes: u64, offset: u64, len: u64) -> (u64, u64) {
        let aligned_offset = (offset / chunk_bytes) * chunk_bytes;
        let end = offset + len;
        let aligned_end = end.div_ceil(chunk_bytes) * chunk_bytes;
        (aligned_offset, aligned_end - aligned_offset)
    }

    /// プール全体(全物理ストライプ)をスキャンし、チェックサム不一致
    /// (サイレント破損)を検知・自己修復する(ZFSの`zpool scrub`に相当)。
    ///
    /// `scrub`自体は[`crate::vdev::RaidZVdev`]/[`crate::raid10::Raid10Vdev`]の
    /// どちらにも実装済みだったが、`Pool`が`vdev`フィールドを非公開で保持する
    /// ため、`Pool`しか持たない呼び出し側(`mount.rs`等)からは一切呼び出せない
    /// という抜けがあった。`Vdev`トレイトに`scrub`を追加したことで、`Pool`は
    /// 内部のvdev実装(RAID-Z系かRAID10か)を意識せずにこのメソッド1つで
    /// 委譲できる。
    pub fn scrub(&mut self) -> BridgeResult<crate::vdev::ScrubReport> {
        self.vdev.scrub(self.total_stripes)
    }

    /// 内部の`Vdev`実装への可変参照を返す。
    ///
    /// `resilver`(故障ディスクの交換・再構築)は[`crate::vdev::RaidZVdev`]と
    /// [`crate::raid10::Raid10Vdev`]とでシグネチャが異なる
    /// (前者は対象ディスク1個のインデックス、後者はミラーグループと
    /// グループ内インデックスの2階層)ため、`scrub`のように`Vdev`トレイトへ
    /// 単純には統一できていない。ディスク交換のような低頻度・vdev固有の
    /// 操作を行う呼び出し側は、このメソッドで内部の具体的な型
    /// (`V::resilver`等)へ直接アクセスすること。
    pub fn vdev_mut(&mut self) -> &mut V {
        &mut self.vdev
    }
}
