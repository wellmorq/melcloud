use crate::error::{Result, SiteError};
use crate::file_store::{read_backup_text, read_text_with_backup, write_with_backup};
use crate::models::FixedPresetId;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct SiteStateFile {
    pub active_preset_id: Option<FixedPresetId>,
}

pub(crate) fn load_site_state(path: &Path) -> Result<SiteStateFile> {
    let Some(raw) = read_text_with_backup(path)? else {
        return Ok(SiteStateFile::default());
    };
    match serde_json::from_str(&raw) {
        Ok(state) => Ok(state),
        Err(primary_err) => {
            if let Some(backup_raw) = read_backup_text(path)? {
                if let Ok(state) = serde_json::from_str(&backup_raw) {
                    return Ok(state);
                }
            }
            Err(SiteError::Json(primary_err))
        }
    }
}

pub(crate) fn write_site_state(path: &Path, state: &SiteStateFile) -> Result<()> {
    let raw = serde_json::to_vec_pretty(state)?;
    write_with_backup(path, &raw)
}
