use serde::{Serialize, Serializer, ser::SerializeStruct};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use wotslog_core::AppError;
use wotslog_core::approval::ApprovalStore;
use wotslog_core::credentials::CredentialStore;
use wotslog_core::session::Session;

/// Command error: typed code + message, and whether one controlled retry is still possible.
#[derive(Debug)]
pub struct CmdError {
    pub err: AppError,
    pub retry_allowed: bool,
}

impl From<AppError> for CmdError {
    fn from(err: AppError) -> Self {
        Self {
            err,
            retry_allowed: false,
        }
    }
}

impl Serialize for CmdError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("CmdError", 3)?;
        st.serialize_field("code", self.err.code())?;
        st.serialize_field("message", &self.err.to_string())?;
        st.serialize_field("retryAllowed", &self.retry_allowed)?;
        st.end()
    }
}

pub type CmdResult<T> = Result<T, CmdError>;

pub struct AppState {
    pub session: Mutex<Option<Session>>,
    pub scans: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub approvals: Mutex<ApprovalStore>,
    pub settings_path: PathBuf,
    pub creds: Arc<dyn CredentialStore>,
}

impl AppState {
    pub fn new(settings_path: PathBuf, creds: Arc<dyn CredentialStore>) -> Self {
        Self {
            session: Mutex::new(None),
            scans: Mutex::new(HashMap::new()),
            approvals: Mutex::new(ApprovalStore::default()),
            settings_path,
            creds,
        }
    }

    /// Replace the active session; cancels running scans and drops pending approvals.
    pub fn install_session(&self, s: Session) {
        for f in self.scans.lock().unwrap().values() {
            f.store(true, Ordering::Relaxed);
        }
        self.approvals.lock().unwrap().clear();
        *self.session.lock().unwrap() = Some(s);
    }

    /// Current session if it matches `id` and the file is unchanged on disk.
    pub fn current(&self, id: &str) -> Result<Session, AppError> {
        let g = self.session.lock().unwrap();
        let s = g.as_ref().ok_or(AppError::NoSession)?;
        if s.id != id {
            return Err(AppError::StaleSession);
        }
        s.verify_unchanged()?;
        Ok(s.clone())
    }

    /// Register a cancellable scan; a previous scan with the same id is cancelled.
    pub fn register_scan(&self, scan_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .scans
            .lock()
            .unwrap()
            .insert(scan_id.to_string(), flag.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        flag
    }

    pub fn finish_scan(&self, scan_id: &str, flag: &Arc<AtomicBool>) {
        let mut g = self.scans.lock().unwrap();
        if g.get(scan_id).is_some_and(|f| Arc::ptr_eq(f, flag)) {
            g.remove(scan_id);
        }
    }

    pub fn cancel_scan(&self, scan_id: &str) {
        if let Some(f) = self.scans.lock().unwrap().get(scan_id) {
            f.store(true, Ordering::Relaxed);
        }
    }
}
