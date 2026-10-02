#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec python3 "$repository_root/scripts/m9-artifact-tests.py" "${TTC_M9_BINARY:-$repository_root/target/x86_64-unknown-linux-gnu/release/ttc}"
