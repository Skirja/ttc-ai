#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tool_root=${TTC_M10_TOOL_HOME:-$repository_root/target/m10-tools}
install_root=$tool_root/codex-0.160.0
archive=$tool_root/codex-0.160.0.tar.gz
expected=4fcc47ab57f52ff75363951a8761146cd10c8288bd86fed45487dbb204a16b71

mkdir -p "$tool_root"
if [ ! -f "$archive" ]; then
  curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
    'https://github.com/openai/codex/releases/download/rust-v0.160.0/codex-package-x86_64-unknown-linux-musl.tar.gz' \
    --output "$archive"
fi
printf '%s  %s\n' "$expected" "$archive" | sha256sum --check --status \
  || { rm -f "$archive"; echo 'Codex 0.160.0 checksum mismatch' >&2; exit 1; }

stage=$(mktemp -d "$tool_root/.codex-0.160.0.XXXXXX")
trap 'rm -rf "$stage"' EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
tar -xzf "$archive" -C "$stage"
test "$("$stage/bin/codex" --version)" = 'codex-cli 0.160.0'
rm -rf "$install_root"
mv "$stage" "$install_root"
trap - EXIT HUP INT TERM
printf '%s\n' "$install_root/bin/codex"
