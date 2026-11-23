import assert from 'node:assert/strict';
import { test } from 'node:test';
import '../countdown-format.js';

// Load the existing classic script against a browser-like window.
const schedulerWindow = {};
const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
Object.defineProperty(globalThis, 'window', { configurable: true, value: schedulerWindow });
try {
  await import('../reminder-schedule.js');
} finally {
  if (originalWindow) Object.defineProperty(globalThis, 'window', originalWindow);
  else delete globalThis.window;
}
const { getNextCountdownReminder: next } = schedulerWindow;

const date = local => new Date(local);
const profile = (target, ...reminders) => ({ target, reminders, notifyAtTarget: true });
const once = (at, enabled = true) => ({ repeat: 'once', once: at, enabled });

test('selects the earliest enabled future reminder and formats as of its firing time', () => {
  const format = { units: ['hours', 'minutes'], decimal: false };
  const daily = { repeat: 'daily', time: '09:30', enabled: true, format };
  const p = profile('2025-05-10T12:00', once('2025-05-10T10:00'), once('2025-05-10T09:00', false), daily);
  const now = date('2025-05-10T08:00');
  const occurrence = next(p, now);
  assert.deepEqual(occurrence, { at: date('2025-05-10T09:30'), kind: 'scheduled', reminder: daily });
  assert.equal(globalThis.countdownFormat.remaining(p.target, occurrence.reminder.format, occurrence.at, 'en-US'), '2 hours, 30 minutes left');
});

test('clamps monthly reminders and skips an occurrence exactly at now', () => {
  const monthly = { repeat: 'monthly', day: '31', time: '09:00', enabled: true };
  const p = profile('2024-04-01T00:00', monthly);
  assert.deepEqual(next(p, date('2024-02-01T09:00')).at, date('2024-02-29T09:00'));
  assert.deepEqual(next(p, date('2024-02-29T09:00')).at, date('2024-03-31T09:00'));
});

test('excludes reminders at or beyond the target and stops after completion', () => {
  const target = '2025-05-10T12:00';
  const p = profile(target, once(target), once('2025-05-10T13:00'));
  const before = date('2025-05-10T11:00');
  assert.deepEqual(next(p, before), { at: date(target), kind: 'target', reminder: null });
  assert.equal(next({ ...p, notifyAtTarget: false }, before), null);
  assert.equal(next(p, date(target)), null);
  assert.equal(next(p, date('2025-05-10T13:00')), null);
});
