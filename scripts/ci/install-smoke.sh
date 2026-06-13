#!/usr/bin/env bash
set -euo pipefail

cargo install --path . --locked --debug
skillroom >/dev/null
