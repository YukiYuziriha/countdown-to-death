'use strict';

const fs = require('node:fs');
const path = require('node:path');

const isRecord = value => value !== null && typeof value === 'object' && !Array.isArray(value);

function validate(state) {
  if (!isRecord(state) || !Array.isArray(state.profiles) ||
    (state.autorun !== undefined && typeof state.autorun !== 'boolean') ||
    !state.profiles.every(profile => isRecord(profile) && Array.isArray(profile.reminders) &&
      profile.reminders.every(isRecord))) {
    throw new TypeError('Invalid countdown state');
  }
  return state;
}

function loadState(file) {
  try {
    return validate(JSON.parse(fs.readFileSync(file, 'utf8')));
  } catch (error) {
    if (error.code !== 'ENOENT') console.warn('Could not read countdown state:', error);
    return null;
  }
}

function saveState(file, state) {
  const json = JSON.stringify(state);
  const persisted = validate(JSON.parse(json));
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const temp = `${file}.tmp`;
  fs.writeFileSync(temp, json, { mode: 0o600 });
  fs.renameSync(temp, file);
  return persisted;
}

module.exports = { loadState, saveState };
