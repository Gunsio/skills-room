#!/usr/bin/env bash
set -euo pipefail

if command -v cargo-deny >/dev/null 2>&1; then
  cargo deny check
else
  echo "cargo-deny not installed; skipping license/dependency policy gate"
fi

if command -v cargo-audit >/dev/null 2>&1; then
  cargo audit
else
  echo "cargo-audit not installed; skipping advisory database gate"
fi
