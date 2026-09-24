#!/bin/sh
set -eu

report=${1:?report path required}
binary=${2:-target/release/ttc}
case "$binary" in
  /*) ;;
  *) binary="$(pwd)/$binary" ;;
esac
mkdir -p "$(dirname "$report")"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$scratch/home" "$scratch/state" "$scratch/tmp"

measure() {
  size=$1
  HOME="$scratch/home" XDG_STATE_HOME="$scratch/state" TMPDIR="$scratch/tmp" TTC_INTERNAL_TEST_TMP_ROOT="$scratch/tmp" \
    /usr/bin/time -f '%M' -o "$scratch/rss-$size" \
    "$binary" /bin/dd if=/dev/zero bs=1M count="$size" status=none \
    > /dev/null 2> "$scratch/stderr-$size"
  test ! -s "$scratch/stderr-$size"
  cat "$scratch/rss-$size"
}

small=$(measure 64)
large=$(measure 256)
delta=$((large - small))
test "$delta" -lt 16384
{
  echo "M3 release-mode peak RSS (KiB)"
  echo "64 MiB stream: $small"
  echo "256 MiB stream: $large"
  echo "Delta: $delta KiB (< 16384 KiB gate)"
  echo "No capture files: $(find "$scratch/state" "$scratch/tmp" -type f | wc -l)"
} > "$report"
test "$(find "$scratch/state" "$scratch/tmp" -type f | wc -l)" -eq 0
