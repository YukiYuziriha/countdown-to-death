# Countdown to Death desktop

With Node.js 24, run `npm ci`, `npm test`, then `npm start` from `desktop/`.
`npm run dist -- --win nsis --x64` on Windows or `npm run dist -- --linux AppImage --x64` on Linux builds into `desktop/dist/`. The GitHub Actions workflow uploads the installers without publishing a release. Builds use the shared files from the parent directory; missing required files (including `app-host.js`) fail the build.

Closing the window hides it while reminders run if the tray is available. Use **Show** or **Quit** from the tray menu. If your Linux desktop does not display the tray icon, launch the app again to restore the existing window, then use **Countdown to Death → Quit** (or `Ctrl+Q`) to exit. If Electron cannot create the tray at all, closing the window quits instead.

Packaged apps register to launch at login by default, unless disabled in settings. On Linux, keep the AppImage at the registered location; if moved, launch it again to update its path. Notifications depend on the desktop notification service and require the app to be running. After sleep, already-armed events up to one minute late may be delivered; older events and anything missed while the app was quit are not replayed.
