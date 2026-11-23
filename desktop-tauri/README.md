# Countdown to Death desktop

This Tauri v2 desktop app packages the same UI as the browser and Android builds. Windows uses an NSIS `.exe` installer; Linux uses an `.AppImage`. The native process schedules reminders without an open window.

## Build from source

Install Python 3, stable Rust, and the [Tauri v2 system prerequisites](https://v2.tauri.app/start/prerequisites/). On Windows, use the MSVC Rust toolchain with the Visual Studio C++ build tools and WebView2. On Debian/Ubuntu, the build needs WebKitGTK 4.1, Ayatana AppIndicator, and the other Linux dependencies listed in the Tauri prerequisites; running an AppImage may also require FUSE 2.

From this `desktop-tauri/` directory, stage the UI before compiling. The script refreshes `dist/` from the shared root files, including the native bridge, styles, scripts, and fonts. Rerun it after editing the shared UI. No Node.js frontend build is needed.

```sh
python scripts/stage-web.py
cargo test --locked --manifest-path src-tauri/Cargo.toml --features custom-protocol
```

To package the app, use the Tauri v2 CLI (`cargo install tauri-cli --version '^2.0.0' --locked` if it is not already installed), then run the command for your platform after staging:

```sh
cargo tauri build --bundles nsis      # Windows
cargo tauri build --bundles appimage  # Linux
```

Bundles are written under `src-tauri/target/release/bundle/` by default. Set `CARGO_BUILD_JOBS` to limit concurrent compiler jobs. The checked-in 256×256 PNG and Windows ICO in `src-tauri/icons/` provide the static pixel-skull app icon.

## Reminders and migration

- **Launch at login** is on by default. Installed/release builds use the existing `com.countdowntodeath.desktop` login entry and start with `--hidden`. Keep the AppImage at a stable path so the login entry can find it. Debug builds do not change the login entry.
- Closing the window leaves reminders running when the tray was created. Use tray **Show** or open the launcher again to restore the window. Use tray **Quit** or **Settings → Quit app** to stop reminders and exit. If the tray fails to initialize, closing the window exits instead. On Linux desktops that simply do not display tray items, reopen via the launcher and quit from Settings.
- On first launch, if Tauri has no state file, it copies valid Electron state from `Countdown to Death/state.json` under `%APPDATA%` on Windows or `$XDG_CONFIG_HOME` (default `~/.config`) on Linux. It also checks the lowercase legacy folder when the original name is absent. The copy is saved separately under `com.countdowntodeath.desktop/state.json` in the same config root; the original is not changed. An unreadable existing state file is not silently replaced. Quit the old Electron app before switching so only one desktop scheduler is running.
