#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
codex_binary=$(sh "$repository_root/scripts/m10-install-auth-codex.sh")
TTC_M10_AUTH_CODEX=$codex_binary exec python3 -B "$repository_root/scripts/m10-e2e.py" "$@"
