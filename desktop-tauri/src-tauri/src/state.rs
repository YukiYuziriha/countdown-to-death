use serde_json::Value;
use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::NamedTempFile;

pub struct Store {
    path: PathBuf,
    pub value: Option<Arc<Value>>,
    exists: bool,
    blocked: Option<String>,
}

impl Store {
    pub fn load(path: PathBuf, legacy: Result<[PathBuf; 2], String>) -> Self {
        let mut store = Self {
            path,
            value: None,
            exists: false,
            blocked: None,
        };
        match read(&store.path) {
            Ok(Some(value)) => {
                store.exists = true;
                store.value = Some(Arc::new(value));
            }
            Err(error) => store.blocked = Some(error),
            Ok(None) => {
                let result = legacy.and_then(|paths| {
                    for path in paths {
                        if let Some(value) = read(&path)? {
                            return Ok(Some(value));
                        }
                    }
                    Ok(None)
                });
                match result {
                    Ok(Some(value)) => {
                        if let Err(error) = store.save(value.clone()) {
                            eprintln!("Could not copy legacy countdown state: {error}");
                            store.value = Some(Arc::new(value));
                        }
                    }
                    Ok(None) => {}
                    Err(error) => store.blocked = Some(error),
                }
            }
        }
        if let Some(error) = &store.blocked {
            eprintln!("Countdown state is unavailable; saving is disabled: {error}");
        }
        store
    }

    pub fn autorun(&self) -> Option<bool> {
        self.blocked.is_none().then(|| {
            self.value
                .as_deref()
                .and_then(|v| v.get("autorun"))
                .and_then(Value::as_bool)
                != Some(false)
        })
    }

    pub fn save(&mut self, value: Value) -> Result<(), String> {
        if let Some(error) = &self.blocked {
            return Err(format!(
                "Saving is disabled because existing state could not be loaded: {error}"
            ));
        }
        validate(&value).map_err(|e| e.to_string())?;
        write_atomic(&self.path, self.exists, |file| {
            serde_json::to_writer(file, &value).map_err(io::Error::other)
        })
        .map_err(|e| format!("Could not write {}: {e}", self.path.display()))?;
        self.exists = true;
        self.value = Some(Arc::new(value));
        Ok(())
    }
}

fn read(path: &Path) -> Result<Option<Value>, String> {
    let result = (|| {
        // A dangling link is not an absent state file.
        match fs::symlink_metadata(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            other => other?,
        };
        let value = serde_json::from_reader(io::BufReader::new(File::open(path)?))?;
        validate(&value)?;
        Ok::<_, io::Error>(Some(value))
    })();
    result.map_err(|e| format!("{}: {e}", path.display()))
}

fn validate(state: &Value) -> io::Result<()> {
    let valid = state.as_object().is_some_and(|state| {
        state.get("autorun").is_none_or(Value::is_boolean)
            && state
                .get("profiles")
                .and_then(Value::as_array)
                .is_some_and(|profiles| {
                    profiles.iter().all(|p| {
                        p.get("reminders")
                            .and_then(Value::as_array)
                            .is_some_and(|reminders| reminders.iter().all(Value::is_object))
                    })
                })
    });
    if valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid countdown state",
        ))
    }
}

pub fn write_atomic(
    path: &Path,
    replace: bool,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut temp = NamedTempFile::new_in(parent)?;
    write(temp.as_file_mut())?;
    temp.as_file().sync_all()?;
    if replace {
        temp.persist(path)?;
    } else {
        temp.persist_noclobber(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn paths(dir: &Path) -> (PathBuf, [PathBuf; 2]) {
        (
            dir.join("app/state.json"),
            [dir.join("old/state.json"), dir.join("fallback/state.json")],
        )
    }

    #[test]
    fn migrates_fallback_and_never_changes_legacy() {
        let dir = tempfile::tempdir().unwrap();
        let (path, legacy) = paths(dir.path());
        let old = br#" { "profiles": [], "autorun": false } "#;
        write_atomic(&legacy[1], false, |file| file.write_all(old)).unwrap();
        let mut store = Store::load(path.clone(), Ok(legacy.clone()));
        assert_eq!(
            read(&path).unwrap(),
            Some(json!({"profiles": [], "autorun": false}))
        );
        assert_eq!(store.autorun(), Some(false));
        let next = json!({"profiles": [], "autorun": true});
        store.save(next.clone()).unwrap();
        assert_eq!(read(&path).unwrap(), Some(next));
        assert_eq!(fs::read(&legacy[1]).unwrap(), old);
    }

    #[test]
    fn invalid_native_state_blocks_saving_and_migration() {
        let dir = tempfile::tempdir().unwrap();
        let (path, legacy) = paths(dir.path());
        write_atomic(&path, false, |file| file.write_all(b"broken JSON")).unwrap();
        write_atomic(&legacy[0], false, |file| {
            file.write_all(br#"{"profiles":[]}"#)
        })
        .unwrap();
        let mut store = Store::load(path.clone(), Ok(legacy));
        assert!(store.value.is_none());
        assert_eq!(store.autorun(), None);
        assert!(store.save(json!({"profiles": []})).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"broken JSON");
    }

    #[test]
    fn invalid_legacy_state_blocks_saving() {
        let dir = tempfile::tempdir().unwrap();
        let (path, legacy) = paths(dir.path());
        write_atomic(&legacy[0], false, |file| file.write_all(b"{}")).unwrap();
        let mut store = Store::load(path.clone(), Ok(legacy));
        assert!(store.save(json!({"profiles": []})).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn empty_state_defaults_to_autorun_but_first_save_does_not_clobber() {
        let dir = tempfile::tempdir().unwrap();
        let (path, legacy) = paths(dir.path());
        let mut store = Store::load(path.clone(), Ok(legacy));
        assert!(store.value.is_none());
        assert_eq!(store.autorun(), Some(true));
        write_atomic(&path, false, |file| {
            file.write_all(br#"{"profiles":[],"autorun":false}"#)
        })
        .unwrap();
        assert!(store.save(json!({"profiles": []})).is_err());
        assert!(store.value.is_none());
        assert_eq!(read(&path).unwrap().unwrap()["autorun"], false);
    }
}
