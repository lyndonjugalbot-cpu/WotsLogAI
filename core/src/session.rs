//! Single active file session. Only native-selected paths and bundled fixtures are ever opened.
use crate::error::{AppError, Result};
use crate::scan::check_size;
use crate::types::Summary;
use serde::Serialize;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Demo,
    Local,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub path: PathBuf,
    pub display_name: String,
    pub source: Source,
    size: u64,
    mtime: SystemTime,
    pub summary: Option<Summary>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub is_demo: bool,
}

impl Session {
    pub fn open(path: PathBuf, source: Source) -> Result<Session> {
        let meta = std::fs::metadata(&path)?;
        if !meta.is_file() {
            return Err(AppError::NotAFile);
        }
        check_size(meta.len())?;
        let display_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "log".into());
        Ok(Session {
            id: uuid::Uuid::new_v4().to_string(),
            path,
            display_name,
            source,
            size: meta.len(),
            mtime: meta.modified().map_err(AppError::from)?,
            summary: None,
        })
    }

    /// Fail if the file's size or mtime differs from when it was opened.
    pub fn verify_unchanged(&self) -> Result<()> {
        let meta = std::fs::metadata(&self.path)?;
        if meta.len() != self.size || meta.modified().map_err(AppError::from)? != self.mtime {
            return Err(AppError::FileChanged);
        }
        Ok(())
    }

    pub fn info(&self) -> SessionInfo {
        SessionInfo {
            session_id: self.id.clone(),
            display_name: self.display_name.clone(),
            size_bytes: self.size,
            is_demo: self.source == Source::Demo,
        }
    }
}
