#!/bin/sh
set -eu

binary=${1:-target/release/ttc}
report=${2:-target/m5-evidence/report.txt}
case "$binary" in /*) ;; *) binary="$(pwd)/$binary" ;; esac
case "$report" in /*) ;; *) report="$(pwd)/$report" ;; esac
mkdir -p "$(dirname "$report")"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$scratch/home" "$scratch/state" "$scratch/tmp" "$scratch/config" "$scratch/data" "$scratch/cache"
rustup_home=${RUSTUP_HOME:-"$HOME/.rustup"}
export HOME="$scratch/home" XDG_STATE_HOME="$scratch/state" XDG_CONFIG_HOME="$scratch/config"
export XDG_DATA_HOME="$scratch/data" TMPDIR="$scratch/tmp" TTC_INTERNAL_TEST_TMP_ROOT="$scratch/tmp"
export RUSTUP_HOME="$rustup_home" RUSTUP_TOOLCHAIN=1.98.1
export CARGO_HOME="$scratch/home/.cargo" CARGO_TARGET_DIR="$scratch/cargo-target"
export PIP_CACHE_DIR="$scratch/cache/pip" GOCACHE="$scratch/cache/go-build" GOMODCACHE="$scratch/cache/go-mod"

test -x "$binary"
rust_version=$(rustc --version | awk '{print $2}')
python_version=$(python3 --version | awk '{print $2}')
go_version=$(go version | awk '{print $3}')
test "$rust_version" = 1.98.1
test "$python_version" = 3.14.7
test "$go_version" = go1.27.1
printf 'toolchains rust=%s python=%s go=%s\n' "$rust_version" "$python_version" "$go_version"

if [ -n "${M5_NEXTEST_BIN:-}" ]; then
  nextest_bin=$M5_NEXTEST_BIN
else
  cargo install --locked cargo-nextest --version 0.9.108 > "$scratch/cargo-nextest-install.log" 2>&1 || {
    cat "$scratch/cargo-nextest-install.log" >&2
    exit 1
  }
  nextest_bin="$CARGO_HOME/bin/cargo-nextest"
fi
test -x "$nextest_bin"
export PATH="$(dirname "$nextest_bin"):$PATH"
nextest_output=$("$nextest_bin" --version)
case "$nextest_output" in
  *0.9.108*) ;;
  *) printf 'unexpected cargo-nextest version: %s\n' "$nextest_output" >&2; exit 1 ;;
esac
nextest_version=0.9.108
printf 'installed cargo-nextest %s\n' "$nextest_version"

python3 -m venv "$scratch/venv"
# shellcheck disable=SC1091
. "$scratch/venv/bin/activate"
python -m pip install --disable-pip-version-check -r scripts/m5-python-requirements.txt
test "$(python --version)" = "Python 3.14.7"
pytest_version=$(pytest --version)
case "$pytest_version" in
  *9.1.1*) ;;
  *) printf 'unexpected pytest version: %s\n' "$pytest_version" >&2; exit 1 ;;
esac
printf 'installed %s\n' "$pytest_version"

report_versions() {
  {
    printf 'M5 pinned real-tool smoke\n'
    printf 'rust=%s nextest=%s python=%s pytest=%s go=%s\n' \
      "$(rustc --version | awk '{print $2}')" \
      "$nextest_version" \
      "$(python --version | awk '{print $2}')" \
      "$(pytest --version | awk '{print $2}')" \
      "$(go version | awk '{print $3}')"
  } > "$report"
}

compare() {
  label=$1
  shift
  set +e
  "$@" > "$scratch/$label-direct.out" 2> "$scratch/$label-direct.err"
  direct_status=$?
  "$binary" "$@" > "$scratch/$label-ttc.out" 2> "$scratch/$label-ttc.err"
  ttc_status=$?
  set -e
  test "$direct_status" -eq "$ttc_status" || {
    printf '%s exit mismatch: direct=%s ttc=%s\n' "$label" "$direct_status" "$ttc_status" >&2
    return 1
  }
  for stream in out err; do
    grep -Eai 'warning|error|fail|panic|assert|expected|actual|security|race' \
      "$scratch/$label-direct.$stream" \
      | sed -E -e 's/(thread .*[[:space:]]\()[0-9]+(\) panicked)/\1PID\2/' \
          -e 's/[0-9]+(\.[0-9]+)?s/ELAPSED/g' \
      > "$scratch/$label-direct-$stream-diagnostics" || true
    grep -Eai 'warning|error|fail|panic|assert|expected|actual|security|race' \
      "$scratch/$label-ttc.$stream" \
      | sed -E -e 's/(thread .*[[:space:]]\()[0-9]+(\) panicked)/\1PID\2/' \
          -e 's/[0-9]+(\.[0-9]+)?s/ELAPSED/g' \
      > "$scratch/$label-ttc-$stream-diagnostics" || true
    cmp "$scratch/$label-direct-$stream-diagnostics" "$scratch/$label-ttc-$stream-diagnostics" || {
      diff -u "$scratch/$label-direct-$stream-diagnostics" "$scratch/$label-ttc-$stream-diagnostics" >&2 || true
      return 1
    }
  done
  case "$label" in
    pytest-failure | go-failure)
      for stream in out err; do
        sed -E 's/[0-9]+(\.[0-9]+)?s/ELAPSED/g' "$scratch/$label-direct.$stream" \
          > "$scratch/$label-direct-$stream-normalized"
        sed -E 's/[0-9]+(\.[0-9]+)?s/ELAPSED/g' "$scratch/$label-ttc.$stream" \
          > "$scratch/$label-ttc-$stream-normalized"
        cmp "$scratch/$label-direct-$stream-normalized" "$scratch/$label-ttc-$stream-normalized"
      done
      ;;
  esac
  direct_bytes=$(($(wc -c < "$scratch/$label-direct.out") + $(wc -c < "$scratch/$label-direct.err")))
  ttc_bytes=$(($(wc -c < "$scratch/$label-ttc.out") + $(wc -c < "$scratch/$label-ttc.err")))
  direct_stdout_bytes=$(wc -c < "$scratch/$label-direct.out")
  direct_stderr_bytes=$(wc -c < "$scratch/$label-direct.err")
  ttc_stdout_bytes=$(wc -c < "$scratch/$label-ttc.out")
  ttc_stderr_bytes=$(wc -c < "$scratch/$label-ttc.err")
  direct_diagnostic_bytes=$(($(wc -c < "$scratch/$label-direct-out-diagnostics") + $(wc -c < "$scratch/$label-direct-err-diagnostics")))
  ttc_diagnostic_bytes=$(($(wc -c < "$scratch/$label-ttc-out-diagnostics") + $(wc -c < "$scratch/$label-ttc-err-diagnostics")))
  printf '%s exit=%s baseline_bytes=%s ttc_bytes=%s baseline_stdout=%s baseline_stderr=%s ttc_stdout=%s ttc_stderr=%s retained_diagnostic_bytes=%s/%s\n' \
    "$label" "$direct_status" "$direct_bytes" "$ttc_bytes" \
    "$direct_stdout_bytes" "$direct_stderr_bytes" "$ttc_stdout_bytes" "$ttc_stderr_bytes" \
    "$direct_diagnostic_bytes" "$ttc_diagnostic_bytes" >> "$report"
}

mkdir -p "$scratch/rust/src" "$scratch/rust/tests" "$scratch/python/tests" "$scratch/go"
cat > "$scratch/rust/Cargo.toml" <<'EOF'
[package]
name = "m5-smoke"
version = "0.1.0"
edition = "2024"
EOF
cat > "$scratch/rust/src/lib.rs" <<'EOF'
pub fn add(left: u32, right: u32) -> u32 { left + right }
EOF
python - "$scratch/rust/tests/generated.rs" <<'PY'
import sys

with open(sys.argv[1], "w", encoding="utf-8") as test_file:
    for index in range(1001):
        test_file.write(
            f"#[test]\nfn generated_case_{index:04}() "
            "{ assert_eq!(m5_smoke::add(1, 1), 2); }\n"
        )
PY
cat > "$scratch/rust/tests/m5_failure.rs" <<'EOF'
#[test]
fn expected_failure() { assert_eq!(1, 2, "expected 1, actual 2"); }
EOF

cat > "$scratch/python/tests/test_generated.py" <<'PY'
def test_add():
    assert 1 + 1 == 2

for index in range(1000):
    globals()[f"test_generated_case_{index:04}"] = lambda: None
PY
cat > "$scratch/python/tests/test_failure.py" <<'PY'
def test_expected_failure():
    assert 1 == 2, "expected 1, actual 2"
PY

cat > "$scratch/go/go.mod" <<'EOF'
module example.com/m5-smoke

go 1.27.1
EOF
python - "$scratch/go/generated_test.go" <<'PY'
import sys

with open(sys.argv[1], "w", encoding="utf-8") as test_file:
    test_file.write("package smoke\nimport \"testing\"\n")
    for index in range(1001):
        test_file.write(
            f"func TestGenerated{index:04}(t *testing.T) "
            "{ if 1+1 != 2 { t.Fatal(\"expected 2 actual 3\") } }\n"
        )
PY

report_versions
cd "$scratch/rust"
cargo fmt
compare cargo-test cargo test --test generated
compare cargo-nextest cargo nextest run --test generated
compare cargo-build cargo build
compare cargo-check cargo check
compare cargo-clippy cargo clippy
compare cargo-fmt cargo fmt --check
compare cargo-doc cargo doc --no-deps
compare cargo-failure cargo test --test m5_failure -- --nocapture
cd "$scratch/python"
compare pytest-success pytest -v --color=no -p no:cacheprovider tests/test_generated.py
compare pytest-failure pytest -v --color=no -p no:cacheprovider tests/test_failure.py
cd "$scratch/go"
compare go-test go test -v -count=1 ./...
compare go-test-json go test -json -v -count=1 ./...
compare go-build go build ./...
compare go-vet go vet ./...
cat > "$scratch/go/failure_test.go" <<'EOF'
package smoke
import "testing"
func TestExpectedFailure(t *testing.T) { t.Fatalf("expected 1, actual 2") }
EOF
compare go-failure go test -v -count=1 -run '^TestExpectedFailure$' ./...

printf 'M5 smoke passed\n' >> "$report"
