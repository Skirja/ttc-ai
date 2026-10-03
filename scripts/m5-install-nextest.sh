#!/bin/sh
set -eu

tool_home=${1:?tool install directory required}
version=0.9.108
archive="cargo-nextest-$version-x86_64-unknown-linux-gnu.tar.gz"
checksum=8fd2d441ac5f11d424bfba4f9887be8387a2871c6caae861a99b32dcb14ca13b
url="https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-$version/$archive"
mkdir -p "$tool_home"
scratch=$(mktemp -d "${TMPDIR:-/tmp}/ttc-nextest.XXXXXX")
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error --retry 3 \
  "$url" --output "$scratch/$archive"
printf '%s  %s\n' "$checksum" "$scratch/$archive" | sha256sum --check --status
tar -xzf "$scratch/$archive" -C "$scratch"
binary=$(find "$scratch" -type f -name cargo-nextest -print -quit)
test -n "$binary"
install -m 755 "$binary" "$tool_home/cargo-nextest"
"$tool_home/cargo-nextest" --version | grep -F "cargo-nextest $version" >/dev/null
printf 'Installed cargo-nextest %s (sha256 %s)\n' "$version" "$checksum"
