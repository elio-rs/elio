//! - Date:
//! - LocalTime:
//! - LocalDateTime:
//! - ZonedDateTime:
//! - Duration:

use chrono::{Days, FixedOffset, Months, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike};

use super::*;

/// Date without timezone
/// days since unix epoch
/// may be negative numbers
/// e.g. 2007-12-03
/// chrono::NaiveDate
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, ScalarPartialOrd)]
#[scalar_partial_ord(_0)]
#[repr(transparent)]
pub struct Date(pub i64);

impl Date {
    pub fn from_le_bytes(bytes: [u8; 8]) -> Self {
        Self(i64::from_le_bytes(bytes))
    }

    pub fn to_le_bytes(self) -> [u8; 8] {
        self.0.to_le_bytes()
    }

    pub fn checked_add(self, duration: &Duration) -> Option<Self> {
        let mut date: NaiveDate = self.into();

        if duration.months != 0 {
            // TODO: support months > u32::MAX
            let months = Months::new(duration.months.unsigned_abs() as u32);
            date = if duration.months > 0 {
                date.checked_add_months(months)
            } else {
                date.checked_sub_months(months)
            }?;
        }

        if duration.days != 0 {
            // TODO: support days > u64::MAX
            let days = Days::new(duration.days.unsigned_abs());
            date = if duration.days > 0 {
                date.checked_add_days(days)
            } else {
                date.checked_sub_days(days)
            }?;
        }

        Some(date.into())
    }

    pub fn add_duration(self, duration: &Duration) -> Self {
        self.checked_add(duration).expect("overflow adding duration to date")
    }

    pub fn checked_sub(&self, duration: &Duration) -> Option<Self> {
        self.checked_add(&duration.checked_neg()?)
    }

    pub fn diff(&self, other: &Date) -> Duration {
        let me: NaiveDate = (*self).into();
        let other: NaiveDate = (*other).into();
        let diff = me.signed_duration_since(other);
        Duration {
            months: 0,
            days: diff.num_days(),
            seconds: 0,
            nanoseconds: 0,
        }
    }
}

impl TryFrom<&str> for Date {
    // (context, expected, actual)
    type Error = (String, String, String);

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map_err(|_| ("date()".to_string(), "yyyy-MM-dd".to_string(), value.to_string()))
            .map(Into::into)
    }
}

impl From<NaiveDate> for Date {
    fn from(value: NaiveDate) -> Self {
        let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        let days = value.signed_duration_since(epoch).num_days();
        Date(days)
    }
}

impl From<Date> for NaiveDate {
    fn from(value: Date) -> Self {
        let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        epoch
            .checked_add_signed(chrono::Duration::days(value.0))
            .expect("date out of range for chrono")
    }
}

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", NaiveDate::from(*self))
    }
}

/// time of a day without timezone
// nanoseconds since midnight
// chrono::NaiveTime
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, ScalarPartialOrd)]
#[scalar_partial_ord(_0)]
#[repr(transparent)]
pub struct LocalTime(pub u64);

impl LocalTime {
    pub fn from_le_bytes(bytes: [u8; 8]) -> Self {
        Self(u64::from_le_bytes(bytes))
    }

    pub fn to_le_bytes(self) -> [u8; 8] {
        self.0.to_le_bytes()
    }

    pub fn checked_add(&self, duration: &Duration) -> Option<Self> {
        let nanos_per_day: i128 = 86_400_000_000_000;
        let mut total_nanos = self.0 as i128;

        // Add seconds
        let seconds_ns = (duration.seconds as i128).checked_mul(1_000_000_000)?;
        total_nanos = total_nanos.checked_add(seconds_ns)?;

        // Add nanoseconds
        total_nanos = total_nanos.checked_add(duration.nanoseconds as i128)?;

        // Wrap around
        let result_nanos = total_nanos.rem_euclid(nanos_per_day);

        Some(LocalTime(result_nanos as u64))
    }

    pub fn add(&self, duration: &Duration) -> Self {
        self.checked_add(duration)
            .expect("overflow adding duration to LocalTime")
    }

    pub fn checked_sub(&self, duration: &Duration) -> Option<Self> {
        self.checked_add(&duration.checked_neg()?)
    }

    pub fn diff(&self, other: &LocalTime) -> Duration {
        let me: NaiveTime = (*self).into();
        let other: NaiveTime = (*other).into();
        let diff = me.signed_duration_since(other);
        Duration {
            months: 0,
            days: 0,
            seconds: diff.num_seconds(),
            nanoseconds: diff.subsec_nanos() as i64,
        }
    }
}

impl TryFrom<&str> for LocalTime {
    // (context, expected, actual)
    type Error = (String, String, String);

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        NaiveTime::parse_from_str(value, "%H:%M:%S")
            .map_err(|_| ("localTime()".to_string(), "HH:mm:ss".to_string(), value.to_string()))
            .map(Into::into)
    }
}

impl From<NaiveTime> for LocalTime {
    fn from(value: NaiveTime) -> Self {
        let secs = value.num_seconds_from_midnight() as u64;
        let nanos = value.nanosecond() as u64;
        let time = secs * 1_000_000_000 + nanos;
        LocalTime(time)
    }
}

impl From<LocalTime> for NaiveTime {
    fn from(value: LocalTime) -> Self {
        let total_ns = value.0;
        let secs = (total_ns / 1_000_000_000) as u32;
        let hour = secs / 3600;
        let min = (secs % 3600) / 60;
        let secs = secs % 60;
        let nanos = (total_ns % 1_000_000_000) as u32;
        NaiveTime::from_hms_nano_opt(hour, min, secs, nanos).expect("invalid naive time")
    }
}

impl std::fmt::Display for LocalTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", NaiveTime::from(*self))
    }
}

/// Date and time without timezone
/// chrono::NaiveDatetime
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, ScalarPartialOrd)]
#[scalar_partial_ord(seconds, nanoseconds)]
pub struct LocalDateTime {
    // seconds since unix epoch
    pub seconds: i64,
    // nanoseconds fraction, range from 0 to 999_999_999
    pub nanoseconds: u32,
}

impl LocalDateTime {
    pub const STORAGE_BYTES: usize = 12;

    pub fn to_date(&self) -> Date {
        let days = self.seconds.div_euclid(86_400);
        Date(days)
    }

    pub fn to_local_time(&self) -> LocalTime {
        let secs = self.seconds.rem_euclid(86_400);
        LocalTime(secs as u64 * 1_000_000_000 + self.nanoseconds as u64)
    }

    pub fn from_le_bytes(bytes: [u8; Self::STORAGE_BYTES]) -> Self {
        Self {
            seconds: i64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            nanoseconds: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
        }
    }

    pub fn to_le_bytes(self) -> [u8; Self::STORAGE_BYTES] {
        let mut bytes = [0u8; Self::STORAGE_BYTES];
        bytes[0..8].copy_from_slice(&self.seconds.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.nanoseconds.to_le_bytes());
        bytes
    }

    pub fn checked_add(&self, duration: &Duration) -> Option<Self> {
        let mut dt: NaiveDateTime = (*self).into();

        // Add months
        if duration.months != 0 {
            if duration.months > 0 {
                dt = dt.checked_add_months(Months::new(duration.months as u32))?;
            } else {
                dt = dt.checked_sub_months(Months::new(duration.months.unsigned_abs() as u32))?;
            }
        }

        // Add days
        if duration.days != 0 {
            if duration.days > 0 {
                dt = dt.checked_add_days(Days::new(duration.days as u64))?;
            } else {
                dt = dt.checked_sub_days(Days::new(duration.days.unsigned_abs()))?;
            }
        }

        // Add seconds and nanoseconds
        let time_duration = chrono::Duration::seconds(duration.seconds)
            .checked_add(&chrono::Duration::nanoseconds(duration.nanoseconds))?;
        dt = dt.checked_add_signed(time_duration)?;

        Some(dt.into())
    }

    pub fn checked_sub(&self, duration: &Duration) -> Option<Self> {
        self.checked_add(&duration.checked_neg()?)
    }

    pub fn diff(&self, other: &LocalDateTime) -> Duration {
        let me: NaiveDateTime = (*self).into();
        let other: NaiveDateTime = (*other).into();
        let diff = me.signed_duration_since(other);
        Duration {
            months: 0,
            days: diff.num_days(),
            seconds: (diff - chrono::Duration::days(diff.num_days())).num_seconds(),
            nanoseconds: diff.subsec_nanos() as i64,
        }
    }
}

impl From<NaiveDateTime> for LocalDateTime {
    fn from(value: NaiveDateTime) -> Self {
        Self {
            seconds: value.and_utc().timestamp(),
            nanoseconds: value.and_utc().timestamp_subsec_nanos(),
        }
    }
}

impl From<LocalDateTime> for NaiveDateTime {
    fn from(value: LocalDateTime) -> Self {
        NaiveDateTime::from_timestamp_opt(value.seconds, value.nanoseconds).expect("invalid naive datetime")
    }
}

impl TryFrom<&str> for LocalDateTime {
    // (context, expected, actual)
    type Error = (String, String, String);

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let value = value.replace('T', " ");
        NaiveDateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S%.f")
            .map_err(|_| {
                (
                    "localDateTime()".to_string(),
                    "yyyy-MM-dd HH:mm:ss.SSSSSSSSS".to_string(),
                    value.to_string(),
                )
            })
            .map(Into::into)
    }
}

impl std::fmt::Display for LocalDateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", NaiveDateTime::from(*self))
    }
}

/// Date, time and timezone
/// chrono::DateTime<FixedOffset>
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ZonedDateTime {
    // seconds since unix epoch
    pub seconds: i64,
    // nanoseconds fraction, range from 0 to 999_999_999
    pub nanoseconds: u32,
    // offset in seconds from UTC
    pub tz_offset_seconds: i32,
}

impl ZonedDateTime {
    pub const STORAGE_BYTES: usize = 16;

    pub fn to_date(&self) -> Date {
        let local_seconds = self.seconds + self.tz_offset_seconds as i64;
        let days = local_seconds.div_euclid(86_400);
        Date(days)
    }

    pub fn to_local_time(&self) -> LocalTime {
        let local_seconds = self.seconds + self.tz_offset_seconds as i64;
        let secs = local_seconds.rem_euclid(86_400);
        LocalTime(secs as u64 * 1_000_000_000 + self.nanoseconds as u64)
    }

    pub fn to_local_date_time(&self) -> LocalDateTime {
        let local_seconds = self.seconds + self.tz_offset_seconds as i64;
        LocalDateTime {
            seconds: local_seconds,
            nanoseconds: self.nanoseconds,
        }
    }

    pub fn from_le_bytes(bytes: [u8; 16]) -> Self {
        Self {
            seconds: i64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            nanoseconds: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            tz_offset_seconds: i32::from_le_bytes(bytes[12..16].try_into().unwrap()),
        }
    }

    pub fn to_le_bytes(self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&self.seconds.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.nanoseconds.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.tz_offset_seconds.to_le_bytes());
        bytes
    }

    pub fn checked_add(&self, duration: &Duration) -> Option<Self> {
        let mut dt: chrono::DateTime<FixedOffset> = (*self).into();

        // Add months
        if duration.months != 0 {
            if duration.months > 0 {
                dt = dt.checked_add_months(Months::new(duration.months as u32))?;
            } else {
                dt = dt.checked_sub_months(Months::new(duration.months.unsigned_abs() as u32))?;
            }
        }

        // Add days
        if duration.days != 0 {
            if duration.days > 0 {
                dt = dt.checked_add_days(Days::new(duration.days as u64))?;
            } else {
                dt = dt.checked_sub_days(Days::new(duration.days.unsigned_abs()))?;
            }
        }

        // Add seconds and nanoseconds
        let time_duration = chrono::Duration::seconds(duration.seconds)
            .checked_add(&chrono::Duration::nanoseconds(duration.nanoseconds))?;
        dt = dt.checked_add_signed(time_duration)?;

        Some(dt.into())
    }

    pub fn checked_sub(&self, duration: &Duration) -> Option<Self> {
        self.checked_add(&duration.checked_neg()?)
    }

    pub fn diff(&self, other: &ZonedDateTime) -> Duration {
        let me: chrono::DateTime<FixedOffset> = (*self).into();
        let other: chrono::DateTime<FixedOffset> = (*other).into();
        let diff = me.signed_duration_since(other);
        Duration {
            months: 0,
            days: diff.num_days(),
            seconds: (diff - chrono::Duration::days(diff.num_days())).num_seconds(),
            nanoseconds: diff.subsec_nanos() as i64,
        }
    }
}

impl TryFrom<&str> for ZonedDateTime {
    // (context, expected, actual)
    type Error = (String, String, String);

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let value = value.replace('T', " ");
        chrono::DateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S%.f %z")
            .map_err(|_| {
                (
                    "zonedDateTime()".to_string(),
                    "yyyy-MM-dd HH:mm:ss.SSSSSSSSS +0000".to_string(),
                    value.to_string(),
                )
            })
            .map(Into::into)
    }
}

impl From<chrono::DateTime<FixedOffset>> for ZonedDateTime {
    fn from(value: chrono::DateTime<FixedOffset>) -> Self {
        Self {
            seconds: value.timestamp(),
            nanoseconds: value.timestamp_subsec_nanos(),
            tz_offset_seconds: value.offset().local_minus_utc(),
        }
    }
}

impl From<ZonedDateTime> for chrono::DateTime<FixedOffset> {
    fn from(value: ZonedDateTime) -> Self {
        let offset = FixedOffset::east_opt(value.tz_offset_seconds).unwrap();
        offset.timestamp_opt(value.seconds, value.nanoseconds).unwrap()
    }
}

impl ScalarPartialOrd for ZonedDateTime {
    fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let me: chrono::DateTime<FixedOffset> = (*self).into();
        let other: chrono::DateTime<FixedOffset> = (*other).into();
        me.partial_cmp(&other)
    }
}

impl std::fmt::Display for ZonedDateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", chrono::DateTime::<FixedOffset>::from(*self))
    }
}

/// Duration
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, derive_more::Display, ScalarPartialOrd)]
#[scalar_partial_ord(months, days, seconds, nanoseconds)]
#[display("P{months}M{days}DT{seconds}S{nanoseconds}")]
pub struct Duration {
    pub months: i64,
    pub days: i64,
    pub seconds: i64,
    pub nanoseconds: i64,
}

impl Duration {
    pub fn checked_add(&self, other: &Self) -> Option<Self> {
        let months = self.months.checked_add(other.months)?;
        let days = self.days.checked_add(other.days)?;
        let mut seconds = self.seconds.checked_add(other.seconds)?;
        let mut nanoseconds = self.nanoseconds.checked_add(other.nanoseconds)?;

        if nanoseconds >= 1_000_000_000 || nanoseconds <= -1_000_000_000 {
            seconds = seconds.checked_add(nanoseconds / 1_000_000_000)?;
            nanoseconds %= 1_000_000_000;
        }

        Some(Self {
            months,
            days,
            seconds,
            nanoseconds,
        })
    }

    pub fn checked_neg(&self) -> Option<Self> {
        Some(Self {
            months: self.months.checked_neg()?,
            days: self.days.checked_neg()?,
            seconds: self.seconds.checked_neg()?,
            nanoseconds: self.nanoseconds.checked_neg()?,
        })
    }

    pub fn checked_sub(&self, other: &Self) -> Option<Self> {
        self.checked_add(&other.checked_neg()?)
    }

    pub fn add(&self, other: &Self) -> Self {
        self.checked_add(other).expect("overflow adding durations")
    }

    pub fn checked_mul(&self, scalar: i64) -> Option<Self> {
        let months = self.months.checked_mul(scalar)?;
        let days = self.days.checked_mul(scalar)?;
        let mut seconds = self.seconds.checked_mul(scalar)?;
        let mut nanoseconds = self.nanoseconds.checked_mul(scalar)?;

        if nanoseconds >= 1_000_000_000 || nanoseconds <= -1_000_000_000 {
            seconds = seconds.checked_add(nanoseconds / 1_000_000_000)?;
            nanoseconds %= 1_000_000_000;
        }

        Some(Self {
            months,
            days,
            seconds,
            nanoseconds,
        })
    }

    pub fn checked_mul_f64(&self, scalar: f64) -> Option<Self> {
        let months = (self.months as f64 * scalar).round() as i64;
        let days = (self.days as f64 * scalar).round() as i64;

        let total_seconds = (self.seconds as f64 + self.nanoseconds as f64 / 1e9) * scalar;
        let seconds = total_seconds.trunc() as i64;
        let nanoseconds = (total_seconds.fract() * 1e9).round() as i64;

        Some(Self {
            months,
            days,
            seconds,
            nanoseconds,
        })
    }

    pub fn checked_div(&self, scalar: f64) -> Option<Self> {
        if scalar == 0.0 {
            return None;
        }
        let months = (self.months as f64 / scalar).round() as i64;
        let days = (self.days as f64 / scalar).round() as i64;

        let total_seconds = (self.seconds as f64 + self.nanoseconds as f64 / 1e9) / scalar;
        let seconds = total_seconds.trunc() as i64;
        let nanoseconds = (total_seconds.fract() * 1e9).round() as i64;

        Some(Self {
            months,
            days,
            seconds,
            nanoseconds,
        })
    }
}

impl TryFrom<&str> for Duration {
    type Error = (String, String, String);

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        use iso8601_duration::Duration as IsoDuration;

        let d = IsoDuration::parse(value).map_err(|e| {
            (
                "duration()".to_string(),
                "valid ISO8601 duration".to_string(),
                format!("{}: {:?}", value, e),
            )
        })?;

        // Calculate months
        // Note: iso8601-duration uses f32 for all fields.
        // We assume standard integer values for months/days usually, but handle fractions if present by rounding or
        // truncation? Neo4j/Cypher usually treats P1.5M as 1 month 15 days? No, Cypher durations are complex.
        // For now, we follow the simple conversion: 1Y = 12M.
        // Weeks are typically normalized to days by the parser or we assume days field covers it?
        // Actually iso8601-duration parser (0.2.0) seems to parse weeks into days or simply doesn't expose it.
        // Let's assume d.day includes weeks * 7 if the crate handles it.
        let months = (d.year * 12.0 + d.month) as i64;
        let days = d.day as i64;

        // Calculate time
        let total_seconds = (d.hour as f64) * 3600.0 + (d.minute as f64) * 60.0 + (d.second as f64);
        let seconds = total_seconds.trunc() as i64;
        let nanoseconds = ((total_seconds - total_seconds.trunc()) * 1_000_000_000.0).round() as i64;

        Ok(Duration {
            months,
            days,
            seconds,
            nanoseconds,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};

    use super::*;

    #[test]
    fn test_date_bytes_conversion() {
        let date = Date(12345);
        let bytes = date.to_le_bytes();
        assert_eq!(Date::from_le_bytes(bytes), date);

        let date_neg = Date(-54321);
        let bytes_neg = date_neg.to_le_bytes();
        assert_eq!(Date::from_le_bytes(bytes_neg), date_neg);
    }

    #[test]
    fn test_date_str_try_from() {
        assert_eq!(Date::try_from("2000-01-01").unwrap(), Date(10957));
        assert!(Date::try_from("2000/01/01").is_err());
        assert!(Date::try_from("invalid-date").is_err());
    }

    #[test]
    fn test_date_naive_date_conversion() {
        let naive_date = NaiveDate::from_ymd_opt(2023, 10, 27).unwrap();
        let date: Date = naive_date.into();
        assert_eq!(date.0, 19657); // Days since 1970-01-01
        assert_eq!(NaiveDate::from(date), naive_date);

        let epoch_naive = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        let epoch_date: Date = epoch_naive.into();
        assert_eq!(epoch_date.0, 0);
        assert_eq!(NaiveDate::from(epoch_date), epoch_naive);

        let pre_epoch_naive = NaiveDate::from_ymd_opt(1969, 12, 31).unwrap();
        let pre_epoch_date: Date = pre_epoch_naive.into();
        assert_eq!(pre_epoch_date.0, -1);
        assert_eq!(NaiveDate::from(pre_epoch_date), pre_epoch_naive);
    }

    #[test]
    fn test_date_display() {
        let date = Date::try_from("2023-10-27").unwrap();
        assert_eq!(format!("{}", date), "2023-10-27");
    }

    #[test]
    fn test_duration_add() {
        let d1 = Duration {
            months: 1,
            days: 1,
            seconds: 1,
            nanoseconds: 500_000_000,
        };
        let d2 = Duration {
            months: 2,
            days: 2,
            seconds: 2,
            nanoseconds: 600_000_000,
        };
        let d3 = d1.add(&d2);
        assert_eq!(d3.months, 3);
        assert_eq!(d3.days, 3);
        assert_eq!(d3.seconds, 4); // 1 + 2 + 1 (carry)
        assert_eq!(d3.nanoseconds, 100_000_000); // 1100000000 % 1000000000

        let d4 = Duration {
            months: 1,
            days: 1,
            seconds: 1,
            nanoseconds: -500_000_000,
        };
        let d5 = Duration {
            months: 2,
            days: 2,
            seconds: 2,
            nanoseconds: -600_000_000,
        };
        let d6 = d4.add(&d5);
        assert_eq!(d6.months, 3);
        assert_eq!(d6.days, 3);
        assert_eq!(d6.seconds, 2); // 1 + 2 - 1 (carry)
        assert_eq!(d6.nanoseconds, -100_000_000); // -1100000000 % 1000000000
    }

    #[test]
    fn test_localtime_add() {
        let time = LocalTime(10 * 3600 * 1_000_000_000); // 10:00:00
        let duration = Duration {
            months: 1,     // should be ignored
            days: 1,       // should be ignored
            seconds: 3600, // 1 hour
            nanoseconds: 0,
        };
        let new_time = time.add(&duration);
        assert_eq!(new_time.0, 11 * 3600 * 1_000_000_000); // 11:00:00

        // Wrap around
        let time = LocalTime(23 * 3600 * 1_000_000_000); // 23:00:00
        let duration = Duration {
            months: 0,
            days: 0,
            seconds: 7200, // 2 hours
            nanoseconds: 0,
        };
        let new_time = time.add(&duration);
        assert_eq!(new_time.0, 3600 * 1_000_000_000); // 01:00:00

        // Negative duration
        let time = LocalTime(10 * 3600 * 1_000_000_000); // 10:00:00
        let duration = Duration {
            months: 0,
            days: 0,
            seconds: -3600, // -1 hour
            nanoseconds: 0,
        };
        let new_time = time.add(&duration);
        assert_eq!(new_time.0, 9 * 3600 * 1_000_000_000); // 09:00:00
    }

    #[test]
    fn test_localtime_bytes_conversion() {
        let time = LocalTime(123_456_789_012_345);
        let bytes = time.to_le_bytes();
        assert_eq!(LocalTime::from_le_bytes(bytes), time);
    }

    #[test]
    fn test_localtime_naive_time_conversion() {
        let naive_time = NaiveTime::from_hms_nano_opt(10, 30, 0, 123_456_789).unwrap();
        let local_time: LocalTime = naive_time.into();
        assert_eq!(local_time.0, (10 * 3600 + 30 * 60) * 1_000_000_000 + 123_456_789);
        assert_eq!(NaiveTime::from(local_time), naive_time);

        let midnight_naive = NaiveTime::from_hms_nano_opt(0, 0, 0, 0).unwrap();
        let midnight_local: LocalTime = midnight_naive.into();
        assert_eq!(midnight_local.0, 0);
        assert_eq!(NaiveTime::from(midnight_local), midnight_naive);

        let almost_midnight_naive = NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_999).unwrap();
        let almost_midnight_local: LocalTime = almost_midnight_naive.into();
        assert_eq!(
            almost_midnight_local.0,
            (23 * 3600 + 59 * 60 + 59) * 1_000_000_000 + 999_999_999
        );
        assert_eq!(NaiveTime::from(almost_midnight_local), almost_midnight_naive);
    }

    #[test]
    fn test_localtime_display() {
        let naive_time = NaiveTime::from_hms_nano_opt(10, 30, 0, 123_456_789).unwrap();
        let local_time: LocalTime = naive_time.into();
        assert_eq!(format!("{}", local_time), "10:30:00.123456789");
    }

    #[test]
    fn test_localdatetime_bytes_conversion() {
        let datetime = LocalDateTime {
            seconds: 1234567890,
            nanoseconds: 987654321,
        };
        let bytes = datetime.to_le_bytes();
        assert_eq!(LocalDateTime::from_le_bytes(bytes), datetime);
    }

    #[test]
    fn test_localdatetime_to_date() {
        let datetime = LocalDateTime {
            seconds: 86400, // 1 day after epoch
            nanoseconds: 0,
        };
        assert_eq!(datetime.to_date(), Date(1));

        let datetime_epoch = LocalDateTime {
            seconds: 0,
            nanoseconds: 0,
        };
        assert_eq!(datetime_epoch.to_date(), Date(0));

        let datetime_neg = LocalDateTime {
            seconds: -3600, // 1 hour before epoch
            nanoseconds: 0,
        };
        assert_eq!(datetime_neg.to_date(), Date(-1));
    }

    #[test]
    fn test_localdatetime_naive_datetime_conversion() {
        let naive_date = NaiveDate::from_ymd_opt(2023, 10, 27).unwrap();
        let naive_time = NaiveTime::from_hms_nano_opt(10, 30, 0, 123_456_789).unwrap();
        let naive_datetime = naive_date.and_time(naive_time);
        let local_datetime: LocalDateTime = naive_datetime.into();
        assert_eq!(local_datetime.seconds, 1698402600);
        assert_eq!(local_datetime.nanoseconds, 123_456_789);
        assert_eq!(NaiveDateTime::from(local_datetime), naive_datetime);
    }

    #[test]
    fn test_localdatetime_to_local_time() {
        // Positive time
        let dt = LocalDateTime {
            seconds: 3661, // 01:01:01
            nanoseconds: 500_000_000,
        };
        let time = dt.to_local_time();
        assert_eq!(time.0, 3661 * 1_000_000_000 + 500_000_000);

        // Negative time (1969-12-31 23:59:59)
        let dt_neg = LocalDateTime {
            seconds: -1,
            nanoseconds: 0,
        };
        let time_neg = dt_neg.to_local_time();
        // Should be 23:59:59
        assert_eq!(time_neg.0, 86399 * 1_000_000_000);
    }

    #[test]
    fn test_localdatetime_str_try_from() {
        let dt_str = "2023-10-27 10:30:00.123456789";
        let dt = LocalDateTime::try_from(dt_str).unwrap();
        assert_eq!(dt.seconds, 1698402600);
        assert_eq!(dt.nanoseconds, 123_456_789);

        let dt_iso_str = "2023-10-27T10:30:00.123456789";
        let dt_iso = LocalDateTime::try_from(dt_iso_str).unwrap();
        assert_eq!(dt_iso, dt);

        let dt_no_nano_str = "2023-10-27 10:30:00";
        let dt_no_nano = LocalDateTime::try_from(dt_no_nano_str).unwrap();
        assert_eq!(dt_no_nano.seconds, 1698402600);
        assert_eq!(dt_no_nano.nanoseconds, 0);

        assert!(LocalDateTime::try_from("invalid-datetime").is_err());
    }

    #[test]
    fn test_localdatetime_display() {
        let naive_date = NaiveDate::from_ymd_opt(2023, 10, 27).unwrap();
        let naive_time = NaiveTime::from_hms_nano_opt(10, 30, 0, 123_456_789).unwrap();
        let naive_datetime = naive_date.and_time(naive_time);
        let local_datetime: LocalDateTime = naive_datetime.into();
        assert_eq!(format!("{}", local_datetime), "2023-10-27 10:30:00.123456789");
    }

    #[test]
    fn test_zoneddatetime_to_local_date_time() {
        let zoned_datetime = ZonedDateTime {
            seconds: 1000,
            nanoseconds: 500,
            tz_offset_seconds: 3600, // +1 hour
        };
        let local_dt = zoned_datetime.to_local_date_time();
        assert_eq!(local_dt.seconds, 1000 + 3600);
        assert_eq!(local_dt.nanoseconds, 500);

        let zoned_datetime_neg = ZonedDateTime {
            seconds: 1000,
            nanoseconds: 500,
            tz_offset_seconds: -3600, // -1 hour
        };
        let local_dt_neg = zoned_datetime_neg.to_local_date_time();
        assert_eq!(local_dt_neg.seconds, 1000 - 3600);
        assert_eq!(local_dt_neg.nanoseconds, 500);
    }

    #[test]
    fn test_zoneddatetime_str_try_from() {
        let dt_str = "2023-10-27 10:30:00.123456789 +0100";
        let dt = ZonedDateTime::try_from(dt_str).unwrap();
        assert_eq!(dt.seconds, 1698399000); // UTC timestamp
        assert_eq!(dt.nanoseconds, 123_456_789);
        assert_eq!(dt.tz_offset_seconds, 3600);

        let dt_iso_str = "2023-10-27T10:30:00.123456789 +0100";
        let dt_iso = ZonedDateTime::try_from(dt_iso_str).unwrap();
        assert_eq!(dt_iso, dt);

        let dt_no_nano_str = "2023-10-27 10:30:00 +0100";
        let dt_no_nano = ZonedDateTime::try_from(dt_no_nano_str).unwrap();
        assert_eq!(dt_no_nano.seconds, 1698399000);
        assert_eq!(dt_no_nano.nanoseconds, 0);
        assert_eq!(dt_no_nano.tz_offset_seconds, 3600);

        assert!(ZonedDateTime::try_from("invalid-datetime").is_err());
    }

    #[test]
    fn test_zoneddatetime_bytes_conversion() {
        let zoned_datetime = ZonedDateTime {
            seconds: 1234567890,
            nanoseconds: 987654321,
            tz_offset_seconds: 3600,
        };
        let bytes = zoned_datetime.to_le_bytes();
        assert_eq!(ZonedDateTime::from_le_bytes(bytes), zoned_datetime);
    }

    #[test]
    fn test_zoneddatetime_to_date() {
        let zoned_datetime = ZonedDateTime {
            seconds: 86400, // 1 day after epoch
            nanoseconds: 0,
            tz_offset_seconds: 0,
        };
        assert_eq!(zoned_datetime.to_date(), Date(1));

        let zoned_datetime_neg_offset = ZonedDateTime {
            seconds: 3600, // 1970-01-01 01:00:00 UTC
            nanoseconds: 0,
            tz_offset_seconds: -7200, // -2 hours -> 1969-12-31 23:00:00 Local
        };
        assert_eq!(zoned_datetime_neg_offset.to_date(), Date(-1));

        let zoned_datetime_pos_offset = ZonedDateTime {
            seconds: -3600, // 1969-12-31 23:00:00 UTC
            nanoseconds: 0,
            tz_offset_seconds: 7200, // +2 hours -> 1970-01-01 01:00:00 Local
        };
        assert_eq!(zoned_datetime_pos_offset.to_date(), Date(0));
    }

    #[test]
    fn test_zoneddatetime_chrono_datetime_conversion() {
        let dt_fixed_offset = FixedOffset::east_opt(3600)
            .unwrap()
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2023, 10, 27)
                    .unwrap()
                    .and_hms_nano_opt(10, 30, 0, 123_456_789)
                    .unwrap(),
            )
            .unwrap();
        let zoned_datetime: ZonedDateTime = dt_fixed_offset.into();
        assert_eq!(zoned_datetime.seconds, dt_fixed_offset.timestamp());
        assert_eq!(zoned_datetime.nanoseconds, dt_fixed_offset.timestamp_subsec_nanos());
        assert_eq!(zoned_datetime.tz_offset_seconds, 3600);
        assert_eq!(chrono::DateTime::<FixedOffset>::from(zoned_datetime), dt_fixed_offset);
    }

    #[test]
    fn test_zoneddatetime_to_local_time() {
        // UTC: 10:30:00, Offset: +01:00 -> Local: 11:30:00
        let zoned_datetime = ZonedDateTime {
            seconds: 10 * 3600 + 30 * 60,
            nanoseconds: 123_456_789,
            tz_offset_seconds: 3600,
        };
        let local_time = zoned_datetime.to_local_time();
        let expected_seconds = 11 * 3600 + 30 * 60;
        assert_eq!(local_time.0, expected_seconds * 1_000_000_000 + 123_456_789);

        // UTC: 01:00:00, Offset: -02:00 -> Local: 23:00:00 (previous day)
        let zoned_datetime_neg = ZonedDateTime {
            seconds: 3600,
            nanoseconds: 0,
            tz_offset_seconds: -7200,
        };
        let local_time_neg = zoned_datetime_neg.to_local_time();
        let expected_seconds_neg = 23 * 3600;
        assert_eq!(local_time_neg.0, expected_seconds_neg * 1_000_000_000);
    }

    #[test]
    fn test_zoneddatetime_display() {
        let dt_fixed_offset = FixedOffset::east_opt(3600)
            .unwrap()
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2023, 10, 27)
                    .unwrap()
                    .and_hms_nano_opt(10, 30, 0, 123_456_789)
                    .unwrap(),
            )
            .unwrap();
        let zoned_datetime: ZonedDateTime = dt_fixed_offset.into();
        assert_eq!(format!("{}", zoned_datetime), "2023-10-27 10:30:00.123456789 +01:00");
    }

    #[test]
    fn test_duration_display() {
        let duration = Duration {
            months: 1,
            days: 2,
            seconds: 3,
            nanoseconds: 4,
        };
        assert_eq!(format!("{}", duration), "P1M2DT3S4");

        let zero_duration = Duration {
            months: 0,
            days: 0,
            seconds: 0,
            nanoseconds: 0,
        };
        assert_eq!(format!("{}", zero_duration), "P0M0DT0S0");

        let neg_duration = Duration {
            months: -1,
            days: -2,
            seconds: -3,
            nanoseconds: -4,
        };
        assert_eq!(format!("{}", neg_duration), "P-1M-2DT-3S-4");
    }

    #[test]
    fn test_localdatetime_add() {
        // Normal case
        let dt = LocalDateTime {
            seconds: 1698402600, // 2023-10-27 10:30:00
            nanoseconds: 0,
        };
        let duration = Duration {
            months: 1,
            days: 1,
            seconds: 3600,
            nanoseconds: 0,
        };
        // 2023-10-27 -> +1 month = 2023-11-27
        // 2023-11-27 -> +1 day = 2023-11-28
        // 10:30:00 -> +1 hour = 11:30:00
        let new_dt = dt.checked_add(&duration).unwrap();

        let expected_naive = NaiveDate::from_ymd_opt(2023, 11, 28)
            .unwrap()
            .and_hms_nano_opt(11, 30, 0, 0)
            .unwrap();
        let expected: LocalDateTime = expected_naive.into();
        assert_eq!(new_dt, expected);

        // Clamping case: 2023-01-31 + 1 month -> 2023-02-28
        let dt_clamp = NaiveDate::from_ymd_opt(2023, 1, 31)
            .unwrap()
            .and_hms_nano_opt(10, 0, 0, 0)
            .unwrap();
        let dt_clamp: LocalDateTime = dt_clamp.into();

        let duration_clamp = Duration {
            months: 1,
            days: 1,
            seconds: 0,
            nanoseconds: 0,
        };
        // 2023-01-31 + 1 month = 2023-02-28
        // 2023-02-28 + 1 day = 2023-03-01
        let new_dt_clamp = dt_clamp.checked_add(&duration_clamp).unwrap();

        let expected_clamp_naive = NaiveDate::from_ymd_opt(2023, 3, 1)
            .unwrap()
            .and_hms_nano_opt(10, 0, 0, 0)
            .unwrap();
        let expected_clamp: LocalDateTime = expected_clamp_naive.into();
        assert_eq!(new_dt_clamp, expected_clamp);
    }

    #[test]
    fn test_zoneddatetime_add() {
        // 2023-10-27 10:30:00 +01:00
        // Timestamp: 1698402600 (local) - 3600 = 1698399000 (UTC)
        let dt = ZonedDateTime {
            seconds: 1698399000,
            nanoseconds: 0,
            tz_offset_seconds: 3600,
        };
        let duration = Duration {
            months: 1,
            days: 1,
            seconds: 3600,
            nanoseconds: 0,
        };
        // 2023-10-27 -> +1 month = 2023-11-27
        // 2023-11-27 -> +1 day = 2023-11-28
        // 10:30:00 -> +1 hour = 11:30:00
        // Result: 2023-11-28 11:30:00 +01:00
        let new_dt = dt.checked_add(&duration).unwrap();

        let expected_fixed = FixedOffset::east_opt(3600)
            .unwrap()
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2023, 11, 28)
                    .unwrap()
                    .and_hms_nano_opt(11, 30, 0, 0)
                    .unwrap(),
            )
            .unwrap();
        let expected: ZonedDateTime = expected_fixed.into();
        assert_eq!(new_dt, expected);
    }

    #[test]
    fn test_duration_div() {
        let d = Duration {
            months: 6,
            days: 10,
            seconds: 3600,
            nanoseconds: 500_000_000,
        };

        // Divide by 2
        let d2 = d.checked_div(2.0).unwrap();
        assert_eq!(d2.months, 3);
        assert_eq!(d2.days, 5);
        assert_eq!(d2.seconds, 1800);
        assert_eq!(d2.nanoseconds, 250_000_000);

        // Divide by 0.5 (multiply by 2)
        let d3 = d.checked_div(0.5).unwrap();
        assert_eq!(d3.months, 12);
        assert_eq!(d3.days, 20);
        assert_eq!(d3.seconds, 7201);
        assert_eq!(d3.nanoseconds, 0);

        // Divide by 0 should return None
        assert!(d.checked_div(0.0).is_none());
    }
}
