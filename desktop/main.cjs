'use strict';

const { app, BrowserWindow, dialog, ipcMain, Menu, Notification, powerMonitor, Tray } = require('electron');
const path = require('node:path');
const { pathToFileURL, fileURLToPath } = require('node:url');
const manifest = require('./package.json');
const { LOGIN_ARG, setAutorun } = require('./autorun.cjs');
const { createScheduler, HEARTBEAT_MS } = require('./scheduler.cjs');
const { loadShared } = require('./shared.cjs');
const { loadState, saveState } = require('./state.cjs');

const name = manifest.productName;
const appId = manifest.desktopAppId;
const shared = app.isPackaged ? path.join(process.resourcesPath, 'shared') : path.resolve(__dirname, '..');
const page = path.join(shared, 'ui-prototype.html');
const icon = path.join(__dirname, 'icons/app.png');
const notifications = new Set();
let window;
let tray;
let timer;
let quitting = false;
let showOnReady = !process.argv.includes(LOGIN_ARG);
let state;
let stateFile;
let scheduler;
let remaining;

app.setName(name);
if (process.platform === 'win32') app.setAppUserModelId(appId);

function showWindow() {
  showOnReady = true;
  if (!window || quitting) return;
  if (window.isMinimized()) window.restore();
  window.show();
  window.focus();
}

function updateAutorun() {
  try {
    setAutorun(app, appId, state?.autorun !== false);
  } catch (error) {
    console.warn('Could not update launch at login:', error);
  }
}

function notify({ profile, reminder, at }) {
  if (!Notification.isSupported()) return;
  try {
    const body = reminder
      ? remaining?.(profile.target, reminder.format ?? profile.format, new Date(at)) ?? 'Countdown reminder.'
      : 'Time’s up. Countdown complete.';
    const notification = new Notification({ title: profile.name || name, body, icon });
    notifications.add(notification);
    const expiry = setTimeout(() => notifications.delete(notification), 300_000);
    expiry.unref();
    const release = () => {
      clearTimeout(expiry);
      notifications.delete(notification);
    };
    notification.once('click', () => { showWindow(); release(); });
    notification.once('close', release);
    notification.once('failed', (_event, error) => {
      console.warn('Desktop notification failed:', error);
      release();
    });
    notification.show();
  } catch (error) {
    console.warn('Could not show countdown notification:', error);
  }
}

function tick() {
  clearTimeout(timer);
  if (quitting) return;
  const { due, nextAt } = scheduler.scan(state?.profiles ?? [], Date.now());
  for (const occurrence of due) notify(occurrence);
  timer = setTimeout(tick, Math.min(HEARTBEAT_MS, Math.max(0, nextAt - Date.now())));
}

function handle(channel, action) {
  ipcMain.handle(`countdown:${channel}`, (event, ...args) => {
    const frame = event.senderFrame;
    if (!window || event.sender !== window.webContents || !frame || frame.parent ||
        !frame.url.startsWith('file:') || fileURLToPath(frame.url) !== page) {
      throw new Error('Untrusted countdown renderer');
    }
    return action(...args);
  });
}

function stop() {
  if (quitting) return;
  quitting = true;
  clearTimeout(timer);
  powerMonitor.removeListener('resume', tick);
  tray?.destroy();
}

async function start() {
  const helpers = loadShared(shared, { allowMissingFormat: !app.isPackaged });
  remaining = helpers.remaining;
  scheduler = createScheduler(helpers.getNext);
  stateFile = path.join(app.getPath('userData'), 'state.json');
  state = loadState(stateFile);
  updateAutorun();

  handle('get-state', () => state);
  handle('save-state', next => {
    const wasEnabled = state?.autorun !== false;
    state = saveState(stateFile, next);
    if (wasEnabled !== (state.autorun !== false)) updateAutorun();
    tick();
  });
  // Electron notifications are best-effort and require this process to be running.
  handle('get-status', () => ({ notifications: Notification.isSupported(), exactAlarms: false }));

  const menu = [
    { label: 'Show', click: showWindow },
    { label: 'Quit', accelerator: 'Ctrl+Q', click: () => app.quit() },
  ];
  Menu.setApplicationMenu(Menu.buildFromTemplate([{ label: name, submenu: menu }]));
  try {
    tray = new Tray(icon);
    tray.setToolTip(name);
    tray.setContextMenu(Menu.buildFromTemplate(menu));
    tray.on('click', showWindow);
  } catch (error) {
    tray?.destroy();
    tray = undefined;
    console.warn('System tray unavailable; closing the window will quit:', error);
  }

  window = new BrowserWindow({
    title: name, icon, width: 1160, height: 820, minWidth: 480, minHeight: 450,
    show: false, backgroundColor: '#161815',
    webPreferences: {
      preload: path.join(__dirname, 'preload.cjs'),
      sandbox: true, contextIsolation: true, nodeIntegration: false, devTools: !app.isPackaged,
    },
  });
  window.on('ready-to-show', () => { if (showOnReady || !tray) showWindow(); });
  window.on('page-title-updated', event => event.preventDefault());
  window.on('close', event => {
    if (quitting || !tray) return;
    event.preventDefault();
    window.hide();
  });
  window.on('closed', () => { window = undefined; });
  window.on('session-end', stop);
  window.webContents.on('will-navigate', event => event.preventDefault());
  window.webContents.on('will-redirect', event => event.preventDefault());
  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  window.webContents.session.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
  window.webContents.session.setPermissionCheckHandler(() => false);
  powerMonitor.on('resume', tick);
  tick();
  await window.loadURL(pathToFileURL(page).href);
}

if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  // Some Linux desktops don't render tray items even when Electron can create one.
  // Opening the launcher again always restores the existing window.
  app.on('second-instance', showWindow);
  app.on('activate', showWindow);
  app.on('before-quit', stop);
  app.on('window-all-closed', () => app.quit());
  app.whenReady().then(start).catch(error => {
    dialog.showErrorBox(`Could not start ${name}`, error.message);
    app.quit();
  });
}
