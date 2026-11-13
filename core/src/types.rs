//! Data contracts shared with the frontend (camelCase JSON).
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Error,
    Warn,
    Info,
    Debug,
    Unknown,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "ERROR",
            Severity::Warn => "WARN",
            Severity::Info => "INFO",
            Severity::Debug => "DEBUG",
            Severity::Unknown => "UNKNOWN",
        }
    }

    /// Map a (case-insensitive, ASCII) level word to a severity. Unrecognized -> Unknown.
    pub fn from_word(w: &str) -> Severity {
        const MAP: [(&str, Severity); 9] = [
            ("error", Severity::Error),
            ("err", Severity::Error),
            ("fatal", Severity::Error),
            ("critical", Severity::Error),
            ("warn", Severity::Warn),
            ("warning", Severity::Warn),
            ("info", Severity::Info),
            ("debug", Severity::Debug),
            ("trace", Severity::Debug),
        ];
        MAP.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(w))
            .map(|(_, s)| *s)
            .unwrap_or(Severity::Unknown)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeverityCounts {
    #[serde(rename = "ERROR")]
    pub error: u64,
    #[serde(rename = "WARN")]
    pub warn: u64,
    #[serde(rename = "INFO")]
    pub info: u64,
    #[serde(rename = "DEBUG")]
    pub debug: u64,
    #[serde(rename = "UNKNOWN")]
    pub unknown: u64,
}

impl SeverityCounts {
    pub fn add(&mut self, s: Severity) {
        match s {
            Severity::Error => self.error += 1,
            Severity::Warn => self.warn += 1,
            Severity::Info => self.info += 1,
            Severity::Debug => self.debug += 1,
            Severity::Unknown => self.unknown += 1,
        }
    }
    pub fn total(&self) -> u64 {
        self.error + self.warn + self.info + self.debug + self.unknown
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub line_number: u64,
    pub timestamp: Option<String>,
    pub severity: Severity,
    pub message: String,
    /// Line exceeded the 64 KiB cap and was cut.
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorGroup {
    pub id: String,
    pub signature: String,
    pub count: u64,
    pub sample_line_numbers: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub total_lines: u64,
    pub severity_counts: SeverityCounts,
    pub top_errors: Vec<ErrorGroup>,
    pub elapsed_ms: u64,
    pub grouping_truncated: bool,
    /// Distinct retained error signatures (capped at 10,000).
    pub distinct_groups: u64,
    pub bytes: u64,
    pub truncated_lines: u64,
    pub invalid_utf8_lines: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub entries: Vec<LogEntry>,
    pub total_matches: u64,
    pub offset: u64,
    pub limit: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub line_number: u64,
    pub explanation: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    pub summary: String,
    pub likely_causes: Vec<String>,
    pub evidence: Vec<Evidence>,
    pub next_steps: Vec<String>,
    pub uncertainty: String,
}
