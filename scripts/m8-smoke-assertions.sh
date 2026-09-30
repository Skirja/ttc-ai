#!/bin/sh
# Sourced by the real-tool smoke and its isolated regression tests.

count_build_records() {
  awk 'index($0, "Building ") || index($0, "Linking ") { count++ } END { print count + 0 }' "$1"
}

assert_build_records_compacted() {
  label=$1
  direct_count=$(count_build_records "$scratch/$label-direct.out")
  ttc_count=$(count_build_records "$scratch/$label-ttc.out")
  test "$direct_count" -ge 4 && test "$ttc_count" -ge 1 && test "$ttc_count" -lt "$direct_count" || {
    printf '%s expected build records to compact: direct=%s TTC=%s\n' "$label" "$direct_count" "$ttc_count" >&2
    return 1
  }
}

assert_ctest_records_compacted() {
  label=$1
  direct_count=$(grep -c ' Test #' "$scratch/$label-direct.out" || true)
  ttc_count=$(grep -c ' Test #' "$scratch/$label-ttc.out" || true)
  test "$direct_count" -ge 8 && test "$ttc_count" -eq 3 || {
    printf '%s expected eight CTest rows direct and three retained by TTC: direct=%s TTC=%s\n' "$label" "$direct_count" "$ttc_count" >&2
    return 1
  }
}

assert_dot_meter_compacted() {
  label=$1
  grep -Eq '^\.{4,}$' "$scratch/$label-direct.out" || {
    printf '%s direct output did not contain a multi-record progress meter\n' "$label" >&2
    return 1
  }
  if grep -Eq '^\.{4,}$' "$scratch/$label-ttc.out"; then
    printf '%s TTC output retained a progress-only meter\n' "$label" >&2
    return 1
  fi
  awk '
    /^TTC: [0-9]+ passing records and [0-9]+ progress records compacted$/ && ($2 > 0 || $6 > 0) { found = 1 }
    END { exit !found }
  ' "$scratch/$label-ttc.err" || {
    printf '%s TTC did not report compacted passing or progress records\n' "$label" >&2
    return 1
  }
}
