#!/bin/sh
set -eu

report=${1:?report path required}
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$(dirname "$report")"
mkdir -p "$scratch/home" "$scratch/state" "$scratch/tmp"

script='i=0; while [ "$i" -lt 20000 ]; do printf "\377\000out\n"; printf "\376\000err\r\n" >&2; i=$((i+1)); done'
/bin/sh -c "$script" > "$scratch/baseline.stdout" 2> "$scratch/baseline.stderr"
HOME="$scratch/home" XDG_STATE_HOME="$scratch/state" TMPDIR="$scratch/tmp" \
  ./target/release/ttc /bin/sh -c "$script" > "$scratch/ttc.stdout" 2> "$scratch/ttc.stderr"
cmp "$scratch/baseline.stdout" "$scratch/ttc.stdout"
cmp "$scratch/baseline.stderr" "$scratch/ttc.stderr"

{
  printf 'M2 baseline versus TTC: byte-exact per stream\n'
  printf 'stdout bytes: '
  wc -c < "$scratch/ttc.stdout"
  printf 'stderr bytes: '
  wc -c < "$scratch/ttc.stderr"
  printf 'stdout cmp: identical\n'
  printf 'stderr cmp: identical\n'
} > "$report"
