#!/bin/sh
set -eu

binary=${1:-target/release/ttc}
report=${2:-target/m4-evidence/report.txt}
case "$binary" in /*) ;; *) binary="$(pwd)/$binary" ;; esac
case "$report" in /*) ;; *) report="$(pwd)/$report" ;; esac
mkdir -p "$(dirname "$report")"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$scratch/home" "$scratch/state" "$scratch/tmp" "$scratch/tools" "$scratch/config" "$scratch/data" "$scratch/npm-cache"
cp -R tests/fixtures/javascript-real "$scratch/work"
export HOME="$scratch/home" XDG_STATE_HOME="$scratch/state" TMPDIR="$scratch/tmp"
export XDG_CONFIG_HOME="$scratch/config" XDG_DATA_HOME="$scratch/data"
export npm_config_cache="$scratch/npm-cache" npm_config_userconfig="$scratch/config/npmrc"
export TTC_INTERNAL_TEST_TMP_ROOT="$scratch/tmp"

npm install --prefix "$scratch/tools" --no-save --no-package-lock --no-audit --no-fund \
  pnpm@9.15.9 yarn@1.22.22 bun@1.4.2 > "$scratch/install-tools.log" 2>&1
export PATH="$scratch/tools/node_modules/.bin:$PATH"

test "$(node --version)" = v24.21.0
test "$(npm --version)" = 11.19.0
test "$(pnpm --version)" = 9.15.9
test "$(yarn --version)" = 1.22.22
test "$(bun --version)" = 1.4.2

cd "$scratch/work"
npm ci --no-audit --no-fund > "$scratch/npm-ci.log" 2>&1
cat > tests/vitest/many.test.mjs <<'EOF'
import { test, expect } from 'vitest';
for (let index = 0; index < 100; index++) {
  test(`generated passing case ${index}`, () => expect(index).toBe(index));
}
EOF

{
  printf 'M4 real-tool smoke\n'
  printf 'node=%s npm=%s pnpm=%s yarn=%s bun=%s\n' "$(node --version)" "$(npm --version)" "$(pnpm --version)" "$(yarn --version)" "$(bun --version)"
  printf 'vitest=5.0.1 jest=30.5.2 eslint=10.11.0 typescript=7.0.2 vite=8.3.1 prettier=3.9.9\n'
} > "$report"

compare() {
  label=$1
  shift
  set +e
  "$@" > "$scratch/$label-direct.out" 2> "$scratch/$label-direct.err"
  direct_status=$?
  "$binary" "$@" > "$scratch/$label-ttc.out" 2> "$scratch/$label-ttc.err"
  ttc_status=$?
  set -e
  test "$direct_status" -eq "$ttc_status"
  direct_bytes=$(wc -c < "$scratch/$label-direct.out")
  direct_errors=$(wc -c < "$scratch/$label-direct.err")
  ttc_bytes=$(wc -c < "$scratch/$label-ttc.out")
  ttc_errors=$(wc -c < "$scratch/$label-ttc.err")
  printf '%s exit=%s direct_stdout=%s direct_stderr=%s ttc_stdout=%s ttc_stderr=%s\n' \
    "$label" "$direct_status" "$direct_bytes" "$direct_errors" "$ttc_bytes" "$ttc_errors" >> "$report"
}

compare vitest npm run test:vitest
compare jest pnpm run test:jest
compare lint yarn run lint
compare typecheck bun run typecheck
compare build npm run build
compare format npm run format:check
compare install npm ci --no-audit --no-fund

cat > tests/vitest/failure.test.mjs <<'EOF'
import { test, expect } from 'vitest';
test('real failure', () => expect(1).toBe(2));
EOF
compare failure ./node_modules/.bin/vitest run tests/vitest/failure.test.mjs
test -s "$scratch/failure-ttc.err" || test -s "$scratch/failure-ttc.out"
grep -Eiq 'expected|assertion' "$scratch/failure-direct.out" "$scratch/failure-direct.err"
grep -Eiq 'expected|assertion' "$scratch/failure-ttc.out" "$scratch/failure-ttc.err"
direct_total=$(($(wc -c < "$scratch/vitest-direct.out") + $(wc -c < "$scratch/vitest-direct.err")))
ttc_total=$(($(wc -c < "$scratch/vitest-ttc.out") + $(wc -c < "$scratch/vitest-ttc.err")))
test "$((ttc_total * 5))" -lt "$direct_total"
