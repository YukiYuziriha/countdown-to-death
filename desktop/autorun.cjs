'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const LOGIN_ARG = '--hidden';

function desktopEntry(name, executable) {
  if (!path.posix.isAbsolute(executable) || /[\u0000-\u001f\u007f]/.test(executable)) {
    throw new Error('Autostart requires an absolute executable path with no control characters');
  }
  // Escape Exec quoting and field codes first, then Desktop Entry string escaping.
  const command = executable.replace(/%/g, '%%').replace(/["\\$`]/g, '\\$&').replace(/\\/g, '\\\\');
  return `[Desktop Entry]\nType=Application\nName=${name}\nExec="${command}" ${LOGIN_ARG}\nTerminal=false\n`;
}

function setAutorun(app, appId, enabled) {
  // The development Electron binary cannot relaunch this app without a project argument.
  if (!app.isPackaged) return;
  if (process.platform === 'win32') {
    app.setLoginItemSettings({ openAtLogin: enabled, name: appId,
      path: process.execPath, args: [LOGIN_ARG] });
  } else if (process.platform === 'linux') {
    const config = path.isAbsolute(process.env.XDG_CONFIG_HOME ?? '')
      ? process.env.XDG_CONFIG_HOME : path.join(os.homedir(), '.config');
    const directory = path.join(config, 'autostart');
    const file = path.join(directory, `${appId}.desktop`);
    if (enabled) {
      // APPIMAGE is the stable original path, not the executable in the temporary mount.
      const entry = desktopEntry(app.getName(), process.env.APPIMAGE || process.execPath);
      fs.mkdirSync(directory, { recursive: true });
      const temp = `${file}.tmp`;
      fs.writeFileSync(temp, entry, { mode: 0o644 });
      fs.renameSync(temp, file);
    } else {
      fs.rmSync(file, { force: true });
    }
  }
}

module.exports = { desktopEntry, setAutorun, LOGIN_ARG };
