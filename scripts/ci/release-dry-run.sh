#!/usr/bin/env bash
set -euo pipefail

cargo build --release --locked
./target/release/skillroom >/dev/null
