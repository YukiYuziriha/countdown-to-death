use chrono::{FixedOffset, NaiveDateTime};
use chrono_tz::{America::New_York, Australia::Lord_Howe, Pacific::Apia, Tz};
use serde_json::json;

use super::*;

fn wall(text: &str) -> NaiveDateTime {
    parse_local(text).unwrap()
}

fn utc(text: &str) -> DateTime<Utc> {
    wall(text).and_utc()
}

fn zoned(zone: Tz, text: &str) -> DateTime<Tz> {
    resolve(&zone, wall(text)).unwrap()
}

#[test]
fn delivers_all_ties_in_state_order_and_completes_at_target() {
    let state = json!({"profiles": [
        {"id":"a", "name":"A", "target":"2026-12-03T09:00", "notifyAtTarget":true,
         "format":{"units":["days"]}, "reminders":[
             {"id":"daily", "repeat":"daily", "enabled":true, "time":"09:00"},
             {"id":"once", "repeat":"once", "enabled":true, "once":"2026-12-02T09:00",
              "format":{"units":["hours"]}},
             {"id":"paused", "repeat":"daily", "enabled":false, "time":"09:00"}
         ]},
        {"id":"b", "name":"B", "target":"2026-12-02T09:00", "notifyAtTarget":true,
         "reminders":[{"id":"same-as-target", "repeat":"daily", "enabled":true, "time":"09:00"}]}
    ]});
    let mut scheduler = Scheduler::default();
    let at = utc("2026-12-02T09:00");
    assert_eq!(
        scheduler.scan_in(&state, at - Duration::milliseconds(1)),
        (vec![], Some(at))
    );
    let (due, next) = scheduler.scan_in(&state, at);
    assert_eq!(
        due.iter()
            .map(|a| (a.title.as_str(), a.body.as_str()))
            .collect::<Vec<_>>(),
        [
            ("A", "1 day left"),
            ("A", "24 hours left"),
            ("B", "Time’s up. Countdown complete.")
        ]
    );
    let target = utc("2026-12-03T09:00");
    assert_eq!(next, Some(target));
    assert!(scheduler.scan_in(&state, at).0.is_empty());
    let (due, next) = scheduler.scan_in(&state, target);
    assert_eq!(
        due,
        [Alert {
            title: "A".into(),
            body: "Time’s up. Countdown complete.".into()
        }]
    );
    assert_eq!(next, None);
}

#[test]
fn handles_startup_lateness_and_rewind_even_after_pause() {
    let mut state = json!({"profiles":[{
        "id":"a", "target":"2026-12-05T09:00", "reminders":[
            {"id":"daily", "repeat":"daily", "enabled":true, "time":"09:00"},
            {"id":"once", "repeat":"once", "enabled":true, "once":"2026-12-02T09:00"}
        ]
    }]});
    let mut scheduler = Scheduler::default();
    let at = utc("2026-12-02T09:00");
    let tomorrow = at + Duration::days(1);
    assert_eq!(scheduler.scan_in(&state, at), (vec![], Some(tomorrow)));
    assert_eq!(
        scheduler.scan_in(&state, tomorrow + MAX_LATENESS).0.len(),
        1
    );
    let daily = &mut state["profiles"][0]["reminders"][0];
    daily["enabled"] = json!(false);
    assert_eq!(
        scheduler.scan_in(&state, tomorrow + MAX_LATENESS),
        (vec![], None)
    );
    state["profiles"][0]["reminders"][0]["enabled"] = json!(true);
    let day_after = tomorrow + Duration::days(1);
    assert_eq!(
        scheduler.scan_in(&state, tomorrow - Duration::seconds(1)),
        (vec![], Some(day_after))
    );
    assert_eq!(
        scheduler.scan_in(&state, tomorrow),
        (vec![], Some(day_after))
    );
    assert_eq!(
        scheduler.scan_in(&state, day_after + MAX_LATENESS + Duration::milliseconds(1)),
        (vec![], None)
    );
}

#[test]
fn edits_cancel_old_plans_but_presentation_changes_apply_to_armed_alert() {
    let mut state = json!({"profiles":[{
        "id":"a", "name":"Before", "target":"2026-12-02T10:00", "notifyAtTarget":true,
        "reminders":[
            {"id":"edited", "repeat":"daily", "enabled":true, "time":"09:00"},
            {"id":"keep", "repeat":"once", "enabled":true, "once":"2026-12-02T09:00:00.125"}
        ]
    }]});
    let mut scheduler = Scheduler::default();
    let at = utc("2026-12-02T09:00:00.125");
    scheduler.scan_in(&state, at - Duration::seconds(1));
    let profile = &mut state["profiles"][0];
    profile["name"] = json!("After");
    profile["notifyAtTarget"] = json!(false);
    profile["reminders"][0]["time"] = json!("09:30");
    profile["reminders"][1]["format"] = json!({"units":["hours"], "decimal":true});
    let (due, next) = scheduler.scan_in(&state, at + Duration::seconds(30));
    assert_eq!(
        due,
        [Alert {
            title: "After".into(),
            body: "1 hour left".into()
        }]
    );
    assert_eq!(next, Some(utc("2026-12-02T09:30")));
    state["profiles"][0]["target"] = json!("2026-12-02T09:15");
    assert_eq!(
        scheduler.scan_in(&state, utc("2026-12-02T09:30")),
        (vec![], None)
    );
}

#[test]
fn recurrence_clamps_months_without_drift_and_uses_sunday_zero() {
    let monthly = Timing::parse(&json!({"repeat":"monthly", "time":"08:00", "day":"31"})).unwrap();
    let weekly = Timing::parse(&json!({"repeat":"weekly", "time":"08:00", "weekday":"0"})).unwrap();
    let feb = monthly.next(&Utc, utc("2024-01-31T08:00")).unwrap().at;
    assert_eq!(feb, utc("2024-02-29T08:00"));
    assert_eq!(monthly.next(&Utc, feb).unwrap().at, utc("2024-03-31T08:00"));
    assert_eq!(weekly.next(&Utc, feb).unwrap().at, utc("2024-03-03T08:00"));
    assert_eq!(
        monthly.next(&Utc, utc("2025-01-31T08:00")).unwrap().at,
        utc("2025-02-28T08:00")
    );
}

#[test]
fn resolves_dst_gaps_and_overlaps_and_keeps_recurring_wall_time() {
    assert_eq!(
        zoned(New_York, "2024-03-10T02:30").to_utc(),
        utc("2024-03-10T07:30")
    );
    assert_eq!(
        zoned(New_York, "2024-11-03T01:30").to_utc(),
        utc("2024-11-03T05:30")
    );
    assert_eq!(
        zoned(Lord_Howe, "2024-10-06T02:15").naive_local(),
        wall("2024-10-06T02:45")
    );
    assert_eq!(
        zoned(Apia, "2011-12-30T02:30").naive_local(),
        wall("2011-12-31T02:30")
    );
    let state = json!({"profiles":[{"id":"a", "target":"2024-11-06T09:00", "reminders":[
        {"id":"r", "repeat":"daily", "enabled":true, "time":"01:30"}
    ]}]});
    let mut scheduler = Scheduler::default();
    let before = utc("2024-11-03T05:29").with_timezone(&New_York);
    let first = utc("2024-11-03T05:30").with_timezone(&New_York);
    let second = utc("2024-11-03T06:30").with_timezone(&New_York);
    let tomorrow = zoned(New_York, "2024-11-04T01:30");
    assert_eq!(scheduler.scan_in(&state, before), (vec![], Some(first)));
    let (due, next) = scheduler.scan_in(&state, first);
    assert_eq!(due.len(), 1);
    assert_eq!(next, Some(tomorrow));
    assert_eq!(scheduler.scan_in(&state, second), (vec![], Some(tomorrow)));
}

#[test]
fn a_changed_zone_invalidates_old_wall_clock_plans() {
    let state =
        json!({"profiles":[{"id":"a", "target":"2026-12-02T09:00", "notifyAtTarget":true}]});
    let old = FixedOffset::east_opt(3600).unwrap();
    let new = FixedOffset::east_opt(0).unwrap();
    let mut scheduler = Scheduler::default();
    let at = utc("2026-12-02T08:00");
    assert_eq!(
        scheduler
            .scan_in(&state, (at - Duration::seconds(1)).with_timezone(&old))
            .1,
        Some(at.with_timezone(&old))
    );
    assert_eq!(
        scheduler.scan_in(&state, at.with_timezone(&new)),
        (vec![], Some((at + Duration::hours(1)).with_timezone(&new)))
    );
}

#[test]
fn formatting_matches_calendar_boundaries_fixed_units_and_rounding() {
    let remaining = |from, to, units, decimal| {
        format::remaining(
            &zoned(New_York, to),
            &json!({"units": units, "decimal":decimal}),
            &zoned(New_York, from),
        )
        .unwrap()
    };
    assert_eq!(
        remaining(
            "2024-02-10T02:30",
            "2024-03-10T02:30",
            &["months", "hours"][..],
            false
        ),
        "1 month, 0 hours left"
    );
    assert_eq!(
        remaining(
            "2024-03-09T12:00",
            "2024-03-10T12:00",
            &["days", "hours"],
            false
        ),
        "0 days, 23 hours left"
    );
    assert_eq!(
        remaining(
            "2024-02-29T12:00",
            "2025-03-29T15:30",
            &["years", "months", "days", "hours", "minutes"],
            false
        ),
        "1 year, 1 month, 1 day, 3 hours, 30 minutes left"
    );
    assert_eq!(
        remaining("2023-01-31T12:00", "2023-03-16T12:30", &["months"], true),
        "1.5 months left"
    );
    assert_eq!(
        remaining(
            "2025-01-01T12:00",
            "2025-01-01T12:00:57",
            &["minutes"],
            true
        ),
        "1 minute left"
    );
    assert_eq!(
        remaining(
            "2025-01-01T12:00",
            "2025-01-01T12:20:34.650",
            &["seconds"],
            true
        ),
        "1,234.7 seconds left"
    );
}

#[test]
fn rejects_malformed_datetimes_and_tolerates_empty_state_through_public_api() {
    for invalid in [
        "2026-02-29T08:00",
        "0000-01-01T08:00",
        "2026-01-01T8:00",
        "2026-01-01T24:00",
        "2026-01-01T08:00Z",
        "2026-01-01T08:00:60",
        "2026-01-01T08:00:00.1234",
        "2026-01-01",
    ] {
        assert_eq!(parse_local(invalid), None, "{invalid}");
    }
    let now = utc("2026-01-01T08:00").with_timezone(&Local);
    let mut scheduler = Scheduler::default();
    for state in [
        Value::Null,
        json!({"profiles": "invalid"}),
        json!({"profiles":[null, 5, {"id":"bad", "target":"bad"}]}),
    ] {
        let scan = scheduler.scan(&state, now);
        assert!(scan.due.is_empty());
        assert_eq!(scan.next_at, None);
    }
}
