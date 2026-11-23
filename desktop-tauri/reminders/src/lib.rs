//! In-memory scheduler for the shared Countdown to Death UI state.

mod calendar;
mod format;

use std::collections::HashMap;

use chrono::{
    DateTime, Datelike, Days, Duration, Local, Months, NaiveDateTime, NaiveTime, TimeZone, Utc,
};
use serde_json::Value;

use calendar::{clamped_date, parse_local, parse_time, resolve};

const MAX_LATENESS: Duration = Duration::seconds(60);

#[derive(Debug, PartialEq, Eq)]
pub struct Alert {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Default)]
pub struct Scan {
    pub due: Vec<Alert>,
    pub next_at: Option<DateTime<Local>>,
}

/// Keep one scheduler for the lifetime of the host. Pending occurrences and
/// delivery history are intentionally not persisted across restarts.
#[derive(Default)]
pub struct Scheduler {
    pending: HashMap<Key, Occurrence>,
    delivered: HashMap<Key, DateTime<Utc>>,
    last_now: Option<DateTime<Utc>>,
}

#[derive(PartialEq, Eq, Hash)]
struct Key {
    profile: String,
    target: String,
    reminder: Option<(String, Timing)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Timing {
    Once(NaiveDateTime),
    Daily(NaiveTime),
    Weekly(NaiveTime, u32),
    Monthly(NaiveTime, u32),
}

#[derive(Clone, Copy)]
struct Occurrence {
    wall: NaiveDateTime,
    at: DateTime<Utc>,
}

impl Scheduler {
    /// Scan on startup, state changes, resume, and at `next_at`. A periodic
    /// heartbeat also lets the host notice wall-clock changes. The first scan
    /// arms future occurrences only; a later scan may deliver armed occurrences
    /// up to 60 seconds late. Ties retain persisted profile/reminder order.
    pub fn scan(&mut self, state: &Value, now: DateTime<Local>) -> Scan {
        let (due, next_at) = self.scan_in(state, now);
        Scan { due, next_at }
    }

    fn scan_in<Tz: TimeZone>(
        &mut self,
        state: &Value,
        now: DateTime<Tz>,
    ) -> (Vec<Alert>, Option<DateTime<Tz>>) {
        let zone = now.timezone();
        let now = now.to_utc();
        let mut previous = std::mem::take(&mut self.pending);
        if self.last_now.is_some_and(|last| now < last) {
            previous.clear();
        }
        self.last_now = Some(now);

        let mut due = Vec::new();
        let mut next_at = None;
        for profile in array(&state["profiles"]) {
            let (Some(profile_id), Some(raw_target)) =
                (profile["id"].as_str(), profile["target"].as_str())
            else {
                continue;
            };
            let Some(target_wall) = parse_local(raw_target) else {
                continue;
            };
            let Some(target) = resolve(&zone, target_wall) else {
                continue;
            };
            let target_at = target.to_utc();
            let title = profile["name"]
                .as_str()
                .filter(|name| !name.is_empty())
                .unwrap_or("Countdown to Death");
            let mut visit = |reminder: Option<(&Value, &str, Timing)>| {
                let key = Key {
                    profile: profile_id.to_owned(),
                    target: raw_target.to_owned(),
                    reminder: reminder.map(|(_, id, timing)| (id.to_owned(), timing)),
                };
                let mut last_delivered = self.delivered.get(&key).copied();
                // Re-resolve armed wall times so a system zone change can't fire
                // a stale plan, without mistaking a DST offset change for a new zone.
                let mut pending = previous.remove(&key).filter(|event| {
                    (reminder.is_none() || event.at < target_at)
                        && resolve(&zone, event.wall).is_some_and(|at| at.to_utc() == event.at)
                });
                if pending.is_some_and(|event| event.at <= now) {
                    let event = pending.take().expect("checked pending occurrence");
                    if last_delivered.is_none_or(|last| event.at > last)
                        && now - event.at <= MAX_LATENESS
                    {
                        let body = match reminder {
                            None => "Time’s up. Countdown complete.".to_owned(),
                            Some((r, _, _)) => {
                                let at = event.at.with_timezone(&zone);
                                let fmt = r
                                    .get("format")
                                    .filter(|fmt| !fmt.is_null())
                                    .unwrap_or(&profile["format"]);
                                format::remaining(&target, fmt, &at)
                                    .unwrap_or_else(|| "Countdown reminder.".to_owned())
                            }
                        };
                        due.push((
                            event.at,
                            Alert {
                                title: title.to_owned(),
                                body,
                            },
                        ));
                        last_delivered = Some(event.at);
                        self.delivered.insert(
                            Key {
                                profile: key.profile.clone(),
                                target: key.target.clone(),
                                reminder: key.reminder.clone(),
                            },
                            event.at,
                        );
                    }
                }

                let after = last_delivered.map_or(now, |last| now.max(last));
                let next = pending.or_else(|| {
                    if after >= target_at {
                        return None;
                    }
                    match reminder {
                        None => Some(Occurrence {
                            wall: target_wall,
                            at: target_at,
                        }),
                        Some((_, _, timing)) => timing
                            .next(&zone, after)
                            .filter(|event| event.at < target_at),
                    }
                });
                if let Some(event) = next {
                    next_at =
                        Some(next_at.map_or(event.at, |next: DateTime<Utc>| next.min(event.at)));
                    self.pending.insert(key, event);
                }
            };

            if profile["notifyAtTarget"].as_bool() == Some(true) {
                visit(None);
            }
            for reminder in array(&profile["reminders"]) {
                if reminder["enabled"].as_bool() == Some(true) {
                    if let (Some(id), Some(timing)) =
                        (reminder["id"].as_str(), Timing::parse(reminder))
                    {
                        visit(Some((reminder, id, timing)));
                    }
                }
            }
        }
        due.sort_by_key(|(at, _)| *at);
        (
            due.into_iter().map(|(_, alert)| alert).collect(),
            next_at.map(|at| at.with_timezone(&zone)),
        )
    }
}

fn array(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}

impl Timing {
    fn parse(reminder: &Value) -> Option<Self> {
        let time = || parse_time(reminder["time"].as_str()?);
        let index = |field: &str, min: u32, max: u32| {
            let text = reminder[field].as_str()?;
            let value = text.parse::<u32>().ok()?;
            ((min..=max).contains(&value) && value.to_string() == text).then_some(value)
        };
        Some(match reminder["repeat"].as_str()? {
            "once" => Self::Once(parse_local(reminder["once"].as_str()?)?),
            "daily" => Self::Daily(time()?),
            "weekly" => Self::Weekly(time()?, index("weekday", 0, 6)?),
            "monthly" => Self::Monthly(time()?, index("day", 1, 31)?),
            _ => return None,
        })
    }

    fn next<Tz: TimeZone>(self, zone: &Tz, after: DateTime<Utc>) -> Option<Occurrence> {
        let event = |wall| {
            Some(Occurrence {
                wall,
                at: resolve(zone, wall)?.to_utc(),
            })
        };
        if let Self::Once(wall) = self {
            return event(wall).filter(|event| event.at > after);
        }

        let mut date = after.with_timezone(zone).date_naive();
        let (time, step) = match self {
            Self::Daily(time) => (time, 1),
            Self::Weekly(time, weekday) => {
                let distance = (weekday + 7 - date.weekday().num_days_from_sunday()) % 7;
                date = date.checked_add_days(Days::new(distance.into()))?;
                (time, 7)
            }
            Self::Monthly(time, _) => {
                date = date.with_day(1)?;
                (time, 0)
            }
            Self::Once(_) => unreachable!(),
        };
        loop {
            let nominal = match self {
                Self::Monthly(_, day) => clamped_date(date.year(), date.month(), day)?,
                _ => date,
            };
            let occurrence = event(nominal.and_time(time))?;
            if occurrence.at > after {
                return Some(occurrence);
            }
            date = match self {
                Self::Monthly(..) => date.checked_add_months(Months::new(1))?,
                _ => date.checked_add_days(Days::new(step))?,
            };
        }
    }
}

#[cfg(test)]
mod tests;
