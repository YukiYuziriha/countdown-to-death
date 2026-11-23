import assert from 'node:assert/strict';
import { test } from 'node:test';
import '../countdown-format.js';

const { parts, remaining, formatNumber } = globalThis.countdownFormat;
const format = (...units) => ({ units, decimal: false });

test('clamps calendar months at month end before counting the remainder', () => {
  assert.deepEqual(parts('2023-03-01T12:00', format('months', 'days'), new Date('2023-01-31T12:00')), [
    { value: 1, unit: 'month' },
    { value: 1, unit: 'day' },
  ]);
  assert.deepEqual(parts('2024-02-29T11:59', format('months'), new Date('2024-01-31T12:00')), [
    { value: 0, unit: 'months' },
  ]);
});

test('clamps leap-day years and consumes combined units in order', () => {
  const target = new Date(+new Date('2025-03-28T12:00') + 27.5 * 3600000);
  assert.deepEqual(parts(target, format('years', 'months', 'days', 'hours', 'minutes'), new Date('2024-02-29T12:00')), [
    { value: 1, unit: 'year' },
    { value: 1, unit: 'month' },
    { value: 1, unit: 'day' },
    { value: 3, unit: 'hours' },
    { value: 30, unit: 'minutes' },
  ]);
});

test('uses local wall time for months but elapsed time for days and hours', () => {
  // Spring-forward dates in New York and historical Moscow, respectively.
  for (const [previousMonth, previousDay, transitionDay] of [
    ['2024-02-10', '2024-03-09', '2024-03-10'],
    ['2010-02-28', '2010-03-27', '2010-03-28'],
  ]) {
    const now = new Date(`${previousMonth}T02:30`);
    const target = new Date(`${transitionDay}T02:30`);
    assert.deepEqual(parts(target, format('months', 'hours'), now), [
      { value: 1, unit: 'month' },
      { value: 0, unit: 'hours' },
    ]);

    const before = new Date(`${previousDay}T12:00`);
    const after = new Date(`${transitionDay}T12:00`);
    const elapsedHours = (after - before) / 3600000;
    assert.deepEqual(parts(after, format('days', 'hours'), before).map(p => p.value), [
      Math.floor(elapsedHours / 24), Math.floor(elapsedHours % 24),
    ]);
  }
});

test('uses the next calendar boundary for the final fractional unit', () => {
  const now = new Date('2023-01-31T12:00');
  const first = new Date('2023-02-28T12:00');
  const second = new Date('2023-03-31T12:00');
  const halfway = new Date((+first + +second) / 2);
  assert.deepEqual(parts(halfway, { units: ['years', 'months'], decimal: true }, now), [
    { value: 0, unit: 'years' },
    { value: 1.5, unit: 'months' },
  ]);
});

test('localizes numbers but keeps English unit inflection tied to the rounded quantity', () => {
  const now = new Date('2025-01-01T12:00');
  const fmt = { units: ['minutes'], decimal: true };
  const arabic = 'ar-EG-u-nu-arab';
  assert.deepEqual(parts('2025-01-01T12:00:57', fmt, now), [{ value: 0.95, unit: 'minute' }]);
  assert.equal(remaining('2025-01-01T12:01:02', fmt, now, arabic), '١ minute left');
  assert.equal(remaining('2025-01-01T12:01:03', fmt, now, 'de-DE'), '1,1 minutes left');
  assert.equal(formatNumber(1234.56, 'de-DE'), '1.234,6');
});

test('formats relative to the supplied notification time and clamps completed countdowns to zero', () => {
  const target = '2025-05-10T12:00';
  const fmt = { units: ['hours', 'minutes'], decimal: true };
  assert.equal(remaining(target, fmt, new Date('2025-05-10T09:30'), 'en-US'), '2 hours, 30 minutes left');
  for (const now of [new Date(target), new Date('2025-05-10T13:00')]) {
    assert.deepEqual(parts(target, fmt, now), [{ value: 0, unit: 'hours' }, { value: 0, unit: 'minutes' }]);
    assert.equal(remaining(target, fmt, now, 'en-US'), '0 hours, 0 minutes left');
  }
});
