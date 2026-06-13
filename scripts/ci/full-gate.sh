#!/usr/bin/env bash
set -euo pipefail

scripts/ci/fmt.sh
scripts/ci/clippy.sh
scripts/ci/nextest.sh
scripts/ci/snapshot.sh
scripts/ci/supply-chain.sh
scripts/ci/install-smoke.sh
scripts/ci/release-dry-run.sh
