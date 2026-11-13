//! Typed errors shared by core and the Tauri command layer.
use serde::{Serialize, Serializer, ser::SerializeStruct};

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum AppError {
    #[error("No log file is open.")]
    NoSession,
    #[error("This result belongs to a previous file or request and was discarded.")]
    StaleSession,
    #[error("The operation was cancelled.")]
    Cancelled,
    #[error("The file was not found.")]
    FileNotFound,
    #[error("Permission denied reading the file.")]
    PermissionDenied,
    #[error("Not a regular file.")]
    NotAFile,
    #[error("File is {size} bytes; the MVP limit is {limit} bytes.")]
    FileTooLarge { size: u64, limit: u64 },
    #[error("The file changed on disk since it was opened. Re-import it to continue.")]
    FileChanged,
    #[error("I/O error: {0}")]
    Io(String),
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    #[error("Error group not found in the current scan.")]
    GroupNotFound,
    #[error("Scan results are not available yet; run a scan first.")]
    NotScanned,
    #[error(
        "The approval is missing, expired, already used, or no longer matches the current file/provider."
    )]
    ApprovalInvalid,
    #[error("Live AI is disabled: {0}")]
    AiDisabled(String),
    #[error("AI provider is not configured.")]
    AiNotConfigured,
    #[error("Could not reach the AI provider (offline or blocked).")]
    AiOffline,
    #[error("The AI provider timed out.")]
    AiTimeout,
    #[error("Rate limited by the AI provider.")]
    AiRateLimited { retry_after_secs: Option<u64> },
    #[error("The AI provider rejected the credentials.")]
    AiUnauthorized,
    #[error("AI provider returned HTTP {0}.")]
    AiHttp(u16),
    #[error("The AI response was not valid: {0}")]
    AiInvalidResponse(String),
    #[error("Credential store error: {0}")]
    Credential(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        use AppError::*;
        match self {
            NoSession => "noSession",
            StaleSession => "staleSession",
            Cancelled => "cancelled",
            FileNotFound => "fileNotFound",
            PermissionDenied => "permissionDenied",
            NotAFile => "notAFile",
            FileTooLarge { .. } => "fileTooLarge",
            FileChanged => "fileChanged",
            Io(_) => "io",
            InvalidArgument(_) => "invalidArgument",
            GroupNotFound => "groupNotFound",
            NotScanned => "notScanned",
            ApprovalInvalid => "approvalInvalid",
            AiDisabled(_) => "aiDisabled",
            AiNotConfigured => "aiNotConfigured",
            AiOffline => "aiOffline",
            AiTimeout => "aiTimeout",
            AiRateLimited { .. } => "aiRateLimited",
            AiUnauthorized => "aiUnauthorized",
            AiHttp(_) => "aiHttp",
            AiInvalidResponse(_) => "aiInvalidResponse",
            Credential(_) => "credential",
        }
    }

    /// Failures that may succeed on a single controlled retry of the same approved request.
    pub fn is_retryable(&self) -> bool {
        use AppError::*;
        matches!(
            self,
            AiTimeout | AiOffline | AiRateLimited { .. } | AiInvalidResponse(_)
        ) || matches!(self, AiHttp(s) if *s >= 500)
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        use std::io::ErrorKind::*;
        match e.kind() {
            NotFound => AppError::FileNotFound,
            PermissionDenied => AppError::PermissionDenied,
            _ => AppError::Io(e.to_string()),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("AppError", 3)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.serialize_field("retryable", &self.is_retryable())?;
        st.end()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
