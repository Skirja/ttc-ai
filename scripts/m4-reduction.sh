#!/bin/sh
set -eu
report=${1:-target/m4-evidence/reduction.txt}
mkdir -p "$(dirname "$report")"
cargo test --test javascript_fixtures -- --nocapture > "$report" 2>&1
