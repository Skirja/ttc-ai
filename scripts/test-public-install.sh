#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec python3 -B "$repository_root/scripts/m10-public-install.py"
