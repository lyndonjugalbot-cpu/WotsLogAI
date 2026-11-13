//! Best-effort redaction of secrets and personal data before text leaves the machine.
//! Not a guarantee: the user must inspect the preview.
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::LazyLock;

struct Rule {
    label: &'static str,
    re: Regex,
    rep: &'static str,
}

fn rule(label: &'static str, pat: &str, rep: &'static str) -> Rule {
    Rule {
        label,
        re: Regex::new(pat).unwrap(),
        rep,
    }
}

// Order matters: specific token shapes first, generic key=value, then identity data.
static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    vec![
        rule(
            "private key",
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(?:-----END [A-Z ]*PRIVATE KEY-----|$)",
            "[REDACTED_PRIVATE_KEY]",
        ),
        rule(
            "JWT",
            r"\beyJ[A-Za-z0-9_-]{5,}\.eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]*",
            "[REDACTED_JWT]",
        ),
        rule(
            "bearer token",
            r"(?i)\b(bearer|basic)[ \t]+[A-Za-z0-9._~+/=-]{8,}",
            "$1 [REDACTED_TOKEN]",
        ),
        rule(
            "API key",
            r"\b(?:sk-[A-Za-z0-9_-]{16,}|AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{20,}|xox[abprs]-[A-Za-z0-9-]{10,}|AIza[0-9A-Za-z_-]{30,})\b",
            "[REDACTED_API_KEY]",
        ),
        rule(
            "URL credentials",
            r"(?i)\b([a-z][a-z0-9+.-]*://)[^\s/:@]+:[^\s/@]+@",
            "${1}[REDACTED_CREDENTIALS]@",
        ),
        rule(
            "credential",
            r#"(?i)(["']?\b(?:api[_-]?key|apikey|secret|client[_-]?secret|token|access[_-]?token|auth[_-]?token|password|passwd|pwd)["']?[ \t]*[:=][ \t]*)(?:"[^"]*"|'[^']*'|[^\s,;&}"']+)"#,
            "${1}[REDACTED]",
        ),
        rule(
            "email",
            r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}\b",
            "[REDACTED_EMAIL]",
        ),
        rule(
            "IPv4",
            r"\b(?:(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])\b",
            "[REDACTED_IP]",
        ),
        rule(
            "IPv6",
            r"\b(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}\b|\b(?:[0-9a-fA-F]{1,4}:){1,6}(?::[0-9a-fA-F]{1,4}){1,6}\b|::1\b",
            "[REDACTED_IP]",
        ),
    ]
});

/// Returns the redacted text and a count per category.
pub fn redact(input: &str) -> (String, BTreeMap<&'static str, usize>) {
    let mut out = input.to_string();
    let mut counts = BTreeMap::new();
    for r in RULES.iter() {
        let n = r.re.find_iter(&out).count();
        if n > 0 {
            out = r.re.replace_all(&out, r.rep).into_owned();
            *counts.entry(r.label).or_insert(0) += n;
        }
    }
    (out, counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> String {
        redact(s).0
    }

    #[test]
    fn bearer_and_jwt() {
        assert_eq!(
            r("Authorization: Bearer abcdef1234567890"),
            "Authorization: Bearer [REDACTED_TOKEN]"
        );
        assert_eq!(
            r("hdr Bearer abcdef1234567890 end"),
            "hdr Bearer [REDACTED_TOKEN] end"
        );
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            r(&format!("token seen {jwt} ok")),
            "token seen [REDACTED_JWT] ok"
        );
    }

    #[test]
    fn key_value_credentials_incl_json_form() {
        assert_eq!(
            r("password=hunter2 user=bob"),
            "password=[REDACTED] user=bob"
        );
        assert_eq!(
            r(r#"{"api_key":"abc123","x":1}"#),
            r#"{"api_key":[REDACTED],"x":1}"#
        );
        assert_eq!(r("client_secret: s3cr3t!"), "client_secret: [REDACTED]");
    }

    #[test]
    fn known_key_shapes_urls_email_ip() {
        assert_eq!(
            r("key sk-abcdefghijklmnop1234 used"),
            "key [REDACTED_API_KEY] used"
        );
        assert_eq!(r("AKIAIOSFODNN7EXAMPLE"), "[REDACTED_API_KEY]");
        assert_eq!(
            r("db postgres://admin:pw@db.internal:5432/x"),
            "db postgres://[REDACTED_CREDENTIALS]@db.internal:5432/x"
        );
        assert_eq!(
            r("mail jane.doe+x@example.co.nz sent"),
            "mail [REDACTED_EMAIL] sent"
        );
        assert_eq!(
            r("from 192.168.1.20 to 10.0.0.1"),
            "from [REDACTED_IP] to [REDACTED_IP]"
        );
        assert_eq!(
            r("peer 2001:db8:0:0:0:0:0:1 lost"),
            "peer [REDACTED_IP] lost"
        );
    }

    #[test]
    fn leaves_ordinary_text_and_versions_alone() {
        let s = "timeout after 30s calling payments v1.2.3 code=E402";
        assert_eq!(r(s), s);
    }

    #[test]
    fn reports_counts() {
        let (_, c) = redact("a@b.com c@d.org 1.2.3.4");
        assert_eq!(c.get("email"), Some(&2));
        assert_eq!(c.get("IPv4"), Some(&1));
    }
}
