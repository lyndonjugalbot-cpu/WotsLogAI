//! Line parsing: NDJSON objects and plain text with conventional timestamp/level headers.
//! The same rules are mirrored in `benchmarks/baseline.py`; keep them in sync.
use crate::types::Severity;
use regex::Regex;
use std::borrow::Cow;
use std::sync::LazyLock;

// ISO-8601 timestamp at the start of the line, optionally in brackets.
static TS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^[ \t]*\[?([0-9]{4}-[0-9]{2}-[0-9]{2}[T ][0-9]{2}:[0-9]{2}:[0-9]{2}(?:[.,][0-9]+)?(?:Z|[+-][0-9]{2}:?[0-9]{2})?)\]?",
    )
    .unwrap()
});
// Conventional level header directly after the (optional) timestamp.
static LVL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i-u)^[ \t|,:-]*[\[<(]?(ERROR|ERR|FATAL|CRITICAL|WARNING|WARN|INFO|DEBUG|TRACE)\b[\]>)]?[ \t:|-]*",
    )
    .unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed<'a> {
    pub timestamp: Option<Cow<'a, str>>,
    pub severity: Severity,
    pub message: Cow<'a, str>,
}

pub fn parse_line(text: &str) -> Parsed<'_> {
    if text.trim_start_matches([' ', '\t']).starts_with('{')
        && let Some(p) = parse_ndjson(text)
    {
        return p;
    }
    parse_plain(text)
}

fn first_str<'v>(
    map: &'v serde_json::Map<String, serde_json::Value>,
    keys: [&str; 2],
) -> (bool, Option<&'v str>) {
    for k in keys {
        if let Some(v) = map.get(k) {
            return (true, v.as_str());
        }
    }
    (false, None)
}

fn parse_ndjson(text: &str) -> Option<Parsed<'_>> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let map = v.as_object()?;
    let (_, ts) = first_str(map, ["timestamp", "time"]);
    let (_, lvl) = first_str(map, ["level", "severity"]);
    let (_, msg) = first_str(map, ["message", "msg"]);
    Some(Parsed {
        timestamp: ts.map(|s| Cow::Owned(s.to_string())),
        severity: lvl.map(Severity::from_word).unwrap_or(Severity::Unknown),
        message: match msg {
            Some(m) => Cow::Owned(m.to_string()),
            None => Cow::Borrowed(text),
        },
    })
}

fn parse_plain(text: &str) -> Parsed<'_> {
    let (timestamp, rest, had_ts) = match TS_RE.captures(text) {
        Some(c) => {
            let m = c.get(0).unwrap();
            (
                Some(Cow::Borrowed(c.get(1).unwrap().as_str())),
                &text[m.end()..],
                true,
            )
        }
        None => (None, text, false),
    };
    match LVL_RE.captures(rest) {
        Some(c) => Parsed {
            timestamp,
            severity: Severity::from_word(c.get(1).unwrap().as_str()),
            message: Cow::Borrowed(&rest[c.get(0).unwrap().end()..]),
        },
        None => Parsed {
            timestamp,
            severity: Severity::Unknown,
            message: Cow::Borrowed(if had_ts {
                rest.trim_start_matches([' ', '\t'])
            } else {
                text
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> (Option<String>, Severity, String) {
        let r = parse_line(s);
        (
            r.timestamp.map(|t| t.into_owned()),
            r.severity,
            r.message.into_owned(),
        )
    }

    #[test]
    fn iso_timestamp_and_level() {
        let (ts, sev, msg) = p("2025-03-01T10:15:30.123Z ERROR db: connection refused");
        assert_eq!(ts.as_deref(), Some("2025-03-01T10:15:30.123Z"));
        assert_eq!(sev, Severity::Error);
        assert_eq!(msg, "db: connection refused");
    }

    #[test]
    fn bracketed_variants_and_warning_normalised() {
        let (ts, sev, msg) = p("[2025-03-01 10:15:30] [WARNING] disk at 91%");
        assert_eq!(ts.as_deref(), Some("2025-03-01 10:15:30"));
        assert_eq!(sev, Severity::Warn);
        assert_eq!(msg, "disk at 91%");
        assert_eq!(p("INFO: started").1, Severity::Info);
        assert_eq!(
            p("2025-03-01T10:15:30+02:00 | debug | x").1,
            Severity::Debug
        );
    }

    #[test]
    fn no_inference_from_arbitrary_substrings() {
        assert_eq!(
            p("Something failed with an error today").1,
            Severity::Unknown
        );
        assert_eq!(p("ERROR_CODE=5 happened").1, Severity::Unknown);
        assert_eq!(p("errors detected").1, Severity::Unknown);
        assert_eq!(p("").1, Severity::Unknown);
    }

    #[test]
    fn unrecognised_level_is_unknown_and_message_preserved() {
        let (ts, sev, msg) = p("  free text line");
        assert_eq!(
            (ts, sev, msg.as_str()),
            (None, Severity::Unknown, "  free text line")
        );
        let (ts, sev, msg) = p("2025-03-01T10:15:30Z NOTICE hello");
        assert_eq!(ts.as_deref(), Some("2025-03-01T10:15:30Z"));
        assert_eq!(sev, Severity::Unknown);
        assert_eq!(msg, "NOTICE hello");
    }

    #[test]
    fn ndjson_common_keys() {
        let (ts, sev, msg) =
            p(r#"{"timestamp":"2025-03-01T10:00:00Z","level":"warning","message":"slow query"}"#);
        assert_eq!(ts.as_deref(), Some("2025-03-01T10:00:00Z"));
        assert_eq!(sev, Severity::Warn);
        assert_eq!(msg, "slow query");
        let (ts, sev, msg) = p(r#"{"time":"t1","severity":"ERROR","msg":"boom"}"#);
        assert_eq!(
            (ts.as_deref(), sev, msg.as_str()),
            (Some("t1"), Severity::Error, "boom")
        );
    }

    #[test]
    fn ndjson_without_message_keeps_raw_line() {
        let raw = r#"{"level":"info","code":7}"#;
        let (_, sev, msg) = p(raw);
        assert_eq!(sev, Severity::Info);
        assert_eq!(msg, raw);
    }

    #[test]
    fn malformed_json_falls_back_to_plain_text() {
        let raw = r#"{"level":"error","message":"oops""#;
        let (ts, sev, msg) = p(raw);
        assert_eq!((ts, sev, msg.as_str()), (None, Severity::Unknown, raw));
        assert_eq!(p("{not json at all}").1, Severity::Unknown);
        assert_eq!(p("[1,2,3]").1, Severity::Unknown);
    }
}
