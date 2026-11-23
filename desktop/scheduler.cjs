'use strict';

const MAX_LATENESS_MS = 60_000;
const HEARTBEAT_MS = 10_000;

function* schedules(profiles) {
  for (const profile of profiles) {
    if (profile.notifyAtTarget === true) {
      yield {
        key: JSON.stringify([profile.id, profile.target, 'target']),
        profile,
        reminder: null,
      };
    }
    for (const reminder of profile.reminders ?? []) {
      if (reminder.enabled !== true) continue;
      const timing = {
        once: [reminder.once], daily: [reminder.time],
        weekly: [reminder.time, reminder.weekday], monthly: [reminder.time, reminder.day],
      };
      yield {
        key: JSON.stringify([profile.id, profile.target, 'reminder', reminder.id,
          reminder.repeat, timing[reminder.repeat]]),
        profile,
        reminder,
      };
    }
  }
}

// Keep each series armed separately: the shared helper returns only one event per call.
function createScheduler(getNext, { maxLatenessMs = MAX_LATENESS_MS } = {}) {
  let pending = new Map();
  const delivered = new Map();
  let lastNow = -Infinity;
  let lastZone;

  function scan(profiles, now, zone = Intl.DateTimeFormat().resolvedOptions().timeZone) {
    if (now < lastNow || zone !== lastZone) pending.clear();
    lastNow = now;
    lastZone = zone;

    const nextPending = new Map();

    const due = [];
    let nextAt = Infinity;
    for (const schedule of schedules(profiles)) {
      const { key, profile, reminder } = schedule;
      let at = pending.get(key);
      let lastDelivered = delivered.get(key) ?? -Infinity;
      if (at !== undefined && at <= now) {
        if (at > lastDelivered && now - at <= maxLatenessMs) {
          due.push({ at, profile, reminder });
          lastDelivered = at;
          delivered.set(key, at);
        }
        at = undefined;
      }

      if (at === undefined) {
        const next = getNext({ ...profile, notifyAtTarget: reminder === null,
          reminders: reminder ? [reminder] : [] }, new Date(Math.max(now, lastDelivered)));
        at = next?.at.getTime();
      }
      if (at > now) {
        nextPending.set(key, at);
        nextAt = Math.min(nextAt, at);
      }
    }
    pending = nextPending;

    return { due: due.sort((a, b) => a.at - b.at), nextAt };
  }

  return { scan };
}

module.exports = { createScheduler, HEARTBEAT_MS };
