//! Error grouping by normalized signature with bounded memory.
use crate::types::ErrorGroup;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub const MAX_GROUPS: usize = 10_000;
pub const MAX_SAMPLES: usize = 3;
pub const TOP_N: usize = 10;
const MAX_SIGNATURE_CHARS: usize = 1024;

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
        .unwrap()
});
// `request_id=abc`, `req-id: abc`, `trace_id=abc`, `correlationId=abc`
static REQID_KV_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i-u)\b((?:req(?:uest)?|trace|correlation)[-_]?id)[ ]*[=:][ ]*[A-Za-z0-9._-]+")
        .unwrap()
});
// bare `req-9f3a1c` / `req_ab12cd34`
static REQ_TOKEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)\breq[-_][A-Za-z0-9]{4,}\b").unwrap());

/// Collapse whitespace and replace volatile tokens (UUIDs, request IDs). Error codes and text stay.
pub fn signature(message: &str) -> String {
    let s = UUID_RE.replace_all(message, "<uuid>");
    let s = REQID_KV_RE.replace_all(&s, "$1=<id>");
    let s = REQ_TOKEN_RE.replace_all(&s, "<req-id>");
    let joined = s.split_whitespace().collect::<Vec<_>>().join(" ");
    match joined.char_indices().nth(MAX_SIGNATURE_CHARS) {
        Some((i, _)) => joined[..i].to_string(),
        None => joined,
    }
}

/// Stable (cross-version) FNV-1a id for a signature.
pub fn group_id(sig: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in sig.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("g_{h:016x}")
}

struct Acc {
    count: u64,
    samples: Vec<u64>,
}

#[derive(Default)]
pub struct Grouper {
    map: HashMap<String, Acc>,
    truncated: bool,
}

impl Grouper {
    pub fn add(&mut self, message: &str, line_number: u64) {
        let sig = signature(message);
        if let Some(a) = self.map.get_mut(&sig) {
            a.count += 1;
            if a.samples.len() < MAX_SAMPLES {
                a.samples.push(line_number);
            }
        } else if self.map.len() < MAX_GROUPS {
            self.map.insert(
                sig,
                Acc {
                    count: 1,
                    samples: vec![line_number],
                },
            );
        } else {
            self.truncated = true;
        }
    }

    /// (top groups, truncated, distinct retained)
    pub fn finish(self) -> (Vec<ErrorGroup>, bool, u64) {
        let distinct = self.map.len() as u64;
        let mut v: Vec<_> = self.map.into_iter().collect();
        v.sort_by(|a, b| b.1.count.cmp(&a.1.count).then_with(|| a.0.cmp(&b.0)));
        v.truncate(TOP_N);
        let groups = v
            .into_iter()
            .map(|(sig, a)| ErrorGroup {
                id: group_id(&sig),
                signature: sig,
                count: a.count,
                sample_line_numbers: a.samples,
            })
            .collect();
        (groups, self.truncated, distinct)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_uuid_request_ids_and_whitespace() {
        let a = signature(
            "Payment  failed for order 8f14e45f-ceea-467a-9575-1a1b2c3d4e5f request_id=abc123 code=E402",
        );
        let b = signature(
            "Payment failed for order 11111111-2222-3333-4444-555555555555   request_id=zzz999 code=E402",
        );
        assert_eq!(a, b);
        assert!(a.contains("code=E402"), "error codes preserved: {a}");
        assert!(a.contains("<uuid>") && a.contains("request_id=<id>"));
        assert_eq!(
            signature("failed req-9f3a1c now"),
            signature("failed req_abcd12 now")
        );
    }

    #[test]
    fn different_codes_do_not_merge() {
        assert_ne!(signature("failed code=E402"), signature("failed code=E500"));
    }

    #[test]
    fn top_n_sorted_by_count_then_signature_with_three_samples() {
        let mut g = Grouper::default();
        for i in 1..=5 {
            g.add("beta", i);
        }
        for i in 6..=10 {
            g.add("alpha", i);
        }
        g.add("gamma", 11);
        let (v, trunc, distinct) = g.finish();
        assert!(!trunc);
        assert_eq!(distinct, 3);
        assert_eq!(v[0].signature, "alpha", "tie broken by signature");
        assert_eq!(v[0].count, 5);
        assert_eq!(v[0].sample_line_numbers, vec![6, 7, 8]);
        assert_eq!(v[1].signature, "beta");
        assert_eq!(v[2].signature, "gamma");
    }

    #[test]
    fn caps_distinct_groups_and_reports_truncation() {
        let mut g = Grouper::default();
        for i in 0..(MAX_GROUPS + 50) {
            g.add(&format!("unique error {i}"), i as u64 + 1);
        }
        g.add("unique error 0", 99999); // retained signature still counted exactly
        let (v, trunc, distinct) = g.finish();
        assert!(trunc);
        assert_eq!(distinct as usize, MAX_GROUPS);
        assert_eq!(v[0].signature, "unique error 0");
        assert_eq!(v[0].count, 2);
        assert_eq!(v.len(), TOP_N);
    }

    #[test]
    fn group_id_is_stable() {
        assert_eq!(group_id("abc"), "g_e71fa2190541574b");
        assert_eq!(group_id("abc"), group_id("abc"));
    }

    #[test]
    fn long_signatures_are_capped_on_char_boundary() {
        let s = signature(&"é".repeat(5000));
        assert_eq!(s.chars().count(), MAX_SIGNATURE_CHARS);
    }
}
