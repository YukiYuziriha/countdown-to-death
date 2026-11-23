'use strict';

const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

function loadShared(directory, { allowMissingFormat = false } = {}) {
  const context = vm.createContext({});
  context.window = context;
  const run = name => {
    const filename = path.join(directory, name);
    vm.runInContext(fs.readFileSync(filename, 'utf8'), context, { filename });
  };
  run('reminder-schedule.js');
  const hasFormat = !allowMissingFormat || fs.existsSync(path.join(directory, 'countdown-format.js'));
  if (hasFormat) run('countdown-format.js');
  const getNext = context.getNextCountdownReminder;
  const remaining = context.countdownFormat?.remaining;
  if (typeof getNext !== 'function' || (hasFormat && typeof remaining !== 'function')) {
    throw new Error('Shared countdown scripts did not expose the expected functions');
  }
  return { getNext, remaining };
}

module.exports = { loadShared };
