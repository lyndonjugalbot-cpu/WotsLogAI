use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::NamedTempFile;
use wotslog_core::AppError;
use wotslog_core::ai::{build_user_message, request_diagnosis};
use wotslog_core::credentials::{CredentialStore, MemoryStore, set_verified};
use wotslog_core::excerpt::build_excerpt;
use wotslog_core::scan::{MAX_FILE_BYTES, scan_file};
use wotslog_core::search::{QuerySpec, query_page};
use wotslog_core::session::{Session, Source};
use wotslog_core::settings::ProviderSettings;
use wotslog_core::types::Severity;

fn file(bytes: &[u8]) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(bytes).unwrap();
    f.flush().unwrap();
    f
}
fn no_cancel() -> AtomicBool {
    AtomicBool::new(false)
}

const SAMPLE: &str = "\
2025-03-01T10:00:00Z INFO server started
2025-03-01T10:00:01Z ERROR payment failed order=8f14e45f-ceea-467a-9575-1a1b2c3d4e5f request_id=aaa111 code=E402
2025-03-01T10:00:02Z WARNING slow query 1200ms
{\"timestamp\":\"2025-03-01T10:00:03Z\",\"level\":\"error\",\"message\":\"payment failed order=11111111-2222-3333-4444-555555555555 request_id=bbb222 code=E402\"}
{\"level\":\"error\",\"message\":\"broken json\"
free text with no header
2025-03-01T10:00:05Z DEBUG cache miss key=users:42
2025-03-01T10:00:06Z ERROR db connection refused host=db1
2025-03-01T10:00:07Z ERROR payment failed order=22222222-2222-3333-4444-555555555555 request_id=ccc333 code=E402";

#[test]
fn scan_counts_groups_and_handles_missing_final_newline() {
    let f = file(SAMPLE.as_bytes());
    let s = scan_file(f.path(), &no_cancel(), true, |_| {}).unwrap();
    assert_eq!(s.total_lines, 9);
    assert_eq!(s.severity_counts.total(), 9);
    let c = &s.severity_counts;
    assert_eq!(
        (c.error, c.warn, c.info, c.debug, c.unknown),
        (4, 1, 1, 1, 2)
    );
    assert_eq!(
        s.top_errors[0].count, 3,
        "plain + ndjson + plain merge into one group"
    );
    assert_eq!(s.top_errors[0].sample_line_numbers, vec![2, 4, 9]);
    assert_eq!(s.top_errors[1].count, 1);
    assert!(!s.grouping_truncated);
    assert_eq!(s.bytes, SAMPLE.len() as u64);
}

#[test]
fn scan_is_deterministic() {
    let f = file(SAMPLE.as_bytes());
    let a = scan_file(f.path(), &no_cancel(), true, |_| {}).unwrap();
    let b = scan_file(f.path(), &no_cancel(), true, |_| {}).unwrap();
    assert_eq!(
        (a.top_errors, a.severity_counts),
        (b.top_errors, b.severity_counts)
    );
}

#[test]
fn empty_file_is_valid() {
    let f = file(b"");
    let s = scan_file(f.path(), &no_cancel(), true, |_| {}).unwrap();
    assert_eq!(s.total_lines, 0);
    assert!(s.top_errors.is_empty());
    let p = query_page(f.path(), &QuerySpec::default(), 0, 100, &no_cancel()).unwrap();
    assert_eq!((p.total_matches, p.entries.len()), (0, 0));
}

#[test]
fn missing_file_and_oversized_file_are_typed_errors() {
    let r = scan_file(
        std::path::Path::new("/definitely/not/here.log"),
        &no_cancel(),
        false,
        |_| {},
    );
    assert_eq!(r.unwrap_err(), AppError::FileNotFound);
    let f = NamedTempFile::new().unwrap();
    f.as_file().set_len(MAX_FILE_BYTES + 1).unwrap();
    assert!(matches!(
        scan_file(f.path(), &no_cancel(), false, |_| {}),
        Err(AppError::FileTooLarge { .. })
    ));
    assert!(matches!(
        Session::open(f.path().to_path_buf(), Source::Local),
        Err(AppError::FileTooLarge { .. })
    ));
}

#[test]
fn invalid_utf8_crlf_and_oversized_lines() {
    let mut data = b"ERROR bad \xff\xfe bytes\r\nINFO ok\r\n".to_vec();
    data.extend(b"ERROR ");
    data.extend(vec![b'z'; 200_000]);
    data.extend(b"\nINFO tail");
    let f = file(&data);
    let s = scan_file(f.path(), &no_cancel(), true, |_| {}).unwrap();
    assert_eq!(s.total_lines, 4);
    assert_eq!(s.invalid_utf8_lines, 1);
    assert_eq!(s.truncated_lines, 1);
    assert_eq!(s.severity_counts.error, 2);
    let p = query_page(
        f.path(),
        &QuerySpec {
            query: "bad".into(),
            severity: None,
        },
        0,
        10,
        &no_cancel(),
    )
    .unwrap();
    assert!(p.entries[0].message.contains('\u{FFFD}'));
    assert!(!p.entries[0].message.ends_with('\r'));
    let long = query_page(
        f.path(),
        &QuerySpec {
            severity: Some(Severity::Error),
            ..Default::default()
        },
        1,
        10,
        &no_cancel(),
    )
    .unwrap();
    assert!(long.entries[0].truncated);
}

#[test]
fn search_is_case_insensitive_literal_with_correct_line_numbers() {
    let f = file(SAMPLE.as_bytes());
    let spec = QuerySpec {
        query: "PAYMENT FAILED".into(),
        severity: None,
    };
    let p = query_page(f.path(), &spec, 0, 100, &no_cancel()).unwrap();
    assert_eq!(p.total_matches, 3);
    assert_eq!(
        p.entries.iter().map(|e| e.line_number).collect::<Vec<_>>(),
        vec![2, 4, 9]
    );
    // regex metacharacters are literal
    let p = query_page(
        f.path(),
        &QuerySpec {
            query: "key=users:42 (".into(),
            severity: None,
        },
        0,
        100,
        &no_cancel(),
    )
    .unwrap();
    assert_eq!(p.total_matches, 0);
    let p = query_page(
        f.path(),
        &QuerySpec {
            query: ".*".into(),
            severity: None,
        },
        0,
        100,
        &no_cancel(),
    )
    .unwrap();
    assert_eq!(p.total_matches, 0);
}

#[test]
fn severity_filter_pagination_and_limits() {
    let f = file(SAMPLE.as_bytes());
    let spec = QuerySpec {
        query: String::new(),
        severity: Some(Severity::Error),
    };
    let p1 = query_page(f.path(), &spec, 0, 3, &no_cancel()).unwrap();
    let p2 = query_page(f.path(), &spec, 3, 3, &no_cancel()).unwrap();
    assert_eq!(p1.total_matches, 4);
    assert_eq!(p1.entries.len(), 3);
    assert_eq!(p2.entries.len(), 1);
    assert_eq!(p2.entries[0].line_number, 9);
    assert!(query_page(f.path(), &spec, 0, 101, &no_cancel()).is_err());
    assert!(query_page(f.path(), &spec, 0, 0, &no_cancel()).is_err());
    let both = QuerySpec {
        query: "connection".into(),
        severity: Some(Severity::Warn),
    };
    assert_eq!(
        query_page(f.path(), &both, 0, 10, &no_cancel())
            .unwrap()
            .total_matches,
        0
    );
}

#[test]
fn cancellation_returns_cancelled_for_scan_and_query() {
    let mut data = Vec::new();
    for i in 0..10_000 {
        writeln!(data, "2025-03-01T10:00:00Z INFO line {i}").unwrap();
    }
    let f = file(&data);
    let cancel = AtomicBool::new(true);
    assert_eq!(
        scan_file(f.path(), &cancel, true, |_| {}).unwrap_err(),
        AppError::Cancelled
    );
    assert_eq!(
        query_page(f.path(), &QuerySpec::default(), 0, 10, &cancel).unwrap_err(),
        AppError::Cancelled
    );
}

#[test]
fn changed_file_is_detected() {
    let mut f = file(b"INFO a\n");
    let s = Session::open(f.path().to_path_buf(), Source::Local).unwrap();
    assert!(s.verify_unchanged().is_ok());
    f.write_all(b"INFO b\n").unwrap();
    f.flush().unwrap();
    assert_eq!(s.verify_unchanged().unwrap_err(), AppError::FileChanged);
}

#[test]
fn excerpt_is_bounded_redacted_and_only_for_the_group() {
    let mut data = String::new();
    for i in 0..50 {
        data += &format!(
            "2025-03-01T10:00:{:02}Z ERROR login failed for jane@example.com password=hunter2 from 10.0.0.5 request_id=r{i}\n",
            i % 60
        );
        data += "2025-03-01T10:00:00Z ERROR something else entirely\n";
    }
    let f = file(data.as_bytes());
    let s = scan_file(f.path(), &no_cancel(), true, |_| {}).unwrap();
    let g = s
        .top_errors
        .iter()
        .find(|g| g.signature.starts_with("login failed"))
        .expect("group present");
    let ex = build_excerpt(f.path(), &g.signature, &no_cancel()).unwrap();
    assert_eq!(ex.line_numbers.len(), 20);
    assert!(ex.text.len() <= 12 * 1024);
    assert!(
        !ex.text.contains("@example.com")
            && !ex.text.contains("hunter")
            && !ex.text.contains("10.0.0.5")
    );
    assert!(ex.text.contains("[REDACTED_EMAIL]") && ex.text.contains("password=[REDACTED]"));
    assert!(!ex.text.contains("something else"));
    assert_eq!(
        build_excerpt(f.path(), "no such signature", &no_cancel()).unwrap_err(),
        AppError::GroupNotFound
    );
}

#[test]
fn excerpt_total_size_is_capped_for_huge_lines() {
    let mut data = String::new();
    for _ in 0..10 {
        data += &format!("ERROR {}\n", "A".repeat(5000));
    }
    let f = file(data.as_bytes());
    let s = scan_file(f.path(), &no_cancel(), true, |_| {}).unwrap();
    let ex = build_excerpt(f.path(), &s.top_errors[0].signature, &no_cancel()).unwrap();
    assert!(ex.text.len() <= 12 * 1024);
    assert!(ex.line_numbers.len() < 10);
}

#[test]
fn credential_round_trip_check_rejects_lossy_backend() {
    let ok = MemoryStore::default();
    assert!(set_verified(&ok, "k").is_ok());
    assert_eq!(ok.get().unwrap().as_deref(), Some("k"));

    struct Lossy;
    impl CredentialStore for Lossy {
        fn set(&self, _: &str) -> wotslog_core::Result<()> {
            Ok(())
        }
        fn get(&self) -> wotslog_core::Result<Option<String>> {
            Ok(None)
        }
        fn delete(&self) -> wotslog_core::Result<()> {
            Ok(())
        }
    }
    assert!(matches!(
        set_verified(&Lossy, "k"),
        Err(AppError::Credential(_))
    ));
}

// ---------- AI client against a local mock endpoint ----------

struct Mock {
    base_url: String,
    seen: Arc<Mutex<Vec<String>>>,
}

async fn mock(
    status: u16,
    body: &'static str,
    delay: Duration,
    extra_headers: &'static str,
) -> Mock {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(vec![]));
    let s2 = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let s3 = s2.clone();
            tokio::spawn(async move {
                let mut data = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let n = sock.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    data.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&data).to_string();
                    if let Some(h) = text.find("\r\n\r\n") {
                        let cl = text[..h]
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if data.len() >= h + 4 + cl {
                            break;
                        }
                    }
                }
                s3.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&data).to_string());
                tokio::time::sleep(delay).await;
                let resp = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\n{extra_headers}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    Mock {
        base_url: format!("http://127.0.0.1:{port}/v1"),
        seen,
    }
}

fn settings(m: &Mock) -> ProviderSettings {
    ProviderSettings {
        base_url: m.base_url.clone(),
        model: "test-model".into(),
    }
}
const GOOD: &str = r#"{"choices":[{"message":{"content":"{\"summary\":\"s\",\"likelyCauses\":[\"c\"],\"evidence\":[{\"lineNumber\":2,\"explanation\":\"e\"}],\"nextSteps\":[\"n\"],\"uncertainty\":\"u\"}"}}]}"#;

#[tokio::test]
async fn ai_success_sends_only_the_approved_message_with_bearer_key() {
    let m = mock(200, GOOD, Duration::ZERO, "").await;
    let msg = build_user_message("L2 | - ERROR | boom\n");
    let d = request_diagnosis(
        &settings(&m),
        Some("sekret-key"),
        &msg,
        &[2],
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert_eq!(d.summary, "s");
    let req = m.seen.lock().unwrap()[0].clone();
    assert!(req.starts_with("POST /v1/chat/completions"));
    assert!(
        req.to_ascii_lowercase()
            .contains("authorization: bearer sekret-key")
    );
    let body: serde_json::Value =
        serde_json::from_str(req.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["model"], "test-model");
    assert_eq!(body["messages"][1]["content"], msg.as_str());
    assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    assert!(
        body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("untrusted data")
    );
}

#[tokio::test]
async fn ai_without_key_sends_no_authorization_header() {
    let m = mock(200, GOOD, Duration::ZERO, "").await;
    request_diagnosis(&settings(&m), None, "x", &[2], Duration::from_secs(5))
        .await
        .unwrap();
    assert!(
        !m.seen.lock().unwrap()[0]
            .to_ascii_lowercase()
            .contains("authorization")
    );
}

#[tokio::test]
async fn ai_error_mapping() {
    let t = Duration::from_secs(5);
    let m = mock(401, "{}", Duration::ZERO, "").await;
    assert_eq!(
        request_diagnosis(&settings(&m), Some("k"), "x", &[2], t)
            .await
            .unwrap_err(),
        AppError::AiUnauthorized
    );
    let m = mock(429, "{}", Duration::ZERO, "retry-after: 7\r\n").await;
    assert_eq!(
        request_diagnosis(&settings(&m), None, "x", &[2], t)
            .await
            .unwrap_err(),
        AppError::AiRateLimited {
            retry_after_secs: Some(7)
        }
    );
    let m = mock(503, "{}", Duration::ZERO, "").await;
    let e = request_diagnosis(&settings(&m), None, "x", &[2], t)
        .await
        .unwrap_err();
    assert_eq!(e, AppError::AiHttp(503));
    assert!(e.is_retryable());
    let m = mock(
        200,
        r#"{"choices":[{"message":{"content":"sorry, no json"}}]}"#,
        Duration::ZERO,
        "",
    )
    .await;
    assert!(matches!(
        request_diagnosis(&settings(&m), None, "x", &[2], t)
            .await
            .unwrap_err(),
        AppError::AiInvalidResponse(_)
    ));
    let m = mock(200, GOOD, Duration::ZERO, "").await;
    assert!(
        matches!(
            request_diagnosis(&settings(&m), None, "x", &[99], t)
                .await
                .unwrap_err(),
            AppError::AiInvalidResponse(_)
        ),
        "fabricated evidence line rejected"
    );
}

#[tokio::test]
async fn ai_timeout_and_offline() {
    let m = mock(200, GOOD, Duration::from_millis(1500), "").await;
    assert_eq!(
        request_diagnosis(&settings(&m), None, "x", &[2], Duration::from_millis(200))
            .await
            .unwrap_err(),
        AppError::AiTimeout
    );
    let dead = ProviderSettings {
        base_url: "http://127.0.0.1:1/v1".into(),
        model: "m".into(),
    };
    assert_eq!(
        request_diagnosis(&dead, None, "x", &[2], Duration::from_secs(2))
            .await
            .unwrap_err(),
        AppError::AiOffline
    );
}

#[tokio::test]
async fn ai_refuses_plain_http_to_non_loopback() {
    let s = ProviderSettings {
        base_url: "http://example.com/v1".into(),
        model: "m".into(),
    };
    assert!(matches!(
        request_diagnosis(&s, None, "x", &[2], Duration::from_secs(1)).await,
        Err(AppError::InvalidArgument(_))
    ));
}

#[test]
fn ndjson_and_plain_severity_agree_in_search() {
    let f = file(SAMPLE.as_bytes());
    let spec = QuerySpec {
        query: "payment".into(),
        severity: Some(Severity::Error),
    };
    assert_eq!(
        query_page(f.path(), &spec, 0, 100, &no_cancel())
            .unwrap()
            .total_matches,
        3
    );
}
