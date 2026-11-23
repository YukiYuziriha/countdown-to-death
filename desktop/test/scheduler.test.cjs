'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');
const { createScheduler } = require('../scheduler.cjs');
const { loadShared } = require('../shared.cjs');

const { getNext, remaining } = loadShared(path.resolve(__dirname, '../..'));
const at = (day, time = '09:00') => new Date(`2026-12-${day}T${time}`).getTime();
const daily = { id: 'daily', enabled: true, repeat: 'daily', time: '09:00' };
const once = { id: 'once', enabled: true, repeat: 'once', once: '2026-12-02T09:00' };
const profile = { id: 'one', name: 'A countdown', target: '2026-12-04T09:00',
  notifyAtTarget: true, reminders: [daily, once, { ...daily, id: 'paused', enabled: false }] };

function create() {
  const scheduler = createScheduler(getNext);
  return (now, profiles = [profile], zone = 'test-zone') => scheduler.scan(profiles, now, zone);
}

const ids = ({ due }) => due.map(({ reminder }) => reminder?.id ?? 'target');

test('enables each reminder separately, including simultaneous events across profiles', () => {
  const scan = create();
  const profiles = [profile, { ...profile, id: 'two', reminders: [{ ...daily, id: 'other' }] }];
  assert.equal(scan(at('02') - 1, profiles).nextAt, at('02'));
  assert.deepEqual(ids(scan(at('02'), profiles)), ['daily', 'once', 'other']);
  assert.deepEqual(ids(scan(at('02') + 1, profiles)), []);
  assert.deepEqual(ids(scan(at('03'), profiles)), ['daily', 'other']);
  const target = scan(at('04'), profiles);
  assert.deepEqual(ids(target), ['target', 'target']);
  assert.equal(target.nextAt, Infinity);
});

test('does not replay startup backlog or probe reminders at/after the target', () => {
  const scan = create();
  assert.deepEqual(ids(scan(at('02') + 1)), []);
  assert.equal(scan(at('02') + 1).nextAt, at('03'));
  const ended = create()(at('04') + 1);
  assert.deepEqual(ids(ended), []);
  assert.equal(ended.nextAt, Infinity);
  const off = { ...profile, notifyAtTarget: false, target: '2026-12-02T09:00' };
  assert.equal(create()(at('02') - 1, [off]).nextAt, Infinity);
});

test('fires a recently due alert after resume but drops stale occurrences after long sleep', () => {
  const scan = create();
  scan(at('02') - 1);
  assert.deepEqual(ids(scan(at('02') + 30_000)), ['daily', 'once']);
  const resumed = scan(at('03') + 90_000);
  assert.deepEqual(ids(resumed), []);
  assert.equal(resumed.nextAt, at('04'));
  assert.deepEqual(ids(scan(at('04'))), ['target']);
});

test('replans after a clock rewind without duplicating delivered occurrences', () => {
  const scan = create();
  scan(at('02') - 10_000);
  assert.deepEqual(ids(scan(at('02'))), ['daily', 'once']);
  assert.equal(scan(at('02') - 5_000).nextAt, at('03'));
  assert.deepEqual(ids(scan(at('02'))), []);
  assert.deepEqual(ids(scan(at('03'))), ['daily']);
});

test('does not replay a delivered reminder paused and re-enabled after a rewind', () => {
  const scan = create();
  scan(at('02') - 10_000);
  assert.deepEqual(ids(scan(at('02'))), ['daily', 'once']);
  scan(at('02') + 1, [{ ...profile, reminders: [] }]);
  assert.equal(scan(at('02') - 5_000).nextAt, at('03'));
  assert.deepEqual(ids(scan(at('02'))), []);
});

test('invalidates old wall-clock plans when the time zone changes', () => {
  const scan = create();
  scan(at('02') - 1);
  assert.deepEqual(ids(scan(at('02'), [profile], 'new-zone')), []);
  assert.equal(scan(at('02'), [profile], 'new-zone').nextAt, at('03'));
});

test('state saves cancel disabled or edited reminders and use the latest message data', () => {
  const scan = create();
  scan(at('02') - 1);
  const edited = { ...profile, name: 'Renamed', reminders: [
    { ...daily, time: '10:00' }, { ...once, name: 'New label' },
  ] };
  const saved = scan(at('02'), [edited]);
  assert.deepEqual(ids(saved), ['once']);
  assert.equal(saved.due[0].profile.name, 'Renamed');
  assert.equal(saved.due[0].reminder.name, 'New label');
  assert.equal(saved.nextAt, at('02', '10:00'));
  assert.deepEqual(ids(scan(at('02', '10:00'), [{ ...edited, reminders: [] }])), []);
});

test('uses the shared formatter at the planned timestamp', () => {
  assert.equal(remaining(profile.target, { units: ['days'], decimal: false }, new Date(at('02')), 'en'), '2 days left');
});
