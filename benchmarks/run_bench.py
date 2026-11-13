#!/usr/bin/env python3
"""Reproducible Rust-vs-Python benchmark. Stdlib only.

Builds log_bench in release mode, generates deterministic fixtures, validates that both
implementations produce identical results, then alternates Rust/Python runs
(1 warm-up + N measured) and reports medians of process wall-clock time and peak RSS.
Compilation, AI latency and UI are excluded. Results: benchmarks/results/latest.{json,md}.

  python3 benchmarks/run_bench.py --sizes 10 100 --runs 5
"""
import argparse
import json
import os
import platform
import re
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BENCH = ROOT / "benchmarks"
OPS = [  # (label, op, extra args after the file)
    ("scan (parse + severity counts)", "scan", []),
    ("search 'timeout' (all levels)", "search", ["timeout"]),
    ("search 'payment' + ERROR filter", "search", ["payment", "ERROR"]),
    ("scan + error grouping", "group", []),
]


def sh(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


def timed(cmd):
    """Run under /usr/bin/time; return (stdout, wall_ms, peak_rss_mib or None)."""
    if sys.platform == "darwin":
        wrapped, unit = ["/usr/bin/time", "-l"] + cmd, 1024 * 1024  # bytes
    elif sys.platform.startswith("linux"):
        wrapped, unit = ["/usr/bin/time", "-v"] + cmd, 1024  # kbytes
    else:
        wrapped, unit = cmd, None
    t = time.perf_counter()
    p = sh(wrapped)
    wall = (time.perf_counter() - t) * 1000
    if p.returncode != 0:
        raise RuntimeError(f"{cmd} failed: {p.stderr[-500:]}")
    rss = None
    if unit:
        m = re.search(r"(\d+)\s+maximum resident set size", p.stderr) or re.search(r"Maximum resident set size \(kbytes\): (\d+)", p.stderr)
        rss = int(m.group(1)) / unit if m else None
    return p.stdout.strip(), wall, rss


def strip(j):
    d = json.loads(j)
    d.pop("elapsedMs", None)
    return d


def machine():
    def out(c):
        return sh(c).stdout.strip()
    info = {"os": platform.platform(), "python": sys.version.split()[0], "rustc": out(["rustc", "--version"])}
    if sys.platform == "darwin":
        info["cpu"] = out(["sysctl", "-n", "machdep.cpu.brand_string"])
        info["ram_gib"] = round(int(out(["sysctl", "-n", "hw.memsize"])) / 2**30, 1)
    return info


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sizes", type=int, nargs="+", default=[10, 100], help="dataset sizes in MB (10^6 bytes)")
    ap.add_argument("--runs", type=int, default=5)
    a = ap.parse_args()

    print("building log_bench (release)...", flush=True)
    b = sh(["cargo", "build", "--release", "-p", "wotslog-core", "--bin", "log_bench"], cwd=ROOT)
    if b.returncode:
        sys.exit(b.stderr)
    target = os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target"))
    rust = [str(Path(target) / "release" / "log_bench")]
    py = [sys.executable, "-I", str(BENCH / "baseline.py")]

    (BENCH / "data").mkdir(exist_ok=True)
    (BENCH / "results").mkdir(exist_ok=True)
    rows, ok_all = [], True
    for mb in a.sizes:
        f = BENCH / "data" / f"synthetic_{mb}mb.log"
        if not f.exists():
            print(f"generating {f.name} ...", flush=True)
            r = sh([sys.executable, str(BENCH / "generate_logs.py"), "--size-bytes", str(mb * 1_000_000), "--seed", "42", "--out", str(f)])
            if r.returncode:
                sys.exit(r.stderr)
        for name, op, extra in OPS:
            rcmd, pcmd = rust + [op, str(f)] + extra, py + [op, str(f)] + extra
            ro, _, _ = timed(rcmd)  # warm-up (also the validation sample)
            po, _, _ = timed(pcmd)
            match = strip(ro) == strip(po)
            ok_all &= match
            rw, pw, rr, pr = [], [], [], []
            for _ in range(a.runs):  # alternate to spread cache/thermal effects
                _, w, m = timed(rcmd); rw.append(w); rr.append(m)
                _, w, m = timed(pcmd); pw.append(w); pr.append(m)
            med = statistics.median
            row = {"dataset": f"{mb} MB ({f.stat().st_size:,} bytes)", "operation": name, "outputsMatch": match,
                   "rustMedianMs": med(rw), "pythonMedianMs": med(pw),
                   "rustPeakMiB": med(rr) if all(x is not None for x in rr) else None,
                   "pythonPeakMiB": med(pr) if all(x is not None for x in pr) else None,
                   "rustRunsMs": rw, "pythonRunsMs": pw, "result": strip(ro) if not name.endswith("grouping") else {"totalLines": strip(ro)["totalLines"], "distinctGroups": strip(ro)["distinctGroups"]}}
            row["speedRatio"] = row["pythonMedianMs"] / row["rustMedianMs"]
            rows.append(row)
            print(f"{row['dataset']:>28} | {name:<34} | rust {row['rustMedianMs']:8.0f} ms | py {row['pythonMedianMs']:8.0f} ms | match={match}", flush=True)

    meta = {"machine": machine(), "runs": a.runs, "warmups": 1, "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"), "allOutputsMatch": ok_all}
    (BENCH / "results" / "latest.json").write_text(json.dumps({"meta": meta, "rows": rows}, indent=2))
    fmt = lambda v, p=0: "n/a" if v is None else f"{v:.{p}f}"
    md = ["| Dataset | Operation | Outputs match | Rust median ms | Python median ms | Rust peak MiB | Python peak MiB | Speed ratio (Py/Rust) |", "|---|---|---|---|---|---|---|---|"]
    for r in rows:
        md.append(f"| {r['dataset']} | {r['operation']} | {'yes' if r['outputsMatch'] else '**NO**'} | {fmt(r['rustMedianMs'])} | {fmt(r['pythonMedianMs'])} | {fmt(r['rustPeakMiB'], 1)} | {fmt(r['pythonPeakMiB'], 1)} | {r['speedRatio']:.1f}x |")
    (BENCH / "results" / "latest.md").write_text("\n".join(md) + "\n\n" + json.dumps(meta, indent=2) + "\n")
    print("\n".join(md))
    sys.exit(0 if ok_all else 1)


if __name__ == "__main__":
    main()
