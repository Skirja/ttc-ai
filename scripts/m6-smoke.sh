#!/bin/sh
set -eu

binary=${1:-target/release/ttc}
report_arg=${2:-target/m6-evidence/report.txt}
group=${3:-all}
case "$group" in all|package-managers|runners|systems) ;; *) printf 'unknown M6 smoke group: %s\n' "$group" >&2; exit 2 ;; esac
case "$binary" in /*) ;; *) binary="$(pwd)/$binary" ;; esac
case "$report_arg" in /*) ;; *) report_arg="$(pwd)/$report_arg" ;; esac
report_base=${report_arg##*/}
case "$report_base" in
  *.*) if [ -d "$report_arg" ]; then report="$report_arg/report.txt"; else report="$report_arg"; fi ;;
  *) report="$report_arg/report.txt" ;;
esac
mkdir -p "$(dirname "$report")"
scratch=$(mktemp -d)
cleanup() {
  status=$?
  if [ "$status" -ne 0 ]; then
    for log in "$scratch"/*.log; do
      [ -f "$log" ] || continue
      printf '\n--- %s ---\n' "$(basename "$log")" >&2
      tail -n 35 "$log" >&2
    done
    for output in "$scratch"/*.direct.out "$scratch"/*.direct.err "$scratch"/*.ttc.out "$scratch"/*.ttc.err "$scratch"/*-trace "$scratch"/direct-marker "$scratch"/ttc-marker; do
      [ -f "$output" ] || continue
      printf '\n--- %s ---\n' "$(basename "$output")" >&2
      tail -n 35 "$output" >&2
    done
    if [ "${M6_KEEP_SCRATCH:-0}" = 1 ]; then
      printf '\nM6_KEEP_SCRATCH retained %s\n' "$scratch" >&2
      exit "$status"
    fi
  fi
  rm -rf "$scratch"
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
tools="$scratch/tools"
classic="$scratch/yarn-classic"
modern="$scratch/yarn-modern"
work="$scratch/work"
rustup_home=$(rustup show home)
mkdir -p "$tools" "$classic" "$modern" "$work" "$scratch/home" "$scratch/config" "$scratch/state" "$scratch/cache" "$scratch/tmp"
export HOME="$scratch/home" XDG_CONFIG_HOME="$scratch/config" XDG_STATE_HOME="$scratch/state" XDG_CACHE_HOME="$scratch/cache" TMPDIR="$scratch/tmp" RUSTUP_HOME="$rustup_home"
export CARGO_HOME="$scratch/home/.cargo" PIP_CACHE_DIR="$scratch/cache/pip" GOCACHE="$scratch/cache/go-build" GOMODCACHE="$scratch/cache/go-mod"
export npm_config_cache="$scratch/cache/npm" npm_config_userconfig="$scratch/config/npmrc" npm_config_fetch_timeout=60000 npm_config_fetch_retries=1
export TURBO_TELEMETRY_DISABLED=1 NX_DAEMON=false YARN_ENABLE_TELEMETRY=0
unset NO_COLOR

node_version=$(node --version)
npm_version=$(npm --version)
commit_sha=$(git rev-parse HEAD)
if [ -n "$(git status --porcelain)" ]; then working_tree=dirty; else working_tree=clean; fi
test "$node_version" = "v24.21.0"
test "$npm_version" = "11.19.0"
cat > "$tools/package.json" <<'JSON'
{"name":"ttc-m6-smoke-tools","private":true,"allowScripts":{"bun":true,"nx":true}}
JSON
npm install --prefix "$tools" --no-save --no-package-lock --no-audit --no-fund --legacy-peer-deps \
  pnpm@9.15.9 bun@1.4.2 turbo@2.11.4 nx@23.2.1 lerna@10.0.1 lage@2.17.0 \
  @moonrepo/cli@2.5.5 vitest@5.0.1 vite@8.3.1 > "$scratch/install-runners.log" 2>&1
npm install --prefix "$classic" --no-save --no-package-lock --no-audit --no-fund yarn@1.22.22 > "$scratch/install-yarn-classic.log" 2>&1
npm install --prefix "$modern" --no-save --no-package-lock --no-audit --no-fund @yarnpkg/cli-dist@4.18.1 > "$scratch/install-yarn-modern.log" 2>&1

mkdir -p "$work/packages/api/tests" "$work/packages/web/tests" "$work/packages/shared/tests" "$work/.moon/tasks"
cat > "$work/package.json" <<'JSON'
{"name":"m6-smoke-root","private":true,"workspaces":["packages/*"]}
JSON
cat > "$work/pnpm-workspace.yaml" <<'YAML'
packages:
  - packages/*
YAML
cat > "$work/turbo.json" <<'JSON'
{"tasks":{"test":{"dependsOn":["^test"],"cache":false}},"globalPassThroughEnv":["M6_MARKER","M6_FAIL_TEST","M6_TRACE"]}
JSON
cat > "$work/nx.json" <<'JSON'
{"targetDefaults":{"test":{"cache":false}}}
JSON
cat > "$work/lerna.json" <<'JSON'
{"version":"independent"}
JSON
cat > "$work/lage.config.js" <<'JS'
module.exports = { pipeline: { test: [] } };
JS
cat > "$work/.moon/workspace.yml" <<'YAML'
projects:
  api: packages/api
  web: packages/web
  shared: packages/shared
YAML
cat > "$work/.moon/tasks/all.yml" <<'YAML'
tasks:
  test:
    command: vitest
    args:
      - run
      - --reporter=verbose
YAML

for package in api web shared; do
  case "$package" in api) test_name=API; dependency='"dependencies":{"@m6/shared":"workspace:*"},' ;; web) test_name=Web; dependency='' ;; shared) test_name=Shared; dependency='' ;; esac
  cat > "$work/packages/$package/package.json" <<JSON
{"name":"@m6/$package","version":"1.0.0",$dependency"scripts":{"test":"vitest run --reporter=verbose"}}
JSON
  cat > "$work/packages/$package/project.json" <<JSON
{"targets":{"test":{"executor":"nx:run-commands","options":{"command":"vitest run packages/$package/tests --reporter=verbose"}}}}
JSON
  cat > "$work/packages/$package/tests/case.test.js" <<JS
import { beforeAll, expect, test } from 'vitest';
import { appendFileSync } from 'node:fs';
beforeAll(async () => {
  appendFileSync(process.env.M6_MARKER, '$package\\n');
  if (process.env.M6_TRACE) {
    appendFileSync(process.env.M6_TRACE, '$package start ' + Date.now() + '\\n');
    await new Promise(resolve => setTimeout(resolve, 200));
    appendFileSync(process.env.M6_TRACE, '$package end ' + Date.now() + '\\n');
  }
});
for (let index = 0; index < 8; index++) {
  test('$package passing case ' + index, () => {
    if (process.env.M6_FAIL_TEST === '$package' && index === 0) throw new Error('M6 retained failure from $package');
    expect(index + 1).toBeGreaterThan(0);
  });
}
JS
done
cat > "$work/.yarnrc.yml" <<'YAML'
nodeLinker: node-modules
enableTelemetry: false
YAML
(cd "$work" && YARN_ENABLE_IMMUTABLE_INSTALLS=false "$modern/node_modules/.bin/yarn" install --mode=skip-build) > "$scratch/install-yarn-workspace.log" 2>&1
mkdir -p "$work/node_modules"
ln -s "$tools/node_modules/nx" "$work/node_modules/nx"
export PATH="$modern/node_modules/.bin:$classic/node_modules/.bin:$tools/node_modules/.bin:$PATH"

check_tool_version() {
  tool=$1
  expected=$2
  version_dir=${3:-.}
  actual=$(cd "$version_dir" && "$tool" --version | tr -d '\r')
  case "$actual" in *"$expected"*) ;; *) printf 'expected %s version %s; received %s\n' "$tool" "$expected" "$actual" >&2; return 1 ;; esac
}
check_tool_version "$tools/node_modules/.bin/pnpm" 9.15.9
check_tool_version "$tools/node_modules/.bin/bun" 1.4.2
check_tool_version "$tools/node_modules/.bin/turbo" 2.11.4
check_tool_version "$tools/node_modules/.bin/nx" 23.2.1
check_tool_version "$tools/node_modules/.bin/lerna" 10.0.1
test "$(node -p "require('$tools/node_modules/lage/package.json').version")" = "2.17.0"
check_tool_version "$tools/node_modules/.bin/moon" 2.5.5
check_tool_version "$classic/node_modules/.bin/yarn" 1.22.22
check_tool_version "$modern/node_modules/.bin/yarn" 4.18.1

printf 'M6 package manager and monorepo smoke\ncommit_sha=%s working_tree=%s\nverification_command=sh scripts/m6-smoke.sh %s %s %s\nnode=%s npm=%s pnpm=9.15.9 bun=1.4.2 yarn-classic=1.22.22 yarn-modern=4.18.1\n' \
  "$commit_sha" "$working_tree" "$binary" "$report" "$group" "$node_version" "$npm_version" > "$report"
printf 'turbo=2.11.4 nx=23.2.1 lerna=10.0.1 lage=2.17.0 moon=2.5.5 vitest=5.0.1 vite=8.3.1\n' >> "$report"

run_case() {
  mode=$1
  shift
  case "$mode" in
    npm) npm "$@" ;;
    yarn-classic) CI= "$classic/node_modules/.bin/yarn" "$@" ;;
    yarn-modern) CI= "$modern/node_modules/.bin/yarn" "$@" ;;
    *) "$tools/node_modules/.bin/$mode" "$@" ;;
  esac
}

compare() {
  label=$1
  mode=$2
  expectation=$3
  shift 3
  wrapper_name=$mode
  case "$mode" in yarn-classic|yarn-modern) wrapper_name=yarn ;; esac
  command_text="$wrapper_name $*"
  rm -f "$scratch/direct-marker" "$scratch/ttc-marker"
  set +e
  if [ "$mode" = moon ] && [ "$expectation" != cached ]; then
    (cd "$work" && MOON_CACHE=off M6_MARKER="$scratch/direct-marker" run_case "$mode" "$@") > "$scratch/$label.direct.out" 2> "$scratch/$label.direct.err"
  else
    (cd "$work" && M6_MARKER="$scratch/direct-marker" run_case "$mode" "$@") > "$scratch/$label.direct.out" 2> "$scratch/$label.direct.err"
  fi
  direct_status=$?
  case "$mode" in
    yarn-classic) (cd "$work" && CI= PATH="$classic/node_modules/.bin:$PATH" M6_MARKER="$scratch/ttc-marker" "$binary" "$command_text") > "$scratch/$label.ttc.out" 2> "$scratch/$label.ttc.err" ;;
    yarn-modern) (cd "$work" && CI= PATH="$modern/node_modules/.bin:$PATH" M6_MARKER="$scratch/ttc-marker" "$binary" "$command_text") > "$scratch/$label.ttc.out" 2> "$scratch/$label.ttc.err" ;;
    moon) if [ "$expectation" = cached ]; then (cd "$work" && M6_MARKER="$scratch/ttc-marker" "$binary" "$command_text") > "$scratch/$label.ttc.out" 2> "$scratch/$label.ttc.err"; else (cd "$work" && MOON_CACHE=off M6_MARKER="$scratch/ttc-marker" "$binary" "$command_text") > "$scratch/$label.ttc.out" 2> "$scratch/$label.ttc.err"; fi ;;
    *) (cd "$work" && M6_MARKER="$scratch/ttc-marker" "$binary" "$command_text") > "$scratch/$label.ttc.out" 2> "$scratch/$label.ttc.err" ;;
  esac
  ttc_status=$?
  set -e
  test "$direct_status" -eq "$ttc_status"
  expected_direct_count=3
  expected_ttc_count=3
  if [ "$expectation" = api ]; then expected_direct_count=1; expected_ttc_count=1; fi
  if [ "$expectation" = cached ]; then expected_ttc_count=0; fi
  direct_marker_count=0
  ttc_marker_count=0
  if [ -f "$scratch/direct-marker" ]; then direct_marker_count=$(wc -l < "$scratch/direct-marker" | tr -d ' '); fi
  if [ -f "$scratch/ttc-marker" ]; then ttc_marker_count=$(wc -l < "$scratch/ttc-marker" | tr -d ' '); fi
  test "$direct_marker_count" -eq "$expected_direct_count"
  test "$ttc_marker_count" -eq "$expected_ttc_count"
  if [ "$expectation" = api ]; then test "$(cat "$scratch/direct-marker")" = api; test "$(cat "$scratch/ttc-marker")" = api; fi
  direct_bytes=$(wc -c < "$scratch/$label.direct.out" | tr -d ' ')
  ttc_bytes=$(wc -c < "$scratch/$label.ttc.out" | tr -d ' ')
  direct_stderr_bytes=$(wc -c < "$scratch/$label.direct.err" | tr -d ' ')
  ttc_stderr_bytes=$(wc -c < "$scratch/$label.ttc.err" | tr -d ' ')
  direct_total=$((direct_bytes + direct_stderr_bytes))
  ttc_total=$((ttc_bytes + ttc_stderr_bytes))
  reduction_percent=$(awk -v direct="$direct_total" -v ttc="$ttc_total" 'BEGIN { if (direct == 0) print "0.0"; else printf "%.1f", (direct-ttc)*100/direct }')
  raw_id=$(sed -n 's/^raw: ttc raw //p' "$scratch/$label.ttc.err" | sed -n '1p')
  replay_stdout_sha=none
  replay_stderr_sha=none
  if [ -n "$raw_id" ]; then
    "$binary" raw "$raw_id" --stdout > "$scratch/$label.replay.stdout"
    "$binary" raw "$raw_id" --stderr > "$scratch/$label.replay.stderr"
    replay_stdout_sha=$(sha256sum "$scratch/$label.replay.stdout" | awk '{print $1}')
    replay_stderr_sha=$(sha256sum "$scratch/$label.replay.stderr" | awk '{print $1}')
  fi
  if [ "$expectation" = raw ]; then
    test -z "$raw_id"
    grep ' passing case ' "$scratch/$label.direct.out" | normalize_case_records | sort > "$scratch/$label.direct.tests"
    grep ' passing case ' "$scratch/$label.ttc.out" | normalize_case_records | sort > "$scratch/$label.ttc.tests"
    test "$(wc -l < "$scratch/$label.direct.tests" | tr -d ' ')" -eq 24
    cmp "$scratch/$label.direct.tests" "$scratch/$label.ttc.tests"
    grep -E 'Test Files|Tests[[:space:]]' "$scratch/$label.direct.out" | sort > "$scratch/$label.direct.summaries"
    grep -E 'Test Files|Tests[[:space:]]' "$scratch/$label.ttc.out" | sort > "$scratch/$label.ttc.summaries"
    cmp "$scratch/$label.direct.summaries" "$scratch/$label.ttc.summaries"
    cmp "$scratch/$label.direct.err" "$scratch/$label.ttc.err"
  fi
  printf '%s command=%s exit=%s direct_stdout=%s ttc_stdout=%s direct_stderr=%s ttc_stderr=%s direct_total=%s ttc_total=%s reduction_percent=%s direct_stdout_sha256=%s direct_stderr_sha256=%s raw_id=%s replay_stdout_sha256=%s replay_stderr_sha256=%s direct_marker_count=%s ttc_marker_count=%s\n' \
    "$label" "$command_text" "$ttc_status" "$direct_bytes" "$ttc_bytes" \
    "$direct_stderr_bytes" "$ttc_stderr_bytes" "$direct_total" "$ttc_total" "$reduction_percent" \
    "$(sha256sum "$scratch/$label.direct.out" | awk '{print $1}')" \
    "$(sha256sum "$scratch/$label.direct.err" | awk '{print $1}')" \
    "${raw_id:-none}" "$replay_stdout_sha" "$replay_stderr_sha" \
    "$direct_marker_count" "$ttc_marker_count" >> "$report"
}

raw_replay_checksums() {
  label=$1
  error_file=$2
  raw_id=$(sed -n 's/^raw: ttc raw //p' "$error_file" | sed -n '1p')
  stdout_sha=none
  stderr_sha=none
  if [ -n "$raw_id" ]; then
    "$binary" raw "$raw_id" --stdout > "$scratch/$label.replay.stdout"
    "$binary" raw "$raw_id" --stderr > "$scratch/$label.replay.stderr"
    stdout_sha=$(sha256sum "$scratch/$label.replay.stdout" | awk '{print $1}')
    stderr_sha=$(sha256sum "$scratch/$label.replay.stderr" | awk '{print $1}')
  fi
  printf 'raw-replay label=%s id=%s stdout_sha256=%s stderr_sha256=%s\n' \
    "$label" "${raw_id:-none}" "$stdout_sha" "$stderr_sha" >> "$report"
}

assert_reduced() {
  direct_total=$(($(wc -c < "$1") + $(wc -c < "$2")))
  ttc_total=$(($(wc -c < "$3") + $(wc -c < "$4")))
  test "$ttc_total" -lt "$direct_total"
}

normalize_case_records() {
  node -e '
let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", chunk => input += chunk);
process.stdin.on("end", () => {
  input = input.replace(/\x1b\[[0-9;]*m/g, "");
  input = input.replace(/[ \t]+[0-9]+(?:\.[0-9]+)?ms\r?$/gm, "");
  process.stdout.write(input);
});'
}

report_bytes() {
  label=$1
  command_text=$2
  status=$3
  direct_out=$4
  direct_err=$5
  ttc_out=$6
  ttc_err=$7
  direct_stdout=$(wc -c < "$direct_out" | tr -d ' ')
  direct_stderr=$(wc -c < "$direct_err" | tr -d ' ')
  ttc_stdout=$(wc -c < "$ttc_out" | tr -d ' ')
  ttc_stderr=$(wc -c < "$ttc_err" | tr -d ' ')
  direct_total=$((direct_stdout + direct_stderr))
  ttc_total=$((ttc_stdout + ttc_stderr))
  reduction=$(awk -v direct="$direct_total" -v ttc="$ttc_total" 'BEGIN { if (direct == 0) print "0.0"; else printf "%.1f", (direct-ttc)*100/direct }')
  printf '%s command=%s exit=%s direct_stdout=%s ttc_stdout=%s direct_stderr=%s ttc_stderr=%s direct_total=%s ttc_total=%s reduction_percent=%s direct_stdout_sha256=%s direct_stderr_sha256=%s\n' \
    "$label" "$command_text" "$status" "$direct_stdout" "$ttc_stdout" "$direct_stderr" "$ttc_stderr" "$direct_total" "$ttc_total" "$reduction" \
    "$(sha256sum "$direct_out" | awk '{print $1}')" "$(sha256sum "$direct_err" | awk '{print $1}')" >> "$report"
}

if [ "$group" = all ] || [ "$group" = package-managers ]; then
  compare npm-workspaces npm raw run test --workspaces --if-present
  compare npm-selected npm api run test --workspace @m6/api
  compare pnpm-recursive pnpm all -r test
  compare pnpm-filter pnpm api --filter @m6/api test
  compare yarn-classic-workspace yarn-classic api workspace @m6/api run test
  compare yarn-classic-workspaces yarn-classic raw workspaces run test
  compare yarn-modern-foreach yarn-modern raw workspaces foreach -A run test
  compare bun-filter bun api run --filter @m6/api test
fi
if [ "$group" = all ] || [ "$group" = runners ]; then
  node -e "const fs=require('node:fs');const p=JSON.parse(fs.readFileSync('$work/package.json'));p.packageManager='pnpm@9.15.9';fs.writeFileSync('$work/package.json',JSON.stringify(p)+'\\n')"
  (cd "$work" && "$tools/node_modules/.bin/pnpm" install --lockfile-only --ignore-scripts --offline) > "$scratch/install-pnpm-lock.log" 2>&1
  compare turbo turbo all run test --concurrency=2
  rm -f "$scratch/direct-marker" "$scratch/ttc-marker" "$scratch/direct-trace" "$scratch/ttc-trace"
  set +e
  (cd "$work" && M6_MARKER="$scratch/direct-marker" M6_TRACE="$scratch/direct-trace" "$tools/node_modules/.bin/turbo" run test --concurrency=2) > "$scratch/concurrency.direct.out" 2> "$scratch/concurrency.direct.err"
  direct_status=$?
  (cd "$work" && M6_MARKER="$scratch/ttc-marker" M6_TRACE="$scratch/ttc-trace" "$binary" "turbo run test --concurrency=2") > "$scratch/concurrency.ttc.out" 2> "$scratch/concurrency.ttc.err"
  ttc_status=$?
  set -e
  test "$direct_status" -eq "$ttc_status"
  test "$(wc -l < "$scratch/direct-marker" | tr -d ' ')" -eq 3
  test "$(wc -l < "$scratch/ttc-marker" | tr -d ' ')" -eq 3
  for marker in "$scratch/direct-marker" "$scratch/ttc-marker"; do
    for package in api web shared; do
      test "$(grep -cx "$package" "$marker")" -eq 1
    done
  done
  for trace in "$scratch/direct-trace" "$scratch/ttc-trace"; do
    node - "$trace" <<'NODE'
const fs = require('node:fs');
const events = fs.readFileSync(process.argv[2], 'utf8').trim().split('\n').map(line => {
  const [project, event, timestamp] = line.split(' ');
  return { project, event, timestamp: Number(timestamp) };
});
const intervals = new Map();
for (const event of events) {
  const interval = intervals.get(event.project) ?? {};
  interval[event.event] = event.timestamp;
  intervals.set(event.project, interval);
}
const tasks = [...intervals.values()];
const byProject = Object.fromEntries(intervals);
let overlap = false;
for (let left = 0; left < tasks.length; left++) {
  for (let right = left + 1; right < tasks.length; right++) {
    if (Math.max(tasks[left].start, tasks[right].start) < Math.min(tasks[left].end, tasks[right].end)) overlap = true;
  }
}
if (tasks.length !== 3 || [...intervals.values()].some(task => task.start == null || task.end == null) || !overlap) process.exit(1);
if (byProject.shared.end > byProject.api.start) process.exit(1);
NODE
  done
  printf 'turbo-concurrency exit=%s direct=overlap ttc=overlap dependency_order=shared-completed-before-api\n' "$ttc_status" >> "$report"
  node -e "const fs=require('node:fs');const p=JSON.parse(fs.readFileSync('$work/package.json'));delete p.packageManager;fs.writeFileSync('$work/package.json',JSON.stringify(p)+'\\n')"
  compare nx nx all run-many -t test --output-style=stream
  compare lerna lerna all run test
  printf '%s\n' 'node_modules/' '.turbo/' '.nx/' '.moon/cache/' > "$work/.gitignore"
  git -C "$work" init --quiet
  git -C "$work" add .gitignore package.json pnpm-workspace.yaml pnpm-lock.yaml yarn.lock .yarnrc.yml turbo.json nx.json lerna.json lage.config.js .moon packages
  git -C "$work" -c user.name='TTC M6 Smoke' -c user.email='ttc-m6@example.invalid' commit --quiet -m 'Create isolated M6 smoke fixture'
  compare lage lage all test --verbose --no-cache
  compare moon-cold moon all run :test
  compare moon-warm-cache moon cached run :test

  node -e "const fs=require('node:fs');const p=JSON.parse(fs.readFileSync('$work/package.json'));p.packageManager='pnpm@9.15.9';fs.writeFileSync('$work/package.json',JSON.stringify(p)+'\\n')"
  rm -f "$scratch/direct-marker" "$scratch/ttc-marker"
  set +e
  (cd "$work" && M6_FAIL_TEST=api M6_MARKER="$scratch/direct-marker" "$tools/node_modules/.bin/turbo" run test) > "$scratch/failure.direct.out" 2> "$scratch/failure.direct.err"
  direct_status=$?
  (cd "$work" && M6_FAIL_TEST=api M6_MARKER="$scratch/ttc-marker" "$binary" "turbo run test") > "$scratch/failure.ttc.out" 2> "$scratch/failure.ttc.err"
  ttc_status=$?
  set -e
  test "$direct_status" -ne 0
  test "$ttc_status" -eq "$direct_status"
  grep -q 'M6 retained failure from api' "$scratch/failure.direct.out" "$scratch/failure.direct.err"
  grep -q 'M6 retained failure from api' "$scratch/failure.ttc.out" "$scratch/failure.ttc.err"
  for package in api web shared; do
    direct_count=$(grep -cx "$package" "$scratch/direct-marker" || true)
    ttc_count=$(grep -cx "$package" "$scratch/ttc-marker" || true)
    test "$direct_count" -eq "$ttc_count"
  done
  test "$(grep -cx api "$scratch/direct-marker")" -eq 1
  test "$(grep -cx api "$scratch/ttc-marker")" -eq 1
  failure_direct_count=$(wc -l < "$scratch/direct-marker" | tr -d ' ')
  failure_ttc_count=$(wc -l < "$scratch/ttc-marker" | tr -d ' ')
  printf 'turbo-failure exit=%s diagnostic=retained direct_marker_count=%s ttc_marker_count=%s\n' \
    "$ttc_status" "$failure_direct_count" "$failure_ttc_count" >> "$report"
  report_bytes turbo-failure 'turbo run test (M6_FAIL_TEST=api)' "$ttc_status" \
    "$scratch/failure.direct.out" "$scratch/failure.direct.err" \
    "$scratch/failure.ttc.out" "$scratch/failure.ttc.err"
  raw_replay_checksums turbo-failure "$scratch/failure.ttc.err"
fi

if [ "$group" = all ] || [ "$group" = systems ]; then
rustc_version=$(rustc +1.98.1 --version)
python_version=$(python3 --version | awk '{print $2}')
go_version=$(go version)
test "$python_version" = 3.14.7
case "$go_version" in *"go1.27.1"*) ;; *) printf 'expected Go 1.27.1; received %s\n' "$go_version" >&2; exit 1 ;; esac
python3 -m venv "$scratch/venv"
"$scratch/venv/bin/python" -m pip install --disable-pip-version-check --no-cache-dir -r scripts/m5-python-requirements.txt > "$scratch/install-python.log" 2>&1
pytest_version=$("$scratch/venv/bin/python" -m pytest --version)
case "$pytest_version" in *"9.1.1"*) ;; *) printf 'expected pytest 9.1.1; received %s\n' "$pytest_version" >&2; exit 1 ;; esac
printf 'mixed-smoke toolchains python=%s pytest=%s rust=%s go=%s\n' \
  "$python_version" "$(printf '%s' "$pytest_version" | awk '{print $2}')" "$rustc_version" "$go_version" >> "$report"
mkdir -p "$scratch/cargo/crates/api/src" "$scratch/cargo/crates/web/src"
cat > "$scratch/cargo/Cargo.toml" <<'TOML'
[workspace]
members = ["crates/api", "crates/web"]
resolver = "3"
TOML
for package in api web; do
  cat > "$scratch/cargo/crates/$package/Cargo.toml" <<TOML
[package]
name = "$package"
version = "0.1.0"
edition = "2024"
TOML
  cat > "$scratch/cargo/crates/$package/src/lib.rs" <<RUST
#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::Write;
    #[test]
    fn records_workspace_package_once() {
        let mut marker = OpenOptions::new().create(true).append(true).open(std::env::var("M6_MARKER").unwrap()).unwrap();
        writeln!(marker, "$package").unwrap();
    }
    #[test] fn passing_case_1() { assert_eq!(2, 2); }
    #[test] fn passing_case_2() { assert_eq!(3, 3); }
    #[test] fn passing_case_3() { assert_eq!(4, 4); }
    #[test] fn passing_case_4() { assert_eq!(5, 5); }
    #[test] fn passing_case_5() { assert_eq!(6, 6); }
    #[test] fn passing_case_6() { assert_eq!(7, 7); }
    #[test] fn passing_case_7() { assert_eq!(8, 8); }
}
RUST
done
rm -f "$scratch/direct-cargo-marker" "$scratch/ttc-cargo-marker"
set +e
(cd "$scratch/cargo" && RUSTUP_TOOLCHAIN=1.98.1 M6_MARKER="$scratch/direct-cargo-marker" cargo test --workspace) > "$scratch/cargo.direct.out" 2> "$scratch/cargo.direct.err"
direct_status=$?
(cd "$scratch/cargo" && RUSTUP_TOOLCHAIN=1.98.1 M6_MARKER="$scratch/ttc-cargo-marker" "$binary" "cargo test --workspace") > "$scratch/cargo.ttc.out" 2> "$scratch/cargo.ttc.err"
ttc_status=$?
set -e
test "$direct_status" -eq "$ttc_status"
assert_reduced "$scratch/cargo.direct.out" "$scratch/cargo.direct.err" "$scratch/cargo.ttc.out" "$scratch/cargo.ttc.err"
test "$(wc -l < "$scratch/direct-cargo-marker" | tr -d ' ')" -eq 2
test "$(wc -l < "$scratch/ttc-cargo-marker" | tr -d ' ')" -eq 2
printf 'cargo-workspace rustc=%s marker_count=2\n' \
  "$rustc_version" >> "$report"
report_bytes cargo-workspace 'cargo test --workspace' "$ttc_status" \
  "$scratch/cargo.direct.out" "$scratch/cargo.direct.err" \
  "$scratch/cargo.ttc.out" "$scratch/cargo.ttc.err"
raw_replay_checksums cargo-workspace "$scratch/cargo.ttc.err"

mkdir -p "$scratch/go/api" "$scratch/go/web"
cat > "$scratch/go/go.work" <<'GOWORK'
go 1.27.1
use (
  ./api
  ./web
)
GOWORK
for package in api web; do
  case "$package" in api) test_name=API ;; web) test_name=Web ;; esac
  cat > "$scratch/go/$package/go.mod" <<GOMOD
module example.com/$package

go 1.27.1
GOMOD
  cat > "$scratch/go/$package/workspace_test.go" <<GOTEST
package $package

import (
  "fmt"
  "os"
  "testing"
)

func Test${test_name}Workspace(t *testing.T) {
  marker, err := os.OpenFile(os.Getenv("M6_MARKER"), os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0600)
  if err != nil { t.Fatal(err) }
  defer marker.Close()
  if _, err := fmt.Fprintln(marker, "$package"); err != nil { t.Fatal(err) }
  for index := 0; index < 8; index++ {
    t.Run(fmt.Sprintf("case-%d", index), func(t *testing.T) { if index < 0 { t.Fatal(index) } })
  }
}
GOTEST
done
rm -f "$scratch/direct-go-marker" "$scratch/ttc-go-marker"
set +e
(cd "$scratch/go" && M6_MARKER="$scratch/direct-go-marker" go test -v ./api/... ./web/...) > "$scratch/go.direct.out" 2> "$scratch/go.direct.err"
direct_status=$?
(cd "$scratch/go" && M6_MARKER="$scratch/ttc-go-marker" "$binary" "go test -v ./api/... ./web/...") > "$scratch/go.ttc.out" 2> "$scratch/go.ttc.err"
ttc_status=$?
set -e
test "$direct_status" -eq "$ttc_status"
assert_reduced "$scratch/go.direct.out" "$scratch/go.direct.err" "$scratch/go.ttc.out" "$scratch/go.ttc.err"
test "$(wc -l < "$scratch/direct-go-marker" | tr -d ' ')" -eq 2
test "$(wc -l < "$scratch/ttc-go-marker" | tr -d ' ')" -eq 2
printf 'go-workspace version=%s marker_count=2\n' "$go_version" >> "$report"
report_bytes go-workspace 'go test -v ./api/... ./web/...' "$ttc_status" \
  "$scratch/go.direct.out" "$scratch/go.direct.err" "$scratch/go.ttc.out" "$scratch/go.ttc.err"
raw_replay_checksums go-workspace "$scratch/go.ttc.err"

mkdir -p "$scratch/mixed-js-go/go/api" "$scratch/mixed-js-go/go/web"
ln -s "$work/packages" "$scratch/mixed-js-go/packages"
ln -s "$tools/node_modules" "$scratch/mixed-js-go/node_modules"
cat > "$scratch/mixed-js-go/package.json" <<'JSON'
{"name":"m6-mixed-js-go","private":true,"scripts":{"check":"vitest run --reporter=verbose packages/api/tests packages/web/tests && go test -v ./go/api/... ./go/web/..."}}
JSON
cat > "$scratch/mixed-js-go/go.work" <<'GOWORK'
go 1.27.1
use (
  ./go/api
  ./go/web
)
GOWORK
for package in api web; do
  case "$package" in api) test_name=API ;; web) test_name=Web ;; esac
  cat > "$scratch/mixed-js-go/go/$package/go.mod" <<GOMOD
module example.com/m6/$package

go 1.27.1
GOMOD
  cat > "$scratch/mixed-js-go/go/$package/workspace_test.go" <<GOTEST
package $package

import (
  "fmt"
  "os"
  "testing"
)

func Test${test_name}Workspace(t *testing.T) {
  marker, err := os.OpenFile(os.Getenv("M6_MARKER"), os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0600)
  if err != nil { t.Fatal(err) }
  defer marker.Close()
  if _, err := fmt.Fprintln(marker, "go-$package"); err != nil { t.Fatal(err) }
  for index := 0; index < 8; index++ {
    t.Run(fmt.Sprintf("case-%d", index), func(t *testing.T) { if index < 0 { t.Fatal(index) } })
  }
}
GOTEST
done
set +e
(cd "$scratch/mixed-js-go" && M6_MARKER="$scratch/mixed-js-go.direct.marker" npm run check) > "$scratch/mixed-js-go.direct.out" 2> "$scratch/mixed-js-go.direct.err"
direct_status=$?
(cd "$scratch/mixed-js-go" && M6_MARKER="$scratch/mixed-js-go.ttc.marker" "$binary" "npm run check") > "$scratch/mixed-js-go.ttc.out" 2> "$scratch/mixed-js-go.ttc.err"
ttc_status=$?
set -e
test "$direct_status" -eq "$ttc_status"
test "$(wc -l < "$scratch/mixed-js-go.direct.marker" | tr -d ' ')" -eq 4
test "$(wc -l < "$scratch/mixed-js-go.ttc.marker" | tr -d ' ')" -eq 4
for marker in api web go-api go-web; do
  test "$(grep -cx "$marker" "$scratch/mixed-js-go.direct.marker")" -eq 1
  test "$(grep -cx "$marker" "$scratch/mixed-js-go.ttc.marker")" -eq 1
done
assert_reduced "$scratch/mixed-js-go.direct.out" "$scratch/mixed-js-go.direct.err" "$scratch/mixed-js-go.ttc.out" "$scratch/mixed-js-go.ttc.err"
printf 'mixed-js-go marker_count=4\n' >> "$report"
report_bytes mixed-js-go 'npm run check' "$ttc_status" \
  "$scratch/mixed-js-go.direct.out" "$scratch/mixed-js-go.direct.err" \
  "$scratch/mixed-js-go.ttc.out" "$scratch/mixed-js-go.ttc.err"
raw_replay_checksums mixed-js-go "$scratch/mixed-js-go.ttc.err"

mkdir -p "$scratch/nested/py/tests" "$scratch/nested/rust/src"
python3 - "$scratch/nested/package.json" "$scratch/venv/bin/python" <<'PY'
import json
import sys

path, python = sys.argv[1:]
with open(path, "w", encoding="utf-8") as package_file:
    json.dump({"name": "m6-nested-python-rust", "private": True, "scripts": {
        "check": f"{python} -m pytest -v -p no:cacheprovider py/tests && cargo test --manifest-path rust/Cargo.toml -- --nocapture"
    }}, package_file)
PY
cat > "$scratch/nested/py/tests/test_generated.py" <<'PYTEST'
import os

import pytest


def test_marker_record():
    with open(os.environ["M6_MARKER"], "a", encoding="utf-8") as marker:
        marker.write("python\n")


@pytest.mark.parametrize("index", range(8))
def test_generated_record(index):
    assert index >= 0
PYTEST
cat > "$scratch/nested/rust/Cargo.toml" <<'TOML'
[package]
name = "m6-nested-rust"
version = "0.1.0"
edition = "2024"
TOML
cat > "$scratch/nested/rust/src/lib.rs" <<'RUST'
#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::Write;
    #[test]
    fn records_nested_rust_once() {
        let mut marker = OpenOptions::new().create(true).append(true).open(std::env::var("M6_MARKER").unwrap()).unwrap();
        writeln!(marker, "rust").unwrap();
    }
    #[test] fn passing_case_1() { assert_eq!(2, 2); }
    #[test] fn passing_case_2() { assert_eq!(3, 3); }
    #[test] fn passing_case_3() { assert_eq!(4, 4); }
    #[test] fn passing_case_4() { assert_eq!(5, 5); }
    #[test] fn passing_case_5() { assert_eq!(6, 6); }
    #[test] fn passing_case_6() { assert_eq!(7, 7); }
    #[test] fn passing_case_7() { assert_eq!(8, 8); }
}
RUST
set +e
(cd "$scratch/nested" && RUSTUP_TOOLCHAIN=1.98.1 M6_MARKER="$scratch/nested.direct.marker" npm run check) > "$scratch/nested.direct.out" 2> "$scratch/nested.direct.err"
direct_status=$?
(cd "$scratch/nested" && RUSTUP_TOOLCHAIN=1.98.1 M6_MARKER="$scratch/nested.ttc.marker" "$binary" "npm run check") > "$scratch/nested.ttc.out" 2> "$scratch/nested.ttc.err"
ttc_status=$?
set -e
test "$direct_status" -eq "$ttc_status"
test "$(wc -l < "$scratch/nested.direct.marker" | tr -d ' ')" -eq 2
test "$(wc -l < "$scratch/nested.ttc.marker" | tr -d ' ')" -eq 2
grep -qx python "$scratch/nested.direct.marker"
grep -qx rust "$scratch/nested.direct.marker"
grep -qx python "$scratch/nested.ttc.marker"
grep -qx rust "$scratch/nested.ttc.marker"
assert_reduced "$scratch/nested.direct.out" "$scratch/nested.direct.err" "$scratch/nested.ttc.out" "$scratch/nested.ttc.err"
printf 'nested-python-rust marker_count=2\n' >> "$report"
report_bytes nested-python-rust 'npm run check' "$ttc_status" \
  "$scratch/nested.direct.out" "$scratch/nested.direct.err" "$scratch/nested.ttc.out" "$scratch/nested.ttc.err"
raw_replay_checksums nested-python-rust "$scratch/nested.ttc.err"
fi

if [ "$group" = all ]; then
  grep -Eq '^raw-replay label=.*stdout_sha256=[0-9a-f]{64} stderr_sha256=[0-9a-f]{64}$' "$report"
fi
