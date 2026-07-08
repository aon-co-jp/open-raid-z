//! ZFSはNFSv4 ACLモデルを採用しており、Windows(NTFS)のACL/SIDモデルとは
//! 構造が異なります。ここでは両者を相互変換するための中間表現を定義します。
//!
//! 対応関係の要点:
//! - ZFS: ACE(Allow/Deny) + Flags(File/Dir継承) + Permissions bitmask
//! - NTFS: ACCESS_ALLOWED_ACE/ACCESS_DENIED_ACE + SID + ACCESS_MASK
//!
//! 完全な意味論の一致は不可能(例: ZFSのextended attributesとNTFSの
//! 代替データストリームは概念上似ているが実装は異なる)なため、
//! 本層は「実用上ほぼ問題にならない範囲での近似変換」を目的とします。
//!
//! UID/GID <-> SID の実マッピングは[`crate::id_mapping::IdMappingTable`]に委譲する
//! (Active Directory/ローカルSAM等からどう構築するかは呼び出し側の責務)。

use crate::error::{BridgeError, BridgeResult};
use crate::id_mapping::IdMappingTable;
use serde::{Deserialize, Serialize};

const SID_OWNER: &str = "OWNER_SID";
const SID_GROUP: &str = "GROUP_SID";
const SID_EVERYONE: &str = "S-1-1-0";

/// ZFS側のNFSv4 ACEを表す中間表現
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsAce {
    pub who: ZfsPrincipal,
    pub allow: bool,
    pub permissions: ZfsPermissions,
    pub inherit_to_files: bool,
    pub inherit_to_dirs: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZfsPrincipal {
    Owner,
    Group,
    Everyone,
    User(u32),  // UID
    Group_(u32), // GID
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ZfsPermissions: u32 {
        const READ_DATA    = 0b0000_0001;
        const WRITE_DATA   = 0b0000_0010;
        const APPEND_DATA  = 0b0000_0100;
        const EXECUTE      = 0b0000_1000;
        const DELETE       = 0b0001_0000;
        const READ_ACL     = 0b0010_0000;
        const WRITE_ACL    = 0b0100_0000;
        const READ_ATTR    = 0b1000_0000;
    }
}

/// NTFS側のACEを表す中間表現(windows-rs の ACCESS_ALLOWED_ACE 相当に後で変換)
#[derive(Debug, Clone)]
pub struct NtfsAce {
    pub sid_placeholder: String, // 実装時は windows::Win32::Security::SID に置換
    pub allow: bool,
    pub access_mask: u32,
    pub inherit_to_files: bool,
    pub inherit_to_dirs: bool,
}

/// ZFS ACE -> NTFS ACE への変換(近似)
pub fn zfs_ace_to_ntfs(ace: &ZfsAce, mapping: &IdMappingTable) -> BridgeResult<NtfsAce> {
    let sid_placeholder = match &ace.who {
        ZfsPrincipal::Owner => SID_OWNER.to_string(),
        ZfsPrincipal::Group => SID_GROUP.to_string(),
        ZfsPrincipal::Everyone => SID_EVERYONE.to_string(),
        ZfsPrincipal::User(uid) => mapping
            .sid_for_uid(*uid)
            .ok_or_else(|| {
                BridgeError::AclTranslationFailed(format!("UID {uid} に対応するSIDが未登録です"))
            })?
            .to_string(),
        ZfsPrincipal::Group_(gid) => mapping
            .sid_for_gid(*gid)
            .ok_or_else(|| {
                BridgeError::AclTranslationFailed(format!("GID {gid} に対応するSIDが未登録です"))
            })?
            .to_string(),
    };

    let mut access_mask = 0u32;
    if ace.permissions.contains(ZfsPermissions::READ_DATA) {
        access_mask |= 0x0001; // FILE_READ_DATA
    }
    if ace.permissions.contains(ZfsPermissions::WRITE_DATA) {
        access_mask |= 0x0002; // FILE_WRITE_DATA
    }
    if ace.permissions.contains(ZfsPermissions::APPEND_DATA) {
        access_mask |= 0x0004; // FILE_APPEND_DATA
    }
    if ace.permissions.contains(ZfsPermissions::EXECUTE) {
        access_mask |= 0x0020; // FILE_EXECUTE
    }
    if ace.permissions.contains(ZfsPermissions::DELETE) {
        access_mask |= 0x0001_0000; // DELETE
    }
    if ace.permissions.contains(ZfsPermissions::READ_ACL) {
        access_mask |= 0x0002_0000; // READ_CONTROL
    }
    if ace.permissions.contains(ZfsPermissions::WRITE_ACL) {
        access_mask |= 0x0004_0000; // WRITE_DAC
    }

    Ok(NtfsAce {
        sid_placeholder,
        allow: ace.allow,
        access_mask,
        inherit_to_files: ace.inherit_to_files,
        inherit_to_dirs: ace.inherit_to_dirs,
    })
}

/// NTFS ACE -> ZFS ACE への逆変換(近似)。
///
/// 【桁落ちに関する注意】READ_ATTR(FILE_READ_ATTRIBUTES)はNTFS ACCESS_MASK側に
/// 対応する単一ビットの往復対象を用意していないため、`zfs_ace_to_ntfs`で作られた
/// `access_mask`から復元することはできない。往復変換で完全に元のビットへ
/// 戻したい場合は、呼び出し側でREAD_ATTRを別途保持する必要がある。
pub fn ntfs_ace_to_zfs(ace: &NtfsAce, mapping: &IdMappingTable) -> BridgeResult<ZfsAce> {
    let who = match ace.sid_placeholder.as_str() {
        SID_OWNER => ZfsPrincipal::Owner,
        SID_GROUP => ZfsPrincipal::Group,
        SID_EVERYONE => ZfsPrincipal::Everyone,
        sid => {
            if let Some(uid) = mapping.uid_for_sid(sid) {
                ZfsPrincipal::User(uid)
            } else if let Some(gid) = mapping.gid_for_sid(sid) {
                ZfsPrincipal::Group_(gid)
            } else {
                return Err(BridgeError::AclTranslationFailed(format!(
                    "SID {sid} に対応するUID/GIDが未登録です"
                )));
            }
        }
    };

    let mut permissions = ZfsPermissions::empty();
    if ace.access_mask & 0x0001 != 0 {
        permissions |= ZfsPermissions::READ_DATA;
    }
    if ace.access_mask & 0x0002 != 0 {
        permissions |= ZfsPermissions::WRITE_DATA;
    }
    if ace.access_mask & 0x0004 != 0 {
        permissions |= ZfsPermissions::APPEND_DATA;
    }
    if ace.access_mask & 0x0020 != 0 {
        permissions |= ZfsPermissions::EXECUTE;
    }
    if ace.access_mask & 0x0001_0000 != 0 {
        permissions |= ZfsPermissions::DELETE;
    }
    if ace.access_mask & 0x0002_0000 != 0 {
        permissions |= ZfsPermissions::READ_ACL;
    }
    if ace.access_mask & 0x0004_0000 != 0 {
        permissions |= ZfsPermissions::WRITE_ACL;
    }

    Ok(ZfsAce {
        who,
        allow: ace.allow,
        permissions,
        inherit_to_files: ace.inherit_to_files,
        inherit_to_dirs: ace.inherit_to_dirs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_mapping() -> IdMappingTable {
        let mut mapping = IdMappingTable::new();
        mapping.map_user(1000, "S-1-5-21-111-222-333-1000");
        mapping.map_group(2000, "S-1-5-21-111-222-333-2000");
        mapping
    }

    #[test]
    fn zfs_to_ntfs_maps_known_uid_to_sid() {
        let mapping = sample_mapping();
        let ace = ZfsAce {
            who: ZfsPrincipal::User(1000),
            allow: true,
            permissions: ZfsPermissions::READ_DATA | ZfsPermissions::WRITE_DATA,
            inherit_to_files: true,
            inherit_to_dirs: false,
        };

        let ntfs = zfs_ace_to_ntfs(&ace, &mapping).unwrap();
        assert_eq!(ntfs.sid_placeholder, "S-1-5-21-111-222-333-1000");
        assert_eq!(ntfs.access_mask, 0x0001 | 0x0002);
    }

    #[test]
    fn zfs_to_ntfs_fails_for_unmapped_uid() {
        let mapping = sample_mapping();
        let ace = ZfsAce {
            who: ZfsPrincipal::User(9999),
            allow: true,
            permissions: ZfsPermissions::READ_DATA,
            inherit_to_files: false,
            inherit_to_dirs: false,
        };

        assert!(matches!(
            zfs_ace_to_ntfs(&ace, &mapping),
            Err(BridgeError::AclTranslationFailed(_))
        ));
    }

    #[test]
    fn round_trip_user_ace_preserves_identity_and_permissions() {
        let mapping = sample_mapping();
        let original = ZfsAce {
            who: ZfsPrincipal::User(1000),
            allow: true,
            permissions: ZfsPermissions::READ_DATA
                | ZfsPermissions::WRITE_DATA
                | ZfsPermissions::EXECUTE,
            inherit_to_files: true,
            inherit_to_dirs: true,
        };

        let ntfs = zfs_ace_to_ntfs(&original, &mapping).unwrap();
        let round_tripped = ntfs_ace_to_zfs(&ntfs, &mapping).unwrap();

        assert_eq!(round_tripped.who, original.who);
        assert_eq!(round_tripped.permissions, original.permissions);
        assert_eq!(round_tripped.allow, original.allow);
    }

    #[test]
    fn round_trip_special_principals_preserve_identity() {
        let mapping = sample_mapping();
        for who in [ZfsPrincipal::Owner, ZfsPrincipal::Group, ZfsPrincipal::Everyone] {
            let original = ZfsAce {
                who: who.clone(),
                allow: true,
                permissions: ZfsPermissions::READ_DATA,
                inherit_to_files: false,
                inherit_to_dirs: false,
            };
            let ntfs = zfs_ace_to_ntfs(&original, &mapping).unwrap();
            let round_tripped = ntfs_ace_to_zfs(&ntfs, &mapping).unwrap();
            assert_eq!(round_tripped.who, who);
        }
    }

    #[test]
    fn ntfs_to_zfs_fails_for_unmapped_sid() {
        let mapping = sample_mapping();
        let ntfs = NtfsAce {
            sid_placeholder: "S-1-5-21-999-999-999-9999".to_string(),
            allow: true,
            access_mask: 0x0001,
            inherit_to_files: false,
            inherit_to_dirs: false,
        };

        assert!(matches!(
            ntfs_ace_to_zfs(&ntfs, &mapping),
            Err(BridgeError::AclTranslationFailed(_))
        ));
    }
}
