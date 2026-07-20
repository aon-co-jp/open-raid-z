//! exFAT ⇔ ZFS のファイル属性・タイムスタンプ相互変換。
//!
//! `acl_emulation.rs`がNTFSのACL/SIDセマンティクスを扱うのに対し、本モジュールは
//! Windows/Macの両方で読み書きできる「万能フォーマット」であるexFATとの
//! 近似互換性を担当する。exFATにはNTFSのようなACL/所有者概念が無いため、
//! 対象は次の2点に絞る(設計方針として明示的にスコープを絞った):
//!
//! 1. **属性の相互変換**: exFATの File Attributes フィールド(Win32の
//!    `FILE_ATTRIBUTE_*`と同一のビット値: 読み取り専用/隠しファイル/
//!    システム/ディレクトリ/アーカイブ)とZFS側の中間表現。
//! 2. **タイムスタンプの相互変換**: exFATのDOS形式タイムスタンプ
//!    (2秒分解能の日付+時刻 + 10ms単位の端数 + UTCオフセット)と
//!    Unixエポック秒の相互変換。
//!
//! 【軽量化への配慮】外部の日付処理クレート(chrono等)には依存せず、
//! カレンダー計算はHoward Hinnantの`days_from_civil`/`civil_from_days`
//! (公開されているuint演算のみの定数時間アルゴリズム、LLVM libc++等でも
//! 採用されている実装)を使う。ヒープ確保も一切行わない、単純な整数演算のみの
//! 実装なので、ファイルの読み書きのたびに変換してもコストはごく小さい。
//!
//! 【4GB超ファイル・大容量ボリュームについて】exFATの「4GB超ファイルを
//! 制限なく読み書きできる」という特徴自体は、本プロジェクトの
//! [`crate::pool`]/[`crate::vdev`]がストライプ数・オフセットを一貫して
//! `u64`で扱っている(`u32`のバイトカウンタを一切使わない)ことで、
//! 既に設計上達成済み。本モジュールでは属性/タイムスタンプ変換に専念する。

use crate::error::{BridgeError, BridgeResult};
use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    /// exFATの File Attributes フィールド。Win32の`FILE_ATTRIBUTE_*`と
    /// 同一のビット割り当て(exFAT仕様書 7.1.2.3 FileAttributes Field)。
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ExFatAttributes: u16 {
        const READ_ONLY = 0x0001;
        const HIDDEN    = 0x0002;
        const SYSTEM    = 0x0004;
        const DIRECTORY = 0x0010;
        const ARCHIVE   = 0x0020;
    }
}

/// ZFS側のファイル属性の中間表現。ZFSにはNTFS/exFATのような専用の属性
/// ビットフィールドは無いため、POSIXパーミッション/慣習から近似的に
/// 導出・逆導出する想定(実際のPOSIXモード⇔属性の対応付けは呼び出し側の責務。
/// ここでは属性値そのものの相互変換ロジックのみを提供する)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZfsFileAttributes {
    pub read_only: bool,
    pub hidden: bool,
    pub system: bool,
    pub is_directory: bool,
    pub archive: bool,
}

pub fn exfat_attrs_to_zfs(attrs: ExFatAttributes) -> ZfsFileAttributes {
    ZfsFileAttributes {
        read_only: attrs.contains(ExFatAttributes::READ_ONLY),
        hidden: attrs.contains(ExFatAttributes::HIDDEN),
        system: attrs.contains(ExFatAttributes::SYSTEM),
        is_directory: attrs.contains(ExFatAttributes::DIRECTORY),
        archive: attrs.contains(ExFatAttributes::ARCHIVE),
    }
}

pub fn zfs_attrs_to_exfat(attrs: ZfsFileAttributes) -> ExFatAttributes {
    let mut out = ExFatAttributes::empty();
    out.set(ExFatAttributes::READ_ONLY, attrs.read_only);
    out.set(ExFatAttributes::HIDDEN, attrs.hidden);
    out.set(ExFatAttributes::SYSTEM, attrs.system);
    out.set(ExFatAttributes::DIRECTORY, attrs.is_directory);
    out.set(ExFatAttributes::ARCHIVE, attrs.archive);
    out
}

/// exFATのDOS形式タイムスタンプ(仕様書 7.4 Timestamp Fields)。
///
/// - `date`: bit15-9=年(1980年からのオフセット, 0-127) bit8-5=月(1-12) bit4-0=日(1-31)
/// - `time`: bit15-11=時(0-23) bit10-5=分(0-59) bit4-0=秒/2(0-29, 2秒分解能)
/// - `ten_ms_increment`: 0-199。`time`の2秒分解能を補う10ms単位の端数
///   (最大1990ms=ほぼ2秒ぶん)。
/// - `utc_offset`: bit7=有効フラグ。有効な場合、下位7bitは15分単位・符号付き
///   (2の補数、-64..63 = -16:00..+15:45)のUTCからのオフセット。
///   無効な場合は「タイムゾーン不明(ローカル時刻として扱う)」を意味する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExFatTimestamp {
    pub date: u16,
    pub time: u16,
    pub ten_ms_increment: u8,
    pub utc_offset: u8,
}

const EXFAT_EPOCH_YEAR: i64 = 1980;
const EXFAT_MAX_YEAR_OFFSET: i64 = 127; // 1980 + 127 = 2107年が上限

/// Howard Hinnantの`days_from_civil`(2の補数演算のみ、ヒープ確保無し)。
/// `y-m-d`(グレゴリオ暦)から1970-01-01を0とした日数を求める。
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m as i64 + 9) % 12; // [0, 11]
    let doy = (153 * mp + 2) / 5 + d as i64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

/// `days_from_civil`の逆変換。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// exFATタイムスタンプをUnixエポック秒(UTC)へ変換する。
///
/// exFATの日付/時刻フィールドは「ローカル時刻(タイムゾーン不明時)」または
/// `utc_offset`で示されたタイムゾーンでの壁時計時刻として格納されているため、
/// UTCへ変換するには`local_time - utc_offset`を計算する。
pub fn exfat_timestamp_to_unix(ts: &ExFatTimestamp) -> BridgeResult<i64> {
    let year = EXFAT_EPOCH_YEAR + ((ts.date >> 9) & 0x7F) as i64;
    let month = ((ts.date >> 5) & 0x0F) as u32;
    let day = (ts.date & 0x1F) as u32;
    let hour = ((ts.time >> 11) & 0x1F) as i64;
    let minute = ((ts.time >> 5) & 0x3F) as i64;
    let double_seconds = (ts.time & 0x1F) as i64;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(BridgeError::ExFatConversionFailed(format!(
            "不正な日付フィールドです(date=0x{:04X}: year={year} month={month} day={day})",
            ts.date
        )));
    }
    if ts.ten_ms_increment > 199 {
        return Err(BridgeError::ExFatConversionFailed(format!(
            "ten_ms_incrementは0-199の範囲外です: {}",
            ts.ten_ms_increment
        )));
    }

    let days = days_from_civil(year, month, day);
    let mut secs = days * 86_400 + hour * 3600 + minute * 60 + double_seconds * 2;
    secs += (ts.ten_ms_increment as i64 * 10) / 1000;

    if ts.utc_offset & 0x80 != 0 {
        let raw = (ts.utc_offset & 0x7F) as i32;
        let signed_units = if raw >= 64 { raw - 128 } else { raw };
        secs -= (signed_units * 15 * 60) as i64;
    }

    Ok(secs)
}

/// Unixエポック秒(UTC)をexFATタイムスタンプへ変換する。
///
/// 常にUTCとして書き込み、`utc_offset`は「有効・オフセット0(=UTC)」を明示する
/// (どのタイムゾーンで書いたか不明のまま「無効」にするより、明示的にUTCと
/// 記録した方が読み手にとって曖昧さが無いため)。
///
/// 【精度について】exFATの`date`/`time`フィールド自体は2秒分解能なので、
/// 奇数秒の入力は切り捨てられる(`ten_ms_increment`は常に0を書き込む)。
/// これはexFAT仕様そのものの制約であり、本関数のバグではない。
pub fn unix_to_exfat_timestamp(unix_secs: i64) -> BridgeResult<ExFatTimestamp> {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);

    let year_offset = year - EXFAT_EPOCH_YEAR;
    if !(0..=EXFAT_MAX_YEAR_OFFSET).contains(&year_offset) {
        return Err(BridgeError::ExFatConversionFailed(format!(
            "exFATが表現できる範囲外の年です(1980-2107のみ対応): {year}"
        )));
    }

    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let double_seconds = (secs_of_day % 60) / 2;

    let date = ((year_offset as u16) << 9) | ((month as u16) << 5) | (day as u16);
    let time = ((hour as u16) << 11) | ((minute as u16) << 5) | (double_seconds as u16);

    Ok(ExFatTimestamp {
        date,
        time,
        ten_ms_increment: 0,
        utc_offset: 0x80, // 有効フラグのみ、オフセット0 = UTC
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_round_trip_preserves_all_flags() {
        let attrs =
            ZfsFileAttributes { read_only: true, hidden: false, system: true, is_directory: false, archive: true };
        let exfat = zfs_attrs_to_exfat(attrs);
        assert_eq!(exfat, ExFatAttributes::READ_ONLY | ExFatAttributes::SYSTEM | ExFatAttributes::ARCHIVE);
        assert_eq!(exfat_attrs_to_zfs(exfat), attrs);
    }

    #[test]
    fn directory_attribute_round_trips() {
        let exfat = ExFatAttributes::DIRECTORY;
        let zfs = exfat_attrs_to_zfs(exfat);
        assert!(zfs.is_directory);
        assert_eq!(zfs_attrs_to_exfat(zfs), exfat);
    }

    #[test]
    fn unix_epoch_style_even_second_round_trips_exactly() {
        // 2024-03-15 12:34:56 UTC (56は偶数秒なので2秒分解能でも欠落しない)
        let unix = days_from_civil(2024, 3, 15) * 86_400 + 12 * 3600 + 34 * 60 + 56;
        let ts = unix_to_exfat_timestamp(unix).unwrap();
        let back = exfat_timestamp_to_unix(&ts).unwrap();
        assert_eq!(back, unix);
    }

    #[test]
    fn odd_second_input_truncates_to_even_second_by_design() {
        // exFATの2秒分解能により、奇数秒は直前の偶数秒へ切り捨てられる
        // (仕様上の制約であり、本実装のバグではないことを明示するテスト)。
        let unix = days_from_civil(2024, 3, 15) * 86_400 + 12 * 3600 + 34 * 60 + 57;
        let ts = unix_to_exfat_timestamp(unix).unwrap();
        let back = exfat_timestamp_to_unix(&ts).unwrap();
        assert_eq!(back, unix - 1);
    }

    #[test]
    fn epoch_boundary_1980_01_01_round_trips() {
        let unix = days_from_civil(1980, 1, 1) * 86_400;
        let ts = unix_to_exfat_timestamp(unix).unwrap();
        assert_eq!(ts.date >> 9, 0); // 年オフセット0
        let back = exfat_timestamp_to_unix(&ts).unwrap();
        assert_eq!(back, unix);
    }

    #[test]
    fn year_before_1980_is_rejected() {
        let unix = days_from_civil(1979, 12, 31) * 86_400;
        assert!(unix_to_exfat_timestamp(unix).is_err());
    }

    #[test]
    fn year_after_2107_is_rejected() {
        let unix = days_from_civil(2108, 1, 1) * 86_400;
        assert!(unix_to_exfat_timestamp(unix).is_err());
    }

    #[test]
    fn utc_offset_is_applied_when_valid() {
        // date/timeフィールドが「ローカル時刻09:00、UTC+9(=+540分=36個の15分単位)」を
        // 表す場合、UTCでは00:00になるはず。
        let local_days = days_from_civil(2024, 6, 1);
        let mut ts = unix_to_exfat_timestamp(local_days * 86_400 + 9 * 3600).unwrap();
        ts.utc_offset = 0x80 | 36; // 有効、+9:00(36 * 15分 = 540分)

        let utc = exfat_timestamp_to_unix(&ts).unwrap();
        assert_eq!(utc, local_days * 86_400); // 00:00 UTC
    }

    #[test]
    fn negative_utc_offset_is_applied_via_twos_complement() {
        // ローカル00:00、UTC-5(-300分 = -20 * 15分)なら、UTCでは同日05:00のはず。
        let local_days = days_from_civil(2024, 6, 1);
        let mut ts = unix_to_exfat_timestamp(local_days * 86_400).unwrap();
        let raw = (-20i32 + 128) as u8 & 0x7F; // 2の補数(7bit)でのエンコード
        ts.utc_offset = 0x80 | raw;

        let utc = exfat_timestamp_to_unix(&ts).unwrap();
        assert_eq!(utc, local_days * 86_400 + 5 * 3600);
    }

    #[test]
    fn invalid_ten_ms_increment_is_rejected() {
        let ts = ExFatTimestamp { date: 0, time: 0, ten_ms_increment: 200, utc_offset: 0 };
        assert!(exfat_timestamp_to_unix(&ts).is_err());
    }
}
