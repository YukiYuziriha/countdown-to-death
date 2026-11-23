'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const { desktopEntry } = require('../autorun.cjs');
const { loadState, saveState } = require('../state.cjs');

test('writes an XDG Exec line for a stable AppImage path with spaces and field codes', () => {
  const entry = desktopEntry('Countdown to Death', '/home/user/My Apps/$100% "rock`.AppImage');
  assert.ok(entry.includes(String.raw`Exec="/home/user/My Apps/\\$100%% \\"rock\\` + '`' + '.AppImage" --hidden\n'));
  assert.throws(() => desktopEntry('Countdown to Death', 'relative.AppImage'));
  assert.throws(() => desktopEntry('Countdown to Death', '/tmp/bad\npath.AppImage'));
});

test('persists state across loads and does not overwrite it with invalid state', t => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'countdown-state-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const file = path.join(dir, 'nested', 'state.json');
  assert.equal(loadState(file), null);
  const state = { autorun: false, theme: 'dark', profiles: [{ id: 'one', reminders: [] }] };
  saveState(file, state);
  assert.deepEqual(loadState(file), state);
  assert.throws(() => saveState(file, { profiles: [{ reminders: [null] }] }));
  assert.deepEqual(loadState(file), state);
  const updated = { ...state, theme: 'light' };
  saveState(file, updated);
  assert.deepEqual(loadState(file), updated);
  assert.deepEqual(fs.readdirSync(path.dirname(file)), ['state.json']);
});
