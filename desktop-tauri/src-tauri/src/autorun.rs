use std::io;

pub const LOGIN_ARG: &str = "--hidden";
const APP_ID: &str = "com.countdowntodeath.desktop";

pub fn update(app: &tauri::AppHandle, enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    // Development runs should not replace the installed app's login entry.
    if !cfg!(debug_assertions) {
        update_platform(app, enabled)?;
    }
    Ok(())
}

#[cfg(windows)]
fn update_platform(_: &tauri::AppHandle, enabled: bool) -> io::Result<()> {
    use winreg::{
        enums::{HKEY_CURRENT_USER, KEY_SET_VALUE},
        RegKey,
    };
    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if enabled {
        let executable = std::env::current_exe()?;
        let mut command = std::ffi::OsString::from("\"");
        command.push(executable);
        command.push("\" ");
        command.push(LOGIN_ARG);
        hkcu.create_subkey(RUN)?.0.set_value(APP_ID, &command)
    } else {
        ignore_missing(
            hkcu.open_subkey_with_flags(RUN, KEY_SET_VALUE)
                .and_then(|key| key.delete_value(APP_ID)),
        )
    }
}

#[cfg(target_os = "linux")]
fn update_platform(
    app: &tauri::AppHandle,
    enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::{io::Write, path::PathBuf};
    use tauri::Manager;
    let file = app
        .path()
        .config_dir()?
        .join("autostart")
        .join(format!("{APP_ID}.desktop"));
    if enabled {
        // The process executable in an AppImage mount is not stable across boots.
        let executable = match std::env::var_os("APPIMAGE").filter(|v| !v.is_empty()) {
            Some(path) => PathBuf::from(path),
            None => std::env::current_exe()?,
        };
        let entry = desktop_entry(&executable)?;
        crate::state::write_atomic(&file, true, |file| file.write_all(entry.as_bytes()))?;
    } else {
        ignore_missing(std::fs::remove_file(file))?;
    }
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux")))]
fn update_platform(_: &tauri::AppHandle, _: bool) -> io::Result<()> {
    Ok(())
}

#[cfg(any(windows, target_os = "linux"))]
fn ignore_missing(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(any(target_os = "linux", test))]
fn desktop_entry(path: &std::path::Path) -> io::Result<String> {
    let executable = path
        .to_str()
        .filter(|p| p.starts_with('/') && !p.chars().any(|c| c.is_ascii_control()))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Autostart requires an absolute UTF-8 executable path without control characters",
            )
        })?;
    // Exec quoting is processed after Desktop Entry string escapes.
    let mut command = String::new();
    for ch in executable.chars() {
        match ch {
            '%' => command.push_str("%%"),
            '\\' => command.push_str(r"\\\\"),
            '"' | '$' | '`' => {
                command.push_str(r"\\");
                command.push(ch);
            }
            _ => command.push(ch),
        }
    }
    Ok(format!("[Desktop Entry]\nType=Application\nName={}\nExec=\"{command}\" {LOGIN_ARG}\nTerminal=false\n", crate::PRODUCT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn desktop_entry_keeps_a_single_quoted_executable_and_hidden_arg() {
        let entry = desktop_entry(Path::new(r#"/home/me/a % "quote" \$`.AppImage"#)).unwrap();
        assert!(
            entry
                .lines()
                .any(|line| line
                    == r#"Exec="/home/me/a %% \\"quote\\" \\\\\\$\\`.AppImage" --hidden"#)
        );
        assert!(desktop_entry(Path::new("relative.AppImage")).is_err());
        assert!(desktop_entry(Path::new("/bad\npath.AppImage")).is_err());
    }
}
