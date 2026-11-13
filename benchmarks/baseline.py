#!/usr/bin/env python3
"""Python 3 standard-library baseline equivalent to core/src/bin/log_bench.rs.

Same streaming rules as the Rust code (see core/src/{linereader,parser,grouping,search}.rs):
binary buffered iteration, split on \\n only, strip one trailing \\r, 64 KiB line cap,
lossy UTF-8, NDJSON-then-plain parsing, ASCII-only case folding.

usage: baseline.py scan|group FILE   |   baseline.py search FILE QUERY [SEVERITY]
"""
import json
import re
import sys
import time

MAX_LINE = 64 * 1024
MAX_GROUPS, MAX_SAMPLES, TOP_N, MAX_SIG = 10_000, 3, 10, 1024

TS_RE = re.compile(r"^[ \t]*\[?([0-9]{4}-[0-9]{2}-[0-9]{2}[T ][0-9]{2}:[0-9]{2}:[0-9]{2}(?:[.,][0-9]+)?(?:Z|[+-][0-9]{2}:?[0-9]{2})?)\]?")
LVL_RE = re.compile(r"^[ \t|,:-]*[\[<(]?(ERROR|ERR|FATAL|CRITICAL|WARNING|WARN|INFO|DEBUG|TRACE)\b[\]>)]?[ \t:|-]*", re.I | re.A)
UUID_RE = re.compile(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
REQKV_RE = re.compile(r"\b((?:req(?:uest)?|trace|correlation)[-_]?id)[ ]*[=:][ ]*[A-Za-z0-9._-]+", re.I | re.A)
REQTOK_RE = re.compile(r"\breq[-_][A-Za-z0-9]{4,}\b", re.A)

SEV = {"error": "ERROR", "err": "ERROR", "fatal": "ERROR", "critical": "ERROR", "warn": "WARN", "warning": "WARN",
       "info": "INFO", "debug": "DEBUG", "trace": "DEBUG"}
SEVERITIES = ["ERROR", "WARN", "INFO", "DEBUG", "UNKNOWN"]


def sev_from_word(w):
    return SEV.get(w.lower(), "UNKNOWN")


def first_str(d, keys):
    for k in keys:
        if k in d:
            return d[k] if isinstance(d[k], str) else None
    return None


def parse_line(text):
    """-> (severity, message)"""
    if text.lstrip(" \t").startswith("{"):
        try:
            v = json.loads(text)
        except Exception:
            v = None
        if isinstance(v, dict):
            lvl = first_str(v, ("level", "severity"))
            msg = first_str(v, ("message", "msg"))
            return (sev_from_word(lvl) if lvl is not None else "UNKNOWN"), (msg if msg is not None else text)
    m = TS_RE.match(text)
    had_ts = m is not None
    rest = text[m.end():] if m else text
    m2 = LVL_RE.match(rest)
    if m2:
        return sev_from_word(m2.group(1)), rest[m2.end():]
    return "UNKNOWN", (rest.lstrip(" \t") if had_ts else text)


def signature(message):
    s = UUID_RE.sub("<uuid>", message)
    s = REQKV_RE.sub(r"\1=<id>", s)
    s = REQTOK_RE.sub("<req-id>", s)
    return " ".join(s.split())[:MAX_SIG]


def lines(path):
    """Yield (capped line bytes, truncated) like the Rust bounded reader."""
    with open(path, "rb", buffering=256 * 1024) as f:
        for raw in f:
            if raw.endswith(b"\n"):
                raw = raw[:-1]
            if len(raw) > MAX_LINE:
                yield raw[:MAX_LINE], True
            else:
                if raw.endswith(b"\r"):
                    raw = raw[:-1]
                yield raw, False


def decode(raw):
    try:
        return raw.decode("utf-8"), False
    except UnicodeDecodeError:
        return raw.decode("utf-8", "replace"), True


def scan(path, group):
    counts = dict.fromkeys(SEVERITIES, 0)
    total = truncated = bad = 0
    groups, gtrunc = {}, False
    for raw, tr in lines(path):
        total += 1
        truncated += tr
        text, invalid = decode(raw)
        bad += invalid
        sev, msg = parse_line(text)
        counts[sev] += 1
        if group and sev == "ERROR":
            sig = signature(msg)
            g = groups.get(sig)
            if g is not None:
                g[0] += 1
                if len(g[1]) < MAX_SAMPLES:
                    g[1].append(total)
            elif len(groups) < MAX_GROUPS:
                groups[sig] = [1, [total]]
            else:
                gtrunc = True
    out = {"op": "group" if group else "scan", "totalLines": total, "severityCounts": counts,
           "truncatedLines": truncated, "invalidUtf8Lines": bad}
    if group:
        top = sorted(groups.items(), key=lambda kv: (-kv[1][0], kv[0]))[:TOP_N]
        out["distinctGroups"] = len(groups)
        out["groupingTruncated"] = gtrunc
        out["topErrors"] = [{"signature": s, "count": c, "samples": smp} for s, (c, smp) in top]
    return out


def search(path, query, severity):
    needle = query.encode().lower()  # bytes.lower() folds ASCII only, like Rust
    matches = 0
    for raw, _ in lines(path):
        if needle and needle not in raw.lower():
            continue
        if severity is not None:
            text, _ = decode(raw)
            if parse_line(text)[0] != severity:
                continue
        matches += 1
    return {"op": "search", "query": query, "severity": severity, "matches": matches}


def main():
    a = sys.argv
    if len(a) < 3:
        sys.exit(__doc__)
    t = time.perf_counter()
    if a[1] in ("scan", "group"):
        out = scan(a[2], a[1] == "group")
    elif a[1] == "search":
        out = search(a[2], a[3] if len(a) > 3 else "", a[4] if len(a) > 4 else None)
    else:
        sys.exit("unknown op")
    out["elapsedMs"] = (time.perf_counter() - t) * 1000
    print(json.dumps(out, separators=(",", ":")))


if __name__ == "__main__":
    main()
