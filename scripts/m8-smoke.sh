#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
binary=${1:-target/release/ttc}
evidence=${2:-target/m8-evidence}
case "$binary" in /*) ;; *) binary="$repository_root/$binary" ;; esac
case "$evidence" in /*) ;; *) evidence="$repository_root/$evidence" ;; esac
test -x "$binary"
mkdir -p "$evidence"
mkdir -p "$evidence/logs"
original_home=${HOME:-}
docker_cli_plugins=${DOCKER_CONFIG:-"$original_home/.docker"}/cli-plugins
scratch=$(mktemp -d "${TMPDIR:-/tmp}/ttc-m8.XXXXXX")
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$scratch/home" "$scratch/config" "$scratch/state" "$scratch/data" "$scratch/cache"
export HOME="$scratch/home" XDG_CONFIG_HOME="$scratch/config" XDG_STATE_HOME="$scratch/state"
export XDG_DATA_HOME="$scratch/data" XDG_CACHE_HOME="$scratch/cache" TTC_INTERNAL_TEST_TMP_ROOT="$scratch"
export BUNDLE_PATH=${BUNDLE_PATH:-"$scratch/bundle"} BUNDLE_APP_CONFIG="$scratch/bundle-config"
export DOCKER_CONFIG="$scratch/docker-config" BUILDKIT_PROGRESS=plain
mkdir -p "$DOCKER_CONFIG/cli-plugins"
if [ -d "$docker_cli_plugins" ]; then
  cp -R "$docker_cli_plugins/." "$DOCKER_CONFIG/cli-plugins/"
fi

version() {
  label=$1
  actual=$2
  expected=$3
  test "$actual" = "$expected" || {
    printf '%s version mismatch: got %s expected %s\n' "$label" "$actual" "$expected" >&2
    exit 1
  }
}

version cmake "$(cmake --version | sed -n '1s/^cmake version //p')" 3.31.6
version ctest "$(ctest --version | sed -n '1s/^ctest version //p')" 3.31.6
version ninja "$(ninja --version)" 1.12.1
version make "$(make --version | sed -n '1s/^GNU Make //p')" 4.4.1
version gcc "$(gcc -dumpfullversion -dumpversion)" 13.3.0
version gxx "$(g++ -dumpfullversion -dumpversion)" 13.3.0
version ruby "$(ruby -e 'print RUBY_VERSION')" 3.3.8
version bundler "$(bundle --version | awk '{print $3}')" 2.6.9
export BUNDLE_GEMFILE="$repository_root/scripts/m8-smoke/ruby/Gemfile"
version rspec "$(bundle exec ruby -e 'require "rubygems"; print Gem::Specification.find_by_name("rspec").version')" 3.13.0
version rspec-core "$(bundle exec ruby -e 'require "rspec/core"; print RSpec::Core::Version::STRING')" 3.13.6
version rubocop "$(bundle exec rubocop --version)" 1.75.5
version rake "$(bundle exec rake --version | awk '/^rake, version / {print $3}')" 13.2.1
swift --version | grep -F 'Swift version 6.1.2' >/dev/null
version docker "$(docker --version | awk '{gsub(/,/, "", $3); print $3}')" 28.1.1
version buildx "$(docker buildx version | awk '{print $2}')" v0.23.0
version compose "$(docker compose version --short)" 2.35.1
version terraform "$(terraform version | sed -n '1s/^Terraform v//p')" 1.11.4
version helm "$(helm version --short | sed 's/+.*//')" v3.17.3

cp "$repository_root/scripts/m8-tool-versions.txt" "$evidence/tool-versions.txt"
cp "$repository_root/scripts/m8-tool-checksums.txt" "$evidence/tool-checksums.txt"
cp "$repository_root/scripts/m8-tool-images.txt" "$evidence/tool-images.txt"
printf 'swift=6.1.2\n' >> "$evidence/tool-versions.txt"
printf 'binary-sha256=' >> "$evidence/tool-versions.txt"
sha256sum "$binary" | awk '{print $1}' >> "$evidence/tool-versions.txt"

compare_in() {
  label=$1
  direct_directory=$2
  ttc_directory=$3
  expected=$4
  shift 4
  mkdir -p "$scratch/$label-direct-cache" "$scratch/$label-ttc-cache"
  set +e
  m8_capture_in "$direct_directory" "$scratch/$label-direct" "$scratch/$label-direct-cache" "$@"
  direct_status=$?
  m8_capture_in "$ttc_directory" "$scratch/$label-ttc" "$scratch/$label-ttc-cache" "$binary" "$@"
  ttc_status=$?
  set -e
  for stream in out err; do
    cp "$scratch/$label-direct.$stream" "$evidence/logs/$label-direct.$stream"
    cp "$scratch/$label-ttc.$stream" "$evidence/logs/$label-ttc.$stream"
  done
  if [ "$direct_status" -ne "$expected" ] || [ "$ttc_status" -ne "$direct_status" ]; then
    for side in direct ttc; do
      for stream in out err; do
        printf '%s %s %s (last 4000 bytes):\n' "$label" "$side" "$stream" >&2
        tail -c 4000 "$scratch/$label-$side.$stream" >&2
      done
    done
  fi
  test "$direct_status" -eq "$expected" || {
    printf '%s baseline status %s expected %s\n' "$label" "$direct_status" "$expected" >&2
    return 1
  }
  test "$ttc_status" -eq "$direct_status" || {
    printf '%s TTC status %s differed from baseline %s\n' "$label" "$ttc_status" "$direct_status" >&2
    return 1
  }
  printf '%s exit=%s bytes=%s/%s\n' "$label" "$ttc_status" \
    "$(($(wc -c < "$scratch/$label-direct.out") + $(wc -c < "$scratch/$label-direct.err")))" \
    "$(($(wc -c < "$scratch/$label-ttc.out") + $(wc -c < "$scratch/$label-ttc.err")))" >> "$evidence/smoke-report.txt"
}

compare() {
  label=$1
  directory=$2
  expected=$3
  shift 3
  compare_in "$label" "$directory" "$directory" "$expected" "$@"
}

require_fragment() {
  label=$1
  side=$2
  fragment=$3
  if ! cat "$scratch/$label-$side.out" "$scratch/$label-$side.err" | grep -F "$fragment" >/dev/null; then
    printf '%s %s output did not retain: %s\n' "$label" "$side" "$fragment" >&2
    return 1
  fi
}

. "$repository_root/scripts/m8-smoke-assertions.sh"

printf 'commit=%s\n' "$(git -C "$repository_root" rev-parse HEAD)" > "$evidence/smoke-report.txt"

build_source="$repository_root/scripts/m8-smoke/build"
prepare_build_pair() {
  label=$1
  shift
  for side in direct ttc; do
    project="$scratch/$label-$side"
    mkdir -p "$project"
    cp -R "$build_source/." "$project/"
    cmake -S "$project" -B "$project/build" -G Ninja "$@" > "$scratch/$label-$side-configure.log"
  done
}
prepare_build_pair cmake
prepare_build_pair ninja
compare_in cmake-build "$scratch/cmake-direct" "$scratch/cmake-ttc" 0 cmake --build build
assert_build_records_compacted cmake-build
compare_in ninja-build "$scratch/ninja-direct/build" "$scratch/ninja-ttc/build" 0 ninja
assert_build_records_compacted ninja-build
prepare_build_pair cmake-warning -DTTC_M8_EMIT_WARNING=ON
compare_in cmake-warning "$scratch/cmake-warning-direct" "$scratch/cmake-warning-ttc" 0 cmake --build build
require_fragment cmake-warning direct 'TTC M8 build warning retention'
require_fragment cmake-warning ttc 'TTC M8 build warning retention'
# Diagnostic output makes the remaining physical stream raw. This four-step
# fixture cannot establish confidence before the warning and must stay raw.
if grep -F 'TTC:' "$scratch/cmake-warning-ttc.err" >/dev/null; then
  printf 'cmake-warning unexpectedly compacted diagnostic output\n' >&2
  exit 1
fi
compare_in ctest-pass "$scratch/cmake-direct" "$scratch/cmake-ttc" 0 ctest --test-dir build --output-on-failure
grep -F '100% tests passed' "$scratch/ctest-pass-direct.out" >/dev/null
grep -F '100% tests passed' "$scratch/ctest-pass-ttc.out" >/dev/null
assert_ctest_records_compacted ctest-pass
CTEST_DIR=build; export CTEST_DIR
compare_in make-test "$scratch/cmake-direct" "$scratch/cmake-ttc" 0 make test
assert_ctest_records_compacted make-test
compare_in make-check "$scratch/cmake-direct" "$scratch/cmake-ttc" 0 make check
assert_ctest_records_compacted make-check

ruby_project="$repository_root/scripts/m8-smoke/ruby"
bundle check >/dev/null
compare rspec-pass "$ruby_project" 0 bundle exec rspec spec/smoke_spec.rb
compare rspec-fail "$ruby_project" 1 bundle exec rspec spec/failure_spec.rb
require_fragment rspec-fail direct 'expected: 5'
require_fragment rspec-fail ttc 'expected: 5'
large_ruby_project="$scratch/ruby-large"
mkdir -p "$large_ruby_project/spec" "$scratch/rubocop-large"
printf "RSpec.describe 'TTC M8 smoke' do\n" > "$large_ruby_project/spec/large_spec.rb"
index=0
while [ "$index" -lt 1001 ]; do
  printf "  it('passes') { expect(2 + 2).to eq(4) }\n" >> "$large_ruby_project/spec/large_spec.rb"
  printf "# frozen_string_literal: true\n\nputs 'TTC M8 smoke'\n" > "$scratch/rubocop-large/fixture_$index.rb"
  index=$((index + 1))
done
printf "end\n" >> "$large_ruby_project/spec/large_spec.rb"
compare rspec-large "$large_ruby_project/spec" 0 bundle exec rspec large_spec.rb
assert_dot_meter_compacted rspec-large
compare rubocop "$ruby_project" 0 bundle exec rubocop spec/smoke_spec.rb
compare rubocop-large "$scratch/rubocop-large" 0 bundle exec rubocop .
assert_dot_meter_compacted rubocop-large
SPEC=spec/smoke_spec.rb; export SPEC
compare rake-test "$ruby_project" 0 bundle exec rake test

for side in direct ttc; do
  swift_project="$scratch/swift-$side"
  mkdir -p "$swift_project"
  cp -R "$repository_root/scripts/m8-smoke/swift/." "$swift_project/"
done
compare_in swift-build "$scratch/swift-direct" "$scratch/swift-ttc" 0 swift build
compare_in swift-test-pass "$scratch/swift-direct" "$scratch/swift-ttc" 0 swift test --filter SmokeTests/testPass
compare_in swift-test-fail "$scratch/swift-direct" "$scratch/swift-ttc" 1 swift test --filter SmokeTests/testFailure
require_fragment swift-test-fail direct 'XCTAssertEqual failed'
require_fragment swift-test-fail ttc 'XCTAssertEqual failed'

docker_project="$repository_root/scripts/m8-smoke/container"
compare docker-build "$docker_project" 0 docker build --no-cache --progress=plain -t ttc-m8-smoke .
require_fragment docker-build direct 'load build definition from Dockerfile'
require_fragment docker-build ttc 'load build definition from Dockerfile'
compare compose-build "$docker_project" 0 docker compose -f compose.yaml build --no-cache --progress plain

terraform_project="$repository_root/scripts/m8-smoke/terraform"
compare terraform-validate "$terraform_project" 0 terraform validate -no-color
cmp "$scratch/terraform-validate-direct.out" "$scratch/terraform-validate-ttc.out"
cmp "$scratch/terraform-validate-direct.err" "$scratch/terraform-validate-ttc.err"
mkdir -p "$scratch/terraform-invalid"
printf 'resource "invalid" {\n' > "$scratch/terraform-invalid/main.tf"
compare terraform-invalid "$scratch/terraform-invalid" 1 terraform validate -no-color
grep -F 'Error' "$scratch/terraform-invalid-ttc.err" >/dev/null
cmp "$scratch/terraform-invalid-direct.out" "$scratch/terraform-invalid-ttc.out"
cmp "$scratch/terraform-invalid-direct.err" "$scratch/terraform-invalid-ttc.err"

helm_project="$repository_root/scripts/m8-smoke/helm"
compare helm-lint "$helm_project" 0 helm lint .

podman_image=$(sed -n 's/^podman=//p' "$repository_root/scripts/m8-tool-images.txt")
docker pull "$podman_image" > "$scratch/podman-pull.log"
mkdir -p "$scratch/podman"
cp "$repository_root/scripts/m8-smoke/container/Dockerfile" "$scratch/podman/Dockerfile"
cp "$repository_root/scripts/m8-smoke/container/payload" "$scratch/podman/payload"
mkdir -p "$scratch/podman-security"
printf '#include <stdio.h>\nint main(void) { fputs("warning: security.capability retention fixture\\n", stderr); return 17; }\n' \
  > "$scratch/podman-security/payload.c"
gcc -static -o "$scratch/podman-security/payload" "$scratch/podman-security/payload.c"
printf 'FROM scratch\nCOPY payload /payload\nRUN ["/payload"]\n' > "$scratch/podman-security/Dockerfile"
if docker run --rm --privileged \
  --tmpfs /var/lib/containers:size=512m \
  --tmpfs /home/podman/.local/share/containers:size=512m \
  --mount "type=bind,src=$binary,dst=/ttc,readonly" \
  --mount "type=bind,src=$scratch/podman,dst=/work" \
  --mount "type=bind,src=$scratch/podman-security,dst=/security" \
  --workdir /work "$podman_image" sh -exc '
    podman --version | grep -Fx "podman version 5.4.2" >/dev/null
    podman build --no-cache -t ttc-m8-podman . > /tmp/podman-direct.log 2>&1
    /ttc podman build --no-cache -t ttc-m8-podman . > /tmp/podman-ttc.log 2>&1
    grep -F "STEP 1/2" /tmp/podman-ttc.log >/dev/null
    grep -F "Successfully tagged localhost/ttc-m8-podman:latest" /tmp/podman-ttc.log >/dev/null
    set +e
    podman build --isolation=chroot --no-cache -t ttc-m8-podman-security /security > /tmp/podman-security-direct.log 2>&1
    direct_status=$?
    /ttc podman build --isolation=chroot --no-cache -t ttc-m8-podman-security /security > /tmp/podman-security-ttc.log 2>&1
    ttc_status=$?
    set -e
    test "$direct_status" -ne 0
    test "$ttc_status" -eq "$direct_status"
    grep -F "warning: security.capability retention fixture" /tmp/podman-security-direct.log >/dev/null
    grep -F "warning: security.capability retention fixture" /tmp/podman-security-ttc.log >/dev/null
    cp /tmp/podman-direct.log /tmp/podman-ttc.log /tmp/podman-security-direct.log /tmp/podman-security-ttc.log /security/
    printf "%s\n" "$direct_status" > /security/status
  ' > "$scratch/podman.log" 2>&1; then
  :
else
  tail -c 8000 "$scratch/podman.log" >&2
  exit 1
fi
cp "$scratch/podman.log" "$evidence/logs/podman.log"
cp "$scratch/podman-security/"*.log "$evidence/logs/"
printf 'podman exit=0 version=5.4.2 summary-retention=verified\n' >> "$evidence/smoke-report.txt"
printf 'podman-security exit=%s diagnostic-retention=verified\n' "$(cat "$scratch/podman-security/status")" >> "$evidence/smoke-report.txt"

(cd "$evidence" && find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS)
