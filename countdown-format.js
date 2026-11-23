(() => {
  'use strict';

  const sizes = { weeks: 604800, days: 86400, hours: 3600, minutes: 60, seconds: 1 };
  const numberOptions = { maximumFractionDigits: 1 };
  const defaultNumberFormat = new Intl.NumberFormat(undefined, numberOptions);

  function formatNumber(value, locale) {
    return (locale === undefined ? defaultNumberFormat : new Intl.NumberFormat(locale, numberOptions)).format(value);
  }

  function daysInMonth(year, month) {
    if (month === 1) {
      return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 29 : 28;
    }
    return [3, 5, 8, 10].includes(month) ? 30 : 31;
  }

  function advance(start, unit, count) {
    if (count === 0) return new Date(start);
    if (unit !== 'years' && unit !== 'months') {
      return new Date(+start + count * sizes[unit] * 1000);
    }

    const monthIndex = start.getMonth() + (unit === 'years' ? 12 * count : count);
    const year = start.getFullYear() + Math.floor(monthIndex / 12);
    const month = monthIndex % 12;
    const day = Math.min(start.getDate(), daysInMonth(year, month));
    const next = new Date(start);
    // Resolve DST only on the final clamped date, not on an intermediate day.
    next.setFullYear(year, month, day);
    return next;
  }

  function parts(target, fmt, now = new Date()) {
    const end = new Date(target);
    const chosen = fmt.units;
    let cursor = new Date(Math.min(+new Date(now), +end));

    return chosen.map((unit, index) => {
      let count;
      if (unit === 'years' || unit === 'months') {
        const years = end.getFullYear() - cursor.getFullYear();
        count = unit === 'years' ? years : years * 12 + end.getMonth() - cursor.getMonth();
        while (count > 0 && advance(cursor, unit, count) > end) count--;
      } else {
        count = Math.floor((end - cursor) / (sizes[unit] * 1000));
      }

      const next = advance(cursor, unit, count);
      let value = count;
      if (fmt.decimal && index === chosen.length - 1 && end > next) {
        value += (end - next) / (advance(cursor, unit, count + 1) - next);
      }
      cursor = next;
      return { value, unit: Math.round(value * 10) === 10 ? unit.slice(0, -1) : unit };
    });
  }

  function remaining(target, fmt, now, locale) {
    return parts(target, fmt, now).map(p => `${formatNumber(p.value, locale)} ${p.unit}`).join(', ') + ' left';
  }

  const api = { parts, remaining, formatNumber };
  globalThis.countdownFormat = api;
  if (typeof module === 'object' && module.exports) module.exports = api;
})();
