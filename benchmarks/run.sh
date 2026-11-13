#!/usr/bin/env bash
# Usage: benchmarks/run.sh [--sizes 10 100] [--runs 5]
set -euo pipefail
cd "$(dirname "$0")/.."
exec python3 benchmarks/run_bench.py "$@"
