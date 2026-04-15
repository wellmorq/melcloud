use melcloud_core::MelcloudError;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn read_text_with_backup(path: &Path) -> Result<Option<String>, MelcloudError> {
    match fs::read_to_string(path) {
        Ok(raw) => Ok(Some(raw)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => read_backup_text(path),
        Err(err) => Err(err.into()),
    }
}

pub(crate) fn read_backup_text(path: &Path) -> Result<Option<String>, MelcloudError> {
    match fs::read_to_string(backup_path(path)) {
        Ok(raw) => Ok(Some(raw)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub(crate) fn write_with_backup(path: &Path, bytes: &[u8]) -> Result<(), MelcloudError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp = temp_path(path);
    let backup = backup_path(path);
    fs::write(&temp, bytes)?;

    if path.exists() {
        fs::copy(path, &backup)?;
        fs::remove_file(path)?;
    }

    if let Err(err) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        if backup.exists() && !path.exists() {
            let _ = fs::copy(&backup, path);
        }
        return Err(err.into());
    }

    Ok(())
}

pub(crate) fn backup_path(path: &Path) -> PathBuf {
    append_suffix(path, ".bak")
}

fn temp_path(path: &Path) -> PathBuf {
    append_suffix(path, ".tmp")
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut raw: OsString = path.as_os_str().to_owned();
    raw.push(suffix);
    PathBuf::from(raw)
}
