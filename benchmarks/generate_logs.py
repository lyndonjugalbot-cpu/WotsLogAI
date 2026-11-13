#!/usr/bin/env python3
"""Deterministic synthetic log generator (stdlib only). No real or private data.

  generate_logs.py --lines 3000 --seed 7 --out fixtures/demo.log        # demo fixture
  generate_logs.py --size-bytes 10000000 --seed 42 --out data/10mb.log   # benchmark fixture

Mix: plain ISO-8601 lines (INFO/DEBUG/WARN/ERROR), NDJSON, malformed JSON, free text,
CRLF endings, rare invalid UTF-8, rare >64 KiB lines, no trailing newline. All emails use
example.com, all IPs are RFC 5737 documentation addresses, the JWT is a non-secret dummy.
"""
import argparse
import random

FAKE_JWT = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJkZW1vLXVzZXIifQ.ZGVtby1zaWduYXR1cmU"
PATHS = ["orders", "users", "cart", "search", "inventory", "invoices", "sessions"]
TABLES = ["orders", "users", "events", "payments"]
ERR_WEIGHTS = [30, 25, 15, 10, 5, 10, 5]


def hexs(rng, n):
    return "".join(rng.choice("0123456789abcdef") for _ in range(n))


def uuid(rng):
    return f"{hexs(rng, 8)}-{hexs(rng, 4)}-{hexs(rng, 4)}-{hexs(rng, 4)}-{hexs(rng, 12)}"


def error_msg(rng):
    k = rng.choices(range(7), ERR_WEIGHTS)[0]
    if k == 0:
        return f"PaymentService: gateway request timed out after 30000ms order={uuid(rng)} request_id=req-{hexs(rng, 6)}"
    if k == 1:
        return f"db pool: connection refused host=db-primary:5432 code=E_DB_CONN request_id=req-{hexs(rng, 6)}"
    if k == 2:
        return f"UserService: NullPointerException at ProfileMapper.map(ProfileMapper.java:88) user={uuid(rng)}"
    if k == 3:
        return f"auth: token validation failed reason=expired user=jane.doe@example.com token={FAKE_JWT} ip=203.0.113.7"
    if k == 4:
        return f"cache: eviction storm detected region=eu-west-{rng.randint(1, 3)}"
    if k == 5:
        return f"worker: job failed after {rng.randint(1, 20000)}ms code=E_JOB"
    return f"disk: quota exceeded volume=/data{rng.randint(1, 3)} used={rng.randint(91, 99)}%"


def info_msg(rng):
    return (f"request handled path=/api/v1/{rng.choice(PATHS)} status={rng.choice([200, 200, 200, 201, 204])} "
            f"duration={rng.randint(1, 900)}ms request_id=req-{hexs(rng, 6)}")


class Gen:
    def __init__(self, seed):
        self.rng = random.Random(seed)
        self.sec = 1740787200  # 2025-03-01T00:00:00Z
        self.ms = 0
        self.cache = (None, "")

    def ts(self):
        self.ms += self.rng.randint(0, 40)
        if self.ms >= 1000:
            self.sec += self.ms // 1000
            self.ms %= 1000
        if self.cache[0] != self.sec:
            s = self.sec % 86400
            d = (self.sec - s) // 86400
            self.cache = (self.sec, f"{d}|{s // 3600:02d}:{s // 60 % 60:02d}:{s % 60:02d}")
        d, hms = self.cache[1].split("|")
        day = 1 + int(d) - 20148  # 2025-03-01 is day 20148 since epoch
        return day, hms

    def line(self):
        rng = self.rng
        day, hms = self.ts()
        stamp = f"2025-03-{day:02d}T{hms}.{self.ms:03d}Z"
        r = rng.random()
        if r < 0.55:
            return f"{stamp} INFO {info_msg(rng)}"
        if r < 0.70:
            return f"{stamp} DEBUG cache lookup key=user:{rng.randint(1, 5000)} hit={rng.choice(['true', 'false'])}"
        if r < 0.78:
            if rng.random() < 0.5:
                return f"[2025-03-{day:02d} {hms}] [WARNING] slow query {rng.randint(500, 3000)}ms table={rng.choice(TABLES)}"
            return f"{stamp} WARN retrying upstream attempt={rng.randint(1, 3)}"
        if r < 0.87:
            return f"{stamp} ERROR {error_msg(rng)}"
        if r < 0.93:
            lvl = rng.choices(["info", "warn", "error", "debug"], [50, 20, 25, 5])[0]
            msg = error_msg(rng) if lvl == "error" else info_msg(rng)
            key = rng.choice(["message", "msg"])
            tk = rng.choice(["timestamp", "time"])
            lk = rng.choice(["level", "severity"])
            return '{"%s":"%s","%s":"%s","%s":"%s","service":"api"}' % (tk, stamp, lk, lvl, key, msg.replace('"', "'"))
        if r < 0.94:
            return '{"level":"error","message":"truncated json'  # malformed NDJSON
        if r < 0.97:
            return f"    at com.acme.service.Handler.process(Handler.java:{rng.randint(10, 400)})"
        return f"free-form note {rng.randint(1, 10**6)} without a header"


def render(rng, text, i):
    eol = b"\r\n" if rng.random() < 0.02 else b"\n"
    if rng.random() < 0.0002:
        return b"2025-03-01T00:00:00.000Z INFO binary-ish payload \xff\xfe\xfa end" + eol
    return text.encode() + eol


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--lines", type=int)
    ap.add_argument("--size-bytes", type=int)
    a = ap.parse_args()
    if not (a.lines or a.size_bytes):
        ap.error("need --lines or --size-bytes")
    g = Gen(a.seed)
    written, i, next_long = 0, 0, 8_000_000
    with open(a.out, "wb") as f:
        while True:
            if a.lines and i >= a.lines:
                break
            if a.size_bytes and written >= a.size_bytes:
                break
            if a.size_bytes and written >= next_long:
                b = b"2025-03-01T00:00:00.000Z ERROR oversized payload dump " + b"x" * 70000 + b"\n"
                next_long += 40_000_000
            else:
                b = render(g.rng, g.line(), i)
            f.write(b)
            written += len(b)
            i += 1
        # strip the final newline so the last line has no terminator
        f.seek(-1, 1)
        f.truncate()
        # (also drops a possible \r of a CRLF; harmless)
    print(f"wrote {a.out}: {i} lines")


if __name__ == "__main__":
    main()
