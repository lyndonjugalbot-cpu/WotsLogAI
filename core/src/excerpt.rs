//! Build the exact, redacted excerpt that would be sent to the AI provider for one error group.
use crate::error::{AppError, Result};
use crate::grouping::signature;
use crate::linereader::read_line;
use crate::parser::parse_line;
use crate::redaction::redact;
use crate::scan::{check_size, open_reader};
use crate::types::Severity;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

pub const MAX_EXCERPT_LINES: usize = 20;
pub const MAX_EXCERPT_BYTES: usize = 12 * 1024;

#[derive(Debug, Clone)]
pub struct Excerpt {
    pub text: String,
    pub line_numbers: Vec<u64>,
    pub redactions: BTreeMap<&'static str, usize>,
}

/// Take the first lines (<= 20, <= 12 KiB total) of the group, formatted `L<n> | <ts> <SEV> | <message>`, redacted.
pub fn build_excerpt(path: &Path, group_signature: &str, cancel: &AtomicBool) -> Result<Excerpt> {
    let (mut rd, len) = open_reader(path)?;
    check_size(len)?;
    let (mut buf, mut text) = (Vec::new(), String::new());
    let mut line_numbers = vec![];
    let mut redactions = BTreeMap::new();
    let mut n = 0u64;
    while line_numbers.len() < MAX_EXCERPT_LINES {
        let Some(_) = read_line(&mut rd, &mut buf)? else {
            break;
        };
        n += 1;
        if n.is_multiple_of(2048) && cancel.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        let s = String::from_utf8_lossy(&buf);
        let p = parse_line(&s);
        if p.severity != Severity::Error || signature(&p.message) != group_signature {
            continue;
        }
        let (red, counts) = redact(&format!(
            "L{n} | {} ERROR | {}",
            p.timestamp.as_deref().unwrap_or("-"),
            p.message
        ));
        let mut line = red;
        if text.len() + line.len() + 1 > MAX_EXCERPT_BYTES {
            if !line_numbers.is_empty() {
                break;
            }
            let mut cut = MAX_EXCERPT_BYTES - 1;
            while !line.is_char_boundary(cut) {
                cut -= 1;
            }
            line.truncate(cut);
        }
        for (k, v) in counts {
            *redactions.entry(k).or_insert(0) += v;
        }
        text.push_str(&line);
        text.push('\n');
        line_numbers.push(n);
    }
    if line_numbers.is_empty() {
        return Err(AppError::GroupNotFound);
    }
    Ok(Excerpt {
        text,
        line_numbers,
        redactions,
    })
}
