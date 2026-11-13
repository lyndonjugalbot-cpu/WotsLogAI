| Dataset | Operation | Outputs match | Rust median ms | Python median ms | Rust peak MiB | Python peak MiB | Speed ratio (Py/Rust) |
|---|---|---|---|---|---|---|---|
| 10 MB (10,000,103 bytes) | scan (parse + severity counts) | yes | 72 | 228 | 7.5 | 12.7 | 3.2x |
| 10 MB (10,000,103 bytes) | search 'timeout' (all levels) | yes | 18 | 102 | 6.5 | 12.5 | 5.8x |
| 10 MB (10,000,103 bytes) | search 'payment' + ERROR filter | yes | 21 | 111 | 7.5 | 12.3 | 5.2x |
| 10 MB (10,000,103 bytes) | scan + error grouping | yes | 92 | 300 | 8.4 | 13.2 | 3.3x |
| 100 MB (100,000,007 bytes) | scan (parse + severity counts) | yes | 664 | 1942 | 7.5 | 13.2 | 2.9x |
| 100 MB (100,000,007 bytes) | search 'timeout' (all levels) | yes | 95 | 781 | 6.5 | 12.8 | 8.2x |
| 100 MB (100,000,007 bytes) | search 'payment' + ERROR filter | yes | 118 | 810 | 7.5 | 13.0 | 6.9x |
| 100 MB (100,000,007 bytes) | scan + error grouping | yes | 820 | 2821 | 10.5 | 16.8 | 3.4x |

{
  "machine": {
    "os": "macOS-26.6.2-arm64-arm-64bit-Mach-O",
    "python": "3.14.6",
    "rustc": "rustc 1.98.0 (88d9e12ae 2026-08-18)",
    "cpu": "Apple M4",
    "ram_gib": 16.0
  },
  "runs": 5,
  "warmups": 1,
  "timestamp": "2026-11-13 20:53:10",
  "allOutputsMatch": true
}
