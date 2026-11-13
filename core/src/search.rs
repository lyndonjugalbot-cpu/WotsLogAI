//! Streaming literal search + severity filter + pagination. Matching is ASCII case-insensitive
//! against the full raw line (what is on disk, capped at 64 KiB).
use crate::error::{AppError, Result};
use crate::linereader::read_line;
use crate::parser::parse_line;
use crate::scan::{check_size, open_reader};
use crate::types::{LogEntry, Page, Severity};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

pub const MAX_PAGE: u64 = 100;
const CANCEL_CHECK_EVERY: u64 = 2048;

#[derive(Debug, Clone, Default)]
pub struct QuerySpec {
    pub query: String,
    pub severity: Option<Severity>,
}

pub fn query_page(
    path: &Path,
    spec: &QuerySpec,
    offset: u64,
    limit: u64,
    cancel: &AtomicBool,
) -> Result<Page> {
    if limit == 0 || limit > MAX_PAGE {
        return Err(AppError::InvalidArgument(format!(
            "limit must be 1..={MAX_PAGE}"
        )));
    }
    let (mut rd, len) = open_reader(path)?;
    check_size(len)?;
    let needle = spec.query.to_ascii_lowercase().into_bytes();
    let finder = memchr::memmem::Finder::new(&needle);
    let mut scratch = Vec::new();
    let (mut buf, mut entries) = (Vec::with_capacity(4096), Vec::new());
    let (mut lines, mut matches) = (0u64, 0u64);
    while let Some(meta) = read_line(&mut rd, &mut buf)? {
        lines += 1;
        if lines.is_multiple_of(CANCEL_CHECK_EVERY) && cancel.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        if !needle.is_empty() {
            scratch.clear();
            scratch.extend_from_slice(&buf);
            scratch.make_ascii_lowercase();
            if finder.find(&scratch).is_none() {
                continue;
            }
        }
        let text = String::from_utf8_lossy(&buf);
        let p = parse_line(&text);
        if spec.severity.is_some_and(|s| s != p.severity) {
            continue;
        }
        if matches >= offset && (entries.len() as u64) < limit {
            entries.push(LogEntry {
                line_number: lines,
                timestamp: p.timestamp.map(|t| t.into_owned()),
                severity: p.severity,
                message: p.message.into_owned(),
                truncated: meta.truncated,
            });
        }
        matches += 1;
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::Cancelled);
    }
    Ok(Page {
        entries,
        total_matches: matches,
        offset,
        limit,
    })
}

/// Count-only variant used by the benchmark (identical matching rules, no entry materialisation).
pub fn count_matches(path: &Path, spec: &QuerySpec, cancel: &AtomicBool) -> Result<u64> {
    Ok(query_page(path, spec, u64::MAX, 1, cancel)?.total_matches)
}
