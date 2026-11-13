//! Benchmark driver using the production scan/search code.
//! usage: log_bench scan|group <file>   |   log_bench search <file> <query> [SEVERITY]
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use wotslog_core::scan::scan_file;
use wotslog_core::search::{QuerySpec, count_matches};
use wotslog_core::types::Severity;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: log_bench scan|group <file> | search <file> <query> [SEVERITY]");
        std::process::exit(2);
    }
    let (op, path) = (a[1].as_str(), Path::new(&a[2]));
    let cancel = AtomicBool::new(false);
    let t = Instant::now();
    let out = match op {
        "scan" | "group" => {
            let s = scan_file(path, &cancel, op == "group", |_| {}).unwrap_or_else(|e| die(e));
            let mut v = serde_json::json!({"op": op, "totalLines": s.total_lines, "severityCounts": s.severity_counts,
                "truncatedLines": s.truncated_lines, "invalidUtf8Lines": s.invalid_utf8_lines});
            if op == "group" {
                v["distinctGroups"] = s.distinct_groups.into();
                v["groupingTruncated"] = s.grouping_truncated.into();
                v["topErrors"] = s.top_errors.iter().map(|g| serde_json::json!({"signature": g.signature, "count": g.count, "samples": g.sample_line_numbers})).collect();
            }
            v
        }
        "search" => {
            let severity = a.get(4).map(|s| match s.as_str() {
                "ERROR" => Severity::Error,
                "WARN" => Severity::Warn,
                "INFO" => Severity::Info,
                "DEBUG" => Severity::Debug,
                "UNKNOWN" => Severity::Unknown,
                _ => die("bad severity"),
            });
            let spec = QuerySpec {
                query: a.get(3).cloned().unwrap_or_default(),
                severity,
            };
            let n = count_matches(path, &spec, &cancel).unwrap_or_else(|e| die(e));
            serde_json::json!({"op": op, "query": spec.query, "severity": severity.map(|s| s.as_str()), "matches": n})
        }
        _ => die("unknown op"),
    };
    let mut out = out;
    out["elapsedMs"] = (t.elapsed().as_secs_f64() * 1000.0).into();
    println!("{out}");
}

fn die(e: impl std::fmt::Display) -> ! {
    eprintln!("error: {e}");
    std::process::exit(1)
}
