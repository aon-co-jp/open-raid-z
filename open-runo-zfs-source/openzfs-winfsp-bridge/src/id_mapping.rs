//! NTFS(Windows SID) ⇔ ZFS(POSIX UID/GID) の相互マッピングテーブル。
//!
//! 実運用ではActive Directory / ローカルSAM / `idmap.conf`相当の設定から
//! このテーブルを構築することを想定するが、本モジュールは
//! テーブル自体の構造と参照ロジックのみを提供する(構築元は呼び出し側の責務)。

use std::collections::HashMap;

/// UID/GID ⇔ SID(文字列表現, 例: "S-1-5-21-...")の双方向マッピング。
#[derive(Debug, Default, Clone)]
pub struct IdMappingTable {
    uid_to_sid: HashMap<u32, String>,
    sid_to_uid: HashMap<String, u32>,
    gid_to_sid: HashMap<u32, String>,
    sid_to_gid: HashMap<String, u32>,
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

    pub fn sid_for_uid(&self, uid: u32) -> Option<&str> {
        self.uid_to_sid.get(&uid).map(String::as_str)
    }

    pub fn sid_for_gid(&self, gid: u32) -> Option<&str> {
        self.gid_to_sid.get(&gid).map(String::as_str)
    }

    pub fn uid_for_sid(&self, sid: &str) -> Option<u32> {
        self.sid_to_uid.get(sid).copied()
    }

    pub fn gid_for_sid(&self, sid: &str) -> Option<u32> {
        self.sid_to_gid.get(sid).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_mapping_is_bidirectional() {
        let mut table = IdMappingTable::new();
        table.map_user(1000, "S-1-5-21-111-222-333-1000");

        assert_eq!(table.sid_for_uid(1000), Some("S-1-5-21-111-222-333-1000"));
        assert_eq!(table.uid_for_sid("S-1-5-21-111-222-333-1000"), Some(1000));
        assert_eq!(table.sid_for_uid(9999), None);
    }

    #[test]
    fn group_mapping_is_bidirectional() {
        let mut table = IdMappingTable::new();
        table.map_group(2000, "S-1-5-21-111-222-333-2000");

        assert_eq!(table.sid_for_gid(2000), Some("S-1-5-21-111-222-333-2000"));
        assert_eq!(table.gid_for_sid("S-1-5-21-111-222-333-2000"), Some(2000));
    }

    #[test]
    fn overwriting_a_mapping_replaces_old_entry() {
        let mut table = IdMappingTable::new();
        table.map_user(1000, "S-1-5-21-OLD");
        table.map_user(1000, "S-1-5-21-NEW");

        assert_eq!(table.sid_for_uid(1000), Some("S-1-5-21-NEW"));
        assert_eq!(table.uid_for_sid("S-1-5-21-OLD"), None);
        assert_eq!(table.uid_for_sid("S-1-5-21-NEW"), Some(1000));
    }
}
