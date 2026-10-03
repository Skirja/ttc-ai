#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tool_directory="$repository_root/target/m10-tools/codex-0.154.0/bin"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
archive=codex-x86_64-unknown-linux-musl.tar.gz
checksum=d7e18b2597ae8f242f5f31ee9e90deef48dbc9edd634d9868fb6435d08c07f02
curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
  "https://github.com/openai/codex/releases/download/rust-v0.154.0/$archive" -o "$scratch/$archive"
printf '%s  %s\n' "$checksum" "$scratch/$archive" | sha256sum --check --status
mkdir -p "$tool_directory"
tar -xzf "$scratch/$archive" -C "$scratch"
install -m 755 "$scratch/codex-x86_64-unknown-linux-musl" "$tool_directory/codex"
test "$("$tool_directory/codex" --version)" = 'codex-cli 0.154.0'
printf 'Installed pinned Codex 0.154.0 (archive sha256 %s)\n' "$checksum"
