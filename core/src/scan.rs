//! Streaming scan: totals, severity counts and error groups in a single bounded pass.
use crate::error::{AppError, Result};
use crate::grouping::Grouper;
use crate::linereader::read_line;
use crate::parser::parse_line;
use crate::types::{Severity, SeverityCounts, Summary};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;
const CANCEL_CHECK_EVERY: u64 = 2048;
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

pub fn open_reader(path: &Path) -> Result<(BufReader<File>, u64)> {
    let f = File::open(path)?;
    let len = f.metadata()?.len();
    Ok((BufReader::with_capacity(256 * 1024, f), len))
}

pub fn check_size(len: u64) -> Result<()> {
    if len > MAX_FILE_BYTES {
        return Err(AppError::FileTooLarge {
            size: len,
            limit: MAX_FILE_BYTES,
        });
    }
    Ok(())
}

/// Scan `path` (`group` = also build error groups). `progress(bytes_read)` is called at most every ~100 ms.
pub fn scan_file(
    path: &Path,
    cancel: &AtomicBool,
    group: bool,
    mut progress: impl FnMut(u64),
) -> Result<Summary> {
    let start = Instant::now();
    let (mut rd, len) = open_reader(path)?;
    check_size(len)?;
    let mut counts = SeverityCounts::default();
    let mut grouper = Grouper::default();
    let (mut lines, mut bytes, mut truncated, mut bad_utf8) = (0u64, 0u64, 0u64, 0u64);
    let mut buf = Vec::with_capacity(4096);
    let mut last_progress = Instant::now();
    while let Some(meta) = read_line(&mut rd, &mut buf)? {
        lines += 1;
        bytes += meta.consumed;
        truncated += meta.truncated as u64;
        let text = String::from_utf8_lossy(&buf);
        if matches!(text, std::borrow::Cow::Owned(_)) {
            bad_utf8 += 1;
        }
        let p = parse_line(&text);
        counts.add(p.severity);
        if group && p.severity == Severity::Error {
            grouper.add(&p.message, lines);
        }
        if lines.is_multiple_of(CANCEL_CHECK_EVERY) {
            if cancel.load(Ordering::Relaxed) {
                return Err(AppError::Cancelled);
            }
            if last_progress.elapsed() >= PROGRESS_EVERY {
                progress(bytes);
                last_progress = Instant::now();
            }
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::Cancelled);
    }
    progress(bytes);
    let (top_errors, grouping_truncated, distinct_groups) = grouper.finish();
    Ok(Summary {
        total_lines: lines,
        severity_counts: counts,
        top_errors,
        elapsed_ms: start.elapsed().as_millis() as u64,
        grouping_truncated,
        distinct_groups,
        bytes,
        truncated_lines: truncated,
        invalid_utf8_lines: bad_utf8,
    })
}
