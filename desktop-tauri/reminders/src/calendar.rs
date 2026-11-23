use chrono::{
    DateTime, Duration, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone,
};

pub(super) fn parse_local(text: &str) -> Option<NaiveDateTime> {
    let (date, time) = text.split_once(['T', ' '])?;
    let (year, rest) = date.split_once('-')?;
    let (month, day) = rest.split_once('-')?;
    if !(4..=6).contains(&year.len()) {
        return None;
    }
    let year = digits(year, year.len())?;
    if year == 0 {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(year as i32, digits(month, 2)?, digits(day, 2)?)?;
    Some(date.and_time(parse_time(time)?))
}

pub(super) fn parse_time(text: &str) -> Option<NaiveTime> {
    let (hour, rest) = text.split_once(':')?;
    let (minute, seconds) = rest.split_once(':').unwrap_or((rest, "00"));
    let (second, millis) = match seconds.split_once('.') {
        Some((second, fraction)) if (1..=3).contains(&fraction.len()) => (
            second,
            digits(fraction, fraction.len())? * 10_u32.pow(3 - fraction.len() as u32),
        ),
        Some(_) => return None,
        None => (seconds, 0),
    };
    NaiveTime::from_hms_milli_opt(
        digits(hour, 2)?,
        digits(minute, 2)?,
        digits(second, 2)?,
        millis,
    )
}

fn digits(text: &str, width: usize) -> Option<u32> {
    if text.len() != width || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

pub(super) fn clamped_date(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    (1..=day)
        .rev()
        .find_map(|day| NaiveDate::from_ymd_opt(year, month, day))
}

/// Match JavaScript's local-time disambiguation: earlier instant in an overlap,
/// and advance by the size of the offset change in a gap (not to its first minute).
pub(super) fn resolve<Tz: TimeZone>(zone: &Tz, wall: NaiveDateTime) -> Option<DateTime<Tz>> {
    let before = match zone.from_local_datetime(&wall) {
        LocalResult::Single(at) => return Some(at),
        LocalResult::Ambiguous(a, b) => return Some(a.min(b)),
        LocalResult::None => nearby_offset(zone, wall, -1)?,
    };
    let after = nearby_offset(zone, wall, 1)?;
    let gap = after - before;
    if gap <= 0 {
        return None;
    }
    let shifted = wall.checked_add_signed(Duration::seconds(i64::from(gap)))?;
    zone.from_local_datetime(&shifted).earliest()
}

fn nearby_offset<Tz: TimeZone>(zone: &Tz, wall: NaiveDateTime, direction: i64) -> Option<i32> {
    // Exponential probes cover both sub-hour gaps and skipped calendar days.
    for power in 0..=18 {
        let probe = wall.checked_add_signed(Duration::seconds(direction * (1 << power)))?;
        let result = zone.offset_from_local_datetime(&probe);
        let offset = if direction < 0 {
            result.latest()
        } else {
            result.earliest()
        };
        if let Some(offset) = offset {
            return Some(offset.fix().local_minus_utc());
        }
    }
    None
}
