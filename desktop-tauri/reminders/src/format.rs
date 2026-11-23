use chrono::{DateTime, Datelike, Duration, TimeZone};
use serde_json::Value;

use crate::calendar::{clamped_date, resolve};

#[derive(Clone, Copy)]
enum Unit {
    Calendar(i64), // months per unit
    Fixed(i64),    // milliseconds per unit
}

impl Unit {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "years" => Self::Calendar(12),
            "months" => Self::Calendar(1),
            "weeks" => Self::Fixed(604_800_000),
            "days" => Self::Fixed(86_400_000),
            "hours" => Self::Fixed(3_600_000),
            "minutes" => Self::Fixed(60_000),
            "seconds" => Self::Fixed(1_000),
            _ => return None,
        })
    }

    fn advance<Tz: TimeZone>(self, start: &DateTime<Tz>, count: i64) -> Option<DateTime<Tz>> {
        if count == 0 {
            return Some(start.clone());
        }
        match self {
            Self::Fixed(ms) => start
                .clone()
                .checked_add_signed(Duration::milliseconds(count * ms)),
            Self::Calendar(months) => {
                let wall = start.naive_local();
                let index = i64::from(wall.year()) * 12 + i64::from(wall.month0()) + count * months;
                let year = i32::try_from(index.div_euclid(12)).ok()?;
                let month = index.rem_euclid(12) as u32 + 1;
                let date = clamped_date(year, month, wall.day())?;
                resolve(&start.timezone(), date.and_time(wall.time()))
            }
        }
    }

    fn whole_count<Tz: TimeZone>(self, start: &DateTime<Tz>, end: &DateTime<Tz>) -> i64 {
        match self {
            Self::Fixed(ms) => (end.timestamp_millis() - start.timestamp_millis()) / ms,
            Self::Calendar(months) => {
                let diff = i64::from(end.year() - start.year()) * 12
                    + i64::from(end.month() as i32 - start.month() as i32);
                (diff / months).max(0)
            }
        }
    }
}

pub(super) fn remaining<Tz: TimeZone>(
    target: &DateTime<Tz>,
    format: &Value,
    at: &DateTime<Tz>,
) -> Option<String> {
    let units = format.get("units")?.as_array()?;
    if units.is_empty() {
        return None;
    }
    let mut cursor = at.min(target).clone();
    let mut parts = Vec::with_capacity(units.len());
    for (i, name) in units.iter().enumerate() {
        let name = name.as_str()?;
        let unit = Unit::parse(name)?;
        let mut count = unit.whole_count(&cursor, target);
        let next = loop {
            if let Some(next) = unit.advance(&cursor, count) {
                if next <= *target {
                    break next;
                }
            }
            count -= 1;
        };
        let mut tenths = count * 10;
        if format["decimal"].as_bool() == Some(true) && i + 1 == units.len() && target > &next {
            let boundary = unit.advance(&cursor, count + 1)?;
            let span = boundary.timestamp_millis() - next.timestamp_millis();
            let remainder = target.timestamp_millis() - next.timestamp_millis();
            if span > 0 {
                tenths += (20 * remainder + span) / (2 * span);
            }
        }
        cursor = next;
        let label = if tenths == 10 {
            name.strip_suffix('s')?
        } else {
            name
        };
        parts.push(format!("{} {label}", number(tenths)));
    }
    Some(format!("{} left", parts.join(", ")))
}

// The shared formatter uses the host's Intl locale. Notifications here use
// stable English decimal and grouping separators without an ICU dependency.
fn number(tenths: i64) -> String {
    let integer = (tenths / 10).to_string();
    let mut text = String::new();
    let first_group = integer.len() % 3;
    for (i, digit) in integer.chars().enumerate() {
        if i != 0 && i % 3 == first_group {
            text.push(',');
        }
        text.push(digit);
    }
    if tenths % 10 != 0 {
        text.push('.');
        text.push(char::from(b'0' + (tenths % 10) as u8));
    }
    text
}
