'use strict';

const { existsSync } = require('node:fs');
const path = require('node:path');
const { desktopAppId, productName } = require('./package.json');

const sharedFiles = [
  'ui-prototype.html', 'prototype.css', 'animated-texture.js', 'reminder-schedule.js',
  'countdown-format.js', 'app-host.js',
];

module.exports = {
  appId: desktopAppId,
  productName,
  artifactName: '${name}-${version}-${arch}.${ext}',
  files: ['main.cjs', 'preload.cjs', 'autorun.cjs', 'scheduler.cjs', 'shared.cjs', 'state.cjs',
    'icons/app.png', 'package.json'],
  extraResources: [{ from: '..', to: 'shared', filter: [...sharedFiles, 'assets/**/*'] }],
  beforePack() {
    // electron-builder silently ignores missing extraResources filter matches.
    const missing = [...sharedFiles, 'assets'].filter(file => !existsSync(path.join(__dirname, '..', file)));
    if (missing.length) throw new Error(`Missing shared desktop resources: ${missing.join(', ')}`);
  },
  win: { icon: 'icons/app.ico', target: 'nsis' },
  nsis: { uninstallDisplayName: productName },
  linux: { icon: 'icons/app.png', category: 'Utility', target: 'AppImage' },
};
