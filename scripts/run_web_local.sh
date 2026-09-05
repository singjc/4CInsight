#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "${CFS_SKIP_WEB_BUILD:-0}" != "1" ]]; then
  echo "[4C] Building browser frontend with Trunk..."
  (
    cd "$ROOT/web"
    trunk build --release
  )
else
  echo "[4C] CFS_SKIP_WEB_BUILD=1; using existing web/dist assets"
fi

cd "$ROOT"
echo "[4C] Starting local Rust web server..."
exec cargo run --release --bin cfs-server
