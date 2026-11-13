# Source before building: keeps all caches/targets off the (nearly full) internal disk.
R="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
export CARGO_HOME="$R/.cache/cargo"
export CARGO_TARGET_DIR="$R/.cache/target"
export npm_config_cache="$R/.cache/npm"
export TMPDIR="$R/.cache/tmp"; mkdir -p "$TMPDIR"
export CARGO_INCREMENTAL=0
