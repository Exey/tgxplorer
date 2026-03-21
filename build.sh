#!/usr/bin/env bash
set -euo pipefail

echo "==> Building tgxplorer (release)…"
cargo build --release

BIN="target/release/tgxplorer"

if [ ! -f "$BIN" ]; then
  echo "ERROR: build failed, binary not found at $BIN"
  exit 1
fi

echo ""
echo "==> Build complete: $BIN"
echo ""
echo "Run:"
echo "  ./target/release/tgxplorer                  # then use Open JSON… button"
echo "  ./target/release/tgxplorer path/to/result.json   # open file directly"
