#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tool_root=${TTC_M10_TOOL_HOME:-$repository_root/target/m10-tools}
install_root="$tool_root/codex-0.154.0"
archive=codex-package-x86_64-unknown-linux-musl.tar.gz
checksum=fc6e3e3b85f2cf7d664520ee5c66a7fe4aa12bae7d46834f47e2f165fd0d6f78

mkdir -p "$tool_root"
if [ -x "$install_root/bin/codex" ] && [ -x "$install_root/codex-resources/bwrap" ] \
  && [ "$("$install_root/bin/codex" --version)" = 'codex-cli 0.154.0' ]; then
  printf 'Pinned Codex 0.154.0 package already installed\n'
  exit 0
fi

scratch=$(mktemp -d "$tool_root/.codex-0.154.0.XXXXXX")
backup="$tool_root/.codex-0.154.0-backup-$$"
cleanup() {
  if [ -e "$backup" ]; then
    if [ -e "$install_root" ]; then
      rm -rf "$backup"
    else
      mv "$backup" "$install_root"
    fi
  fi
  rm -rf "$scratch"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
  "https://github.com/openai/codex/releases/download/rust-v0.154.0/$archive" -o "$scratch/$archive"
printf '%s  %s\n' "$checksum" "$scratch/$archive" | sha256sum --check --status
mkdir "$scratch/extracted"
tar -xzf "$scratch/$archive" -C "$scratch/extracted"
test -x "$scratch/extracted/bin/codex"
test -x "$scratch/extracted/codex-resources/bwrap"
test "$("$scratch/extracted/bin/codex" --version)" = 'codex-cli 0.154.0'

if [ -e "$install_root" ]; then
  mv "$install_root" "$backup"
fi
if mv "$scratch/extracted" "$install_root"; then
  if [ -e "$backup" ]; then
    rm -rf "$backup"
  fi
else
  if [ -e "$backup" ]; then
    mv "$backup" "$install_root"
  fi
  exit 1
fi
printf 'Installed pinned Codex 0.154.0 package (archive sha256 %s)\n' "$checksum"
