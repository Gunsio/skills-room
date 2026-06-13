#!/usr/bin/env bash
set -euo pipefail

echo "snapshot gate placeholder: M1 introduces ratatui snapshots"
cargo test --all-targets --all-features
