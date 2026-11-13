//! Non-secret provider settings, stored as JSON in the app config dir.
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettings {
    pub base_url: String,
    pub model: String,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4o-mini".into(),
        }
    }
}

impl ProviderSettings {
    /// Identity used to bind approvals to a provider destination.
    pub fn fingerprint(&self) -> String {
        format!("{}|{}", self.base_url.trim_end_matches('/'), self.model)
    }
}

pub fn load(path: &Path) -> Option<ProviderSettings> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

pub fn save(path: &Path, s: &ProviderSettings) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(s).map_err(|e| AppError::Io(e.to_string()))?,
    )?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
