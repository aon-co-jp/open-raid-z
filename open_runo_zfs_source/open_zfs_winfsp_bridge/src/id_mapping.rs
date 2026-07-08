//! NTFS(Windows SID) ⇔ ZFS(POSIX UID/GID) の相互マッピングテーブル。
//!
//! 2種類のマッピング戦略を層として持つ(Sambaのwinbind `idmap.conf`と同じ考え方):
//!
//! 1. **明示的マッピング**([`IdMappingTable::map_user`]/[`map_group`])
//!    個別に登録したUID/GID⇔SIDの対応。最優先で参照される。
//! 2. **アルゴリズム的マッピング**([`AlgorithmicIdMap`])
//!    ドメイン(ローカルSAM or AD)のSIDプレフィックス(例: `S-1-5-21-x-y-z`)ごとに
//!    「RID(SIDの最後のサブオーソリティ)をUID/GIDのベースオフセットへ足すだけ」で
//!    決定論的に導出する(`idmap_rid`/`idmap_autorid`相当)。個別登録が不要なため、
//!    AD上の全アカウントを事前に列挙・登録しなくても、ドメインの範囲を1つ
//!    設定するだけでUID/GIDが決まる。
//!
//! 【実運用での構築方法(設計メモ)】
//! - ローカルSAM: `NetUserEnum`/`NetLocalGroupEnum` + `LookupAccountSidW`で
//!   コンピュータのSID(`S-1-5-21-...`)を得て、[`AlgorithmicIdMap`]を1つ登録する。
//! - Active Directory: ドメインのSID(`(Get-ADDomain).DomainSID`相当)ごとに
//!   [`AlgorithmicIdMap`]を1つ登録する(マルチドメイン環境では複数登録可能)。
//! - 特定アカウントだけ固定UIDにしたい場合(例: root相当の管理者)は
//!   [`IdMappingTable::map_user`]で明示的に上書きする。
//!
//! いずれの構築処理も実際のWindows API呼び出し(NetUserEnum等)が必要で
//! 管理者権限・ドメイン参加が前提となるため、本モジュールでは行わない。
//! ここではテーブル自体の構造と、SID文字列のパース・参照ロジックのみを提供する。

use std::collections::HashMap;

/// SID文字列(`S-1-5-21-111-222-333-1000`の形式)から末尾のRID(最後の
/// サブオーソリティ値)を取り出す。パース失敗(SID形式でない、要素が無い等)なら
/// `None`。
pub fn rid_from_sid(sid: &str) -> Option<u32> {
    let rest = sid.strip_prefix("S-1-")?;
    let last = rest.rsplit('-').next()?;
    last.parse::<u32>().ok()
}

/// SID文字列からRIDを除いた「ドメイン部分」(`S-1-5-21-111-222-333`)を取り出す。
pub fn domain_prefix_of_sid(sid: &str) -> Option<&str> {
    let idx = sid.rfind('-')?;
    if idx == 0 {
        return None;
    }
    Some(&sid[..idx])
}

/// 1ドメイン(ローカルSAM または 1つのADドメイン)分の、RIDベースの決定論的
/// UID/GIDマッピング範囲。
///
/// `uid = uid_base + rid`(`rid < range`の場合のみ有効)というだけの単純な規則。
/// Sambaの`idmap config <DOMAIN> : backend = rid`と同じ考え方で、
/// 個別のアカウント登録なしにドメイン全体のUID/GIDを決定できる。
#[derive(Debug, Clone)]
pub struct AlgorithmicIdMap {
    /// このマッピングが対象とするドメインのSIDプレフィックス(RIDを含まない)。
    pub domain_sid_prefix: String,
    pub uid_base: u32,
    pub gid_base: u32,
    /// このドメインで有効なRIDの範囲(`0..range`)。範囲外のRIDは対象外として扱う。
    pub range: u32,
}

impl AlgorithmicIdMap {
    pub fn new(domain_sid_prefix: impl Into<String>, uid_base: u32, gid_base: u32, range: u32) -> Self {
        Self {
            domain_sid_prefix: domain_sid_prefix.into(),
            uid_base,
            gid_base,
            range,
        }
    }

    fn rid_in_range(&self, sid: &str) -> Option<u32> {
        let prefix = domain_prefix_of_sid(sid)?;
        if prefix != self.domain_sid_prefix {
            return None;
        }
        let rid = rid_from_sid(sid)?;
        if rid < self.range {
            Some(rid)
        } else {
            None
        }
    }

    fn uid_for_sid(&self, sid: &str) -> Option<u32> {
        self.rid_in_range(sid).map(|rid| self.uid_base + rid)
    }

    fn gid_for_sid(&self, sid: &str) -> Option<u32> {
        self.rid_in_range(sid).map(|rid| self.gid_base + rid)
    }

    fn sid_for_uid(&self, uid: u32) -> Option<String> {
        let rid = uid.checked_sub(self.uid_base)?;
        if rid < self.range {
            Some(format!("{}-{}", self.domain_sid_prefix, rid))
        } else {
            None
        }
    }

    fn sid_for_gid(&self, gid: u32) -> Option<String> {
        let rid = gid.checked_sub(self.gid_base)?;
        if rid < self.range {
            Some(format!("{}-{}", self.domain_sid_prefix, rid))
        } else {
            None
        }
    }
}

/// UID/GID ⇔ SID(文字列表現, 例: "S-1-5-21-...")の双方向マッピング。
#[derive(Debug, Default, Clone)]
pub struct IdMappingTable {
    uid_to_sid: HashMap<u32, String>,
    sid_to_uid: HashMap<String, u32>,
    gid_to_sid: HashMap<u32, String>,
    sid_to_gid: HashMap<String, u32>,
    /// ドメインごとのアルゴリズム的マッピング(明示的登録に無いSIDのフォールバック)。
    algorithmic: Vec<AlgorithmicIdMap>,
}

impl IdMappingTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// UID <-> SID の対応を登録する(既存の対応があれば上書き)。
    pub fn map_user(&mut self, uid: u32, sid: impl Into<String>) {
        let sid = sid.into();
        if let Some(old_sid) = self.uid_to_sid.remove(&uid) {
            self.sid_to_uid.remove(&old_sid);
        }
        self.sid_to_uid.insert(sid.clone(), uid);
        self.uid_to_sid.insert(uid, sid);
    }

    /// GID <-> SID の対応を登録する(既存の対応があれば上書き)。
    pub fn map_group(&mut self, gid: u32, sid: impl Into<String>) {
        let sid = sid.into();
        if let Some(old_sid) = self.gid_to_sid.remove(&gid) {
            self.sid_to_gid.remove(&old_sid);
        }
        self.sid_to_gid.insert(sid.clone(), gid);
        self.gid_to_sid.insert(gid, sid);
    }

    /// ドメイン(ローカルSAM/ADドメイン)単位のアルゴリズム的マッピング範囲を追加する。
    /// 明示的マッピングで見つからないSID/UID/GIDに対するフォールバックとして使われる。
    pub fn add_algorithmic_domain(&mut self, domain: AlgorithmicIdMap) {
        self.algorithmic.push(domain);
    }

    pub fn sid_for_uid(&self, uid: u32) -> Option<String> {
        if let Some(sid) = self.uid_to_sid.get(&uid) {
            return Some(sid.clone());
        }
        self.algorithmic.iter().find_map(|d| d.sid_for_uid(uid))
    }

    pub fn sid_for_gid(&self, gid: u32) -> Option<String> {
        if let Some(sid) = self.gid_to_sid.get(&gid) {
            return Some(sid.clone());
        }
        self.algorithmic.iter().find_map(|d| d.sid_for_gid(gid))
    }

    pub fn uid_for_sid(&self, sid: &str) -> Option<u32> {
        if let Some(&uid) = self.sid_to_uid.get(sid) {
            return Some(uid);
        }
        self.algorithmic.iter().find_map(|d| d.uid_for_sid(sid))
    }

    pub fn gid_for_sid(&self, sid: &str) -> Option<u32> {
        if let Some(&gid) = self.sid_to_gid.get(sid) {
            return Some(gid);
        }
        self.algorithmic.iter().find_map(|d| d.gid_for_sid(sid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_mapping_is_bidirectional() {
        let mut table = IdMappingTable::new();
        table.map_user(1000, "S-1-5-21-111-222-333-1000");

        assert_eq!(table.sid_for_uid(1000).as_deref(), Some("S-1-5-21-111-222-333-1000"));
        assert_eq!(table.uid_for_sid("S-1-5-21-111-222-333-1000"), Some(1000));
        assert_eq!(table.sid_for_uid(9999), None);
    }

    #[test]
    fn group_mapping_is_bidirectional() {
        let mut table = IdMappingTable::new();
        table.map_group(2000, "S-1-5-21-111-222-333-2000");

        assert_eq!(table.sid_for_gid(2000).as_deref(), Some("S-1-5-21-111-222-333-2000"));
        assert_eq!(table.gid_for_sid("S-1-5-21-111-222-333-2000"), Some(2000));
    }

    #[test]
    fn overwriting_a_mapping_replaces_old_entry() {
        let mut table = IdMappingTable::new();
        table.map_user(1000, "S-1-5-21-OLD");
        table.map_user(1000, "S-1-5-21-NEW");

        assert_eq!(table.sid_for_uid(1000).as_deref(), Some("S-1-5-21-NEW"));
        assert_eq!(table.uid_for_sid("S-1-5-21-OLD"), None);
        assert_eq!(table.uid_for_sid("S-1-5-21-NEW"), Some(1000));
    }

    #[test]
    fn rid_from_sid_extracts_last_subauthority() {
        assert_eq!(rid_from_sid("S-1-5-21-111-222-333-1000"), Some(1000));
        assert_eq!(rid_from_sid("S-1-1-0"), Some(0));
        assert_eq!(rid_from_sid("not-a-sid"), None);
        assert_eq!(rid_from_sid("S-1-5-21-111-222-333-notanumber"), None);
    }

    #[test]
    fn domain_prefix_of_sid_strips_only_the_rid() {
        assert_eq!(
            domain_prefix_of_sid("S-1-5-21-111-222-333-1000"),
            Some("S-1-5-21-111-222-333")
        );
        assert_eq!(domain_prefix_of_sid("nodashes"), None);
    }

    #[test]
    fn algorithmic_domain_derives_uid_and_gid_from_rid_without_explicit_registration() {
        let mut table = IdMappingTable::new();
        table.add_algorithmic_domain(AlgorithmicIdMap::new(
            "S-1-5-21-111-222-333",
            10_000,
            20_000,
            1_000_000,
        ));

        // 個別登録なしでも、ドメイン内のRIDから一意にUID/GIDが決まる。
        assert_eq!(table.uid_for_sid("S-1-5-21-111-222-333-1001"), Some(11_001));
        assert_eq!(table.gid_for_sid("S-1-5-21-111-222-333-1001"), Some(21_001));

        // 逆方向(UID/GID -> SID)も一意に復元できる。
        assert_eq!(
            table.sid_for_uid(11_001).as_deref(),
            Some("S-1-5-21-111-222-333-1001")
        );
        assert_eq!(
            table.sid_for_gid(21_001).as_deref(),
            Some("S-1-5-21-111-222-333-1001")
        );
    }

    #[test]
    fn algorithmic_domain_ignores_sids_from_other_domains() {
        let mut table = IdMappingTable::new();
        table.add_algorithmic_domain(AlgorithmicIdMap::new(
            "S-1-5-21-111-222-333",
            10_000,
            20_000,
            1_000_000,
        ));

        assert_eq!(table.uid_for_sid("S-1-5-21-999-888-777-1001"), None);
    }

    #[test]
    fn explicit_mapping_takes_priority_over_algorithmic_domain() {
        let mut table = IdMappingTable::new();
        table.add_algorithmic_domain(AlgorithmicIdMap::new(
            "S-1-5-21-111-222-333",
            10_000,
            20_000,
            1_000_000,
        ));
        // このユーザだけ、アルゴリズム計算結果(10_500)とは異なるUIDへ固定で上書きする。
        table.map_user(1, "S-1-5-21-111-222-333-500");

        assert_eq!(table.uid_for_sid("S-1-5-21-111-222-333-500"), Some(1));
        assert_eq!(table.sid_for_uid(1).as_deref(), Some("S-1-5-21-111-222-333-500"));
        // アルゴリズム経由の別UIDは引き続き有効。
        assert_eq!(table.uid_for_sid("S-1-5-21-111-222-333-501"), Some(10_501));
    }
}
