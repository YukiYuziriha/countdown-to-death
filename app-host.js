(() => {
  'use strict';
  const android = globalThis.CountdownAndroid;
  const desktop = globalThis.countdownHost;
  const tauri = globalThis.__TAURI__?.core;
  if (android && typeof android.getState === 'function') {
    globalThis.countdownAppHost = {
      platform: 'android',
      async getState() { const saved = android.getState(); return saved ? JSON.parse(saved) : null; },
      async saveState(state) { android.saveState(JSON.stringify(state)); },
      async getStatus() { return JSON.parse(android.getStatus()); },
      requestNotifications() { android.requestNotifications(); },
      requestExactAlarms() { android.requestExactAlarms(); },
    };
  } else if (desktop && typeof desktop.getState === 'function') {
    globalThis.countdownAppHost = {
      platform: /Linux/i.test(navigator.userAgent) ? 'linux' : 'windows',
      getState: () => desktop.getState(),
      saveState: state => desktop.saveState(state),
      getStatus: () => desktop.getStatus(),
    };
  } else if (typeof tauri?.invoke === 'function') {
    globalThis.countdownAppHost = {
      platform: /Linux/i.test(navigator.userAgent) ? 'linux' : 'windows',
      getState: () => tauri.invoke('get_state'),
      saveState: state => tauri.invoke('save_state', { state }),
      getStatus: () => tauri.invoke('get_status'),
      quitApp: () => tauri.invoke('quit_app'),
    };
  }
})();
