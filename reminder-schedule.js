(() => {
  'use strict';

  const timePattern = /^(?:[01]\d|2[0-3]):[0-5]\d(?::[0-5]\d(?:\.\d{1,3})?)?$/;

  function daysInMonth(year, month) {
    if (month === 2) {
      return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 29 : 28;
    }
    return [4, 6, 9, 11].includes(month) ? 30 : 31;
  }

  function localDate(year, month, day, time) {
    const y = year > 9999 ? `+${String(year).padStart(6, '0')}` : String(year).padStart(4, '0');
    const pad = value => String(value).padStart(2, '0');
    // Native local-time rules shift spring gaps forward and choose the earlier fall overlap.
    return new Date(`${y}-${pad(month)}-${pad(day)}T${time}`);
  }

  function parseLocalDate(value) {
    const parts = typeof value === 'string' && /^(\d{4,6})-(\d{2})-(\d{2})[T ](\S+)$/.exec(value);
    if (!parts || !timePattern.test(parts[4])) return null;
    const year = Number(parts[1]);
    const month = Number(parts[2]);
    const day = Number(parts[3]);
    if (!year || month < 1 || month > 12 || day < 1 || day > daysInMonth(year, month)) return null;
    return localDate(year, month, day, parts[4]);
  }

  function nextOccurrence(reminder, now, nowMs) {
    const { repeat, time, weekday, day } = reminder;
    if (repeat === 'once') return parseLocalDate(reminder.once);
    if (typeof time !== 'string' || !timePattern.test(time)) return null;

    // Use UTC only as a calendar cursor; each occurrence is built in local time.
    const calendar = new Date(0);
    calendar.setUTCFullYear(now.getFullYear(), now.getMonth(), now.getDate());
    let step = 1;
    if (repeat === 'weekly') {
      if (typeof weekday !== 'string' || !/^[0-6]$/.test(weekday)) return null;
      calendar.setUTCDate(calendar.getUTCDate() + (Number(weekday) - calendar.getUTCDay() + 7) % 7);
      step = 7;
    } else if (repeat === 'monthly') {
      if (typeof day !== 'string' || !/^(?:[1-9]|[12]\d|3[01])$/.test(day)) return null;
      calendar.setUTCDate(1);
    } else if (repeat !== 'daily') {
      return null;
    }

    while (Number.isFinite(calendar.getTime())) {
      const year = calendar.getUTCFullYear();
      const month = calendar.getUTCMonth() + 1;
      const date = repeat === 'monthly' ? Math.min(Number(day), daysInMonth(year, month)) : calendar.getUTCDate();
      const at = localDate(year, month, date, time);
      const timestamp = at.getTime();
      if (!Number.isFinite(timestamp)) return null;
      if (timestamp > nowMs) return at;
      if (repeat === 'monthly') calendar.setUTCMonth(month);
      else calendar.setUTCDate(date + step);
    }
    return null;
  }

  window.getNextCountdownReminder = function getNextCountdownReminder(profile, now = new Date()) {
    if (!profile || typeof profile !== 'object') return null;
    let nowMs;
    try {
      nowMs = Date.prototype.getTime.call(now);
    } catch {
      return null;
    }
    if (!Number.isFinite(nowMs)) return null;

    const target = parseLocalDate(profile.target);
    const targetMs = target?.getTime();
    if (!(targetMs > nowMs)) return null;

    const localNow = new Date(nowMs);
    let best = null;
    let bestMs = targetMs;
    for (const reminder of Array.isArray(profile.reminders) ? profile.reminders : []) {
      if (reminder?.enabled !== true) continue;
      const at = nextOccurrence(reminder, localNow, nowMs);
      const timestamp = at?.getTime();
      if (timestamp > nowMs && timestamp < bestMs) {
        best = { at, kind: 'scheduled', reminder };
        bestMs = timestamp;
      }
    }

    return best ?? (profile.notifyAtTarget === true ? { at: target, kind: 'target', reminder: null } : null);
  };
})();
