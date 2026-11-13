#!/usr/bin/env python3
"""Precompute public/web-demo.json (browser preview data) from fixtures/demo.log using the
same parsing/grouping rules as the Rust core (benchmarks/baseline.py, verified equal)."""
import json, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "benchmarks"))
import baseline as b

path = ROOT / "fixtures" / "demo.log"
counts = dict.fromkeys(b.SEVERITIES, 0)
entries, groups = [], {}
for n, (raw, tr) in enumerate(b.lines(path), 1):
    text, _ = b.decode(raw)
    sev, msg = b.parse_line(text)
    m = b.TS_RE.match(text)
    counts[sev] += 1
    entries.append({"lineNumber": n, "timestamp": m.group(1) if m else None, "severity": sev, "message": msg, "truncated": tr, "raw": text})
    if sev == "ERROR":
        g = groups.setdefault(b.signature(msg), [0, []])
        g[0] += 1
        if len(g[1]) < 3: g[1].append(n)
top = sorted(groups.items(), key=lambda kv: (-kv[1][0], kv[0]))[:10]
out = {
    "session": {"sessionId": "web-demo", "displayName": "demo.log", "sizeBytes": path.stat().st_size, "isDemo": True},
    "summary": {"sessionId": "web-demo", "scanId": "web", "maxFileBytes": 104857600, "totalLines": len(entries),
                "severityCounts": counts, "elapsedMs": 0, "groupingTruncated": False, "distinctGroups": len(groups),
                "bytes": path.stat().st_size, "truncatedLines": 0, "invalidUtf8Lines": 0,
                "topErrors": [{"id": f"g{i}", "signature": s, "count": c, "sampleLineNumbers": smp} for i, (s, (c, smp)) in enumerate(top)]},
    "entries": entries,
    "analyses": json.loads((ROOT / "fixtures" / "demo_analysis.json").read_text()),
}
(ROOT / "public" / "web-demo.json").write_text(json.dumps(out, separators=(",", ":")))
print("wrote public/web-demo.json", len(entries), "entries")
