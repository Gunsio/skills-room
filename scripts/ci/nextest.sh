#!/usr/bin/env bash
set -euo pipefail

if command -v cargo-nextest >/dev/null 2>&1; then
  cargo nextest run --all-targets --all-features
else
  echo "cargo-nextest not installed; falling back to cargo test"
  cargo test --all-targets --all-features
fi
