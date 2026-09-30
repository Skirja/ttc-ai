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
scratch=$(mktemp -d "${TMPDIR:-/tmp}/ttc-m8.XXXXXX")
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$scratch/home" "$scratch/config" "$scratch/state" "$scratch/data" "$scratch/cache"
export HOME="$scratch/home" XDG_CONFIG_HOME="$scratch/config" XDG_STATE_HOME="$scratch/state"
export XDG_DATA_HOME="$scratch/data" XDG_CACHE_HOME="$scratch/cache" TTC_INTERNAL_TEST_TMP_ROOT="$scratch"
export BUNDLE_PATH=${BUNDLE_PATH:-"$scratch/bundle"} BUNDLE_APP_CONFIG="$scratch/bundle-config"
export DOCKER_CONFIG="$scratch/docker-config" BUILDKIT_PROGRESS=plain
mkdir -p "$DOCKER_CONFIG"

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
version rspec "$(bundle exec ruby -e 'require "rspec/core"; print RSpec::Core::Version::STRING')" 3.13.0
version rubocop "$(bundle exec rubocop --version)" 1.75.5
version rake "$(bundle exec rake --version | awk '{print $2}')" 13.2.1
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

compare() {
  label=$1
  directory=$2
  expected=$3
  shift 3
  set +e
  (cd "$directory" && "$@") > "$scratch/$label-direct.out" 2> "$scratch/$label-direct.err"
  direct_status=$?
  (cd "$directory" && "$binary" "$@") > "$scratch/$label-ttc.out" 2> "$scratch/$label-ttc.err"
  ttc_status=$?
  set -e
  test "$direct_status" -eq "$expected" || {
    printf '%s baseline status %s expected %s\n' "$label" "$direct_status" "$expected" >&2
    return 1
  }
  test "$ttc_status" -eq "$direct_status" || {
    printf '%s TTC status %s differed from baseline %s\n' "$label" "$ttc_status" "$direct_status" >&2
    return 1
  }
  for stream in out err; do
    cp "$scratch/$label-direct.$stream" "$evidence/logs/$label-direct.$stream"
    cp "$scratch/$label-ttc.$stream" "$evidence/logs/$label-ttc.$stream"
  done
  printf '%s exit=%s bytes=%s/%s\n' "$label" "$ttc_status" \
    "$(($(wc -c < "$scratch/$label-direct.out") + $(wc -c < "$scratch/$label-direct.err")))" \
    "$(($(wc -c < "$scratch/$label-ttc.out") + $(wc -c < "$scratch/$label-ttc.err")))" >> "$evidence/smoke-report.txt"
}

printf 'commit=%s\n' "$(git -C "$repository_root" rev-parse HEAD)" > "$evidence/smoke-report.txt"

build_source="$repository_root/scripts/m8-smoke/build"
mkdir -p "$scratch/cmake-source" "$scratch/cmake-build"
cp -R "$build_source/." "$scratch/cmake-source/"
cmake -S "$scratch/cmake-source" -B "$scratch/cmake-build" -G Ninja > "$scratch/configure.log"
compare cmake-build "$scratch/cmake-source" 0 cmake --build "$scratch/cmake-build"
compare ninja-build "$scratch/cmake-build" 0 ninja
compare ctest-pass "$scratch/cmake-source" 0 ctest --test-dir "$scratch/cmake-build" --output-on-failure
CTEST_DIR="$scratch/cmake-build"; export CTEST_DIR
compare make-test "$scratch/cmake-source" 0 make test
compare make-check "$scratch/cmake-source" 0 make check

ruby_project="$repository_root/scripts/m8-smoke/ruby"
bundle check >/dev/null
compare rspec-pass "$ruby_project" 0 bundle exec rspec spec/smoke_spec.rb
compare rspec-fail "$ruby_project" 1 bundle exec rspec spec/failure_spec.rb
grep -F 'expected: 5' "$scratch/rspec-fail-ttc.out" >/dev/null
compare rubocop "$ruby_project" 0 bundle exec rubocop --format progress spec/smoke_spec.rb
compare rake-test "$ruby_project" 0 env SPEC=spec/smoke_spec.rb bundle exec rake test

swift_project="$scratch/swift"
mkdir -p "$swift_project"
cp -R "$repository_root/scripts/m8-smoke/swift/." "$swift_project/"
compare swift-build "$swift_project" 0 swift build --package-path "$swift_project"
compare swift-test-pass "$swift_project" 0 swift test --package-path "$swift_project" --filter SmokeTests/testPass
compare swift-test-fail "$swift_project" 1 swift test --package-path "$swift_project" --filter SmokeTests/testFailure
grep -F 'XCTAssertEqual failed' "$scratch/swift-test-fail-ttc.out" >/dev/null

docker_project="$repository_root/scripts/m8-smoke/container"
compare docker-build "$docker_project" 0 docker build --progress=plain -t ttc-m8-smoke .
compare compose-build "$docker_project" 0 docker compose -f compose.yaml build --progress plain

terraform_project="$repository_root/scripts/m8-smoke/terraform"
compare terraform-validate "$terraform_project" 0 terraform validate -no-color
mkdir -p "$scratch/terraform-invalid"
printf 'resource "invalid" {\n' > "$scratch/terraform-invalid/main.tf"
compare terraform-invalid "$scratch/terraform-invalid" 1 terraform validate -no-color
grep -F 'Error' "$scratch/terraform-invalid-ttc.err" >/dev/null

helm_project="$repository_root/scripts/m8-smoke/helm"
compare helm-lint "$helm_project" 0 helm lint .

podman_image=$(sed -n 's/^podman=//p' "$repository_root/scripts/m8-tool-images.txt")
docker pull "$podman_image" > "$scratch/podman-pull.log"
mkdir -p "$scratch/podman"
cp "$repository_root/scripts/m8-smoke/container/Dockerfile" "$scratch/podman/Dockerfile"
cp "$repository_root/scripts/m8-smoke/container/payload" "$scratch/podman/payload"
docker run --rm --privileged \
  --tmpfs /var/lib/containers:size=512m \
  --tmpfs /home/podman/.local/share/containers:size=512m \
  --mount "type=bind,src=$binary,dst=/ttc,readonly" \
  --mount "type=bind,src=$scratch/podman,dst=/work" \
  --workdir /work "$podman_image" sh -ec '
    podman --version | grep -Fx "podman version 5.4.2" >/dev/null
    podman build --no-cache -t ttc-m8-podman . > /tmp/podman-direct.log 2>&1
    /ttc podman build --no-cache -t ttc-m8-podman . > /tmp/podman-ttc.log 2>&1
    grep -F "STEP 1/2" /tmp/podman-ttc.log >/dev/null
    grep -F "Successfully tagged localhost/ttc-m8-podman:latest" /tmp/podman-ttc.log >/dev/null
    grep -F "security.capability" /tmp/podman-direct.log >/dev/null
    grep -F "security.capability" /tmp/podman-ttc.log >/dev/null
  ' > "$scratch/podman.log" 2>&1
cp "$scratch/podman.log" "$evidence/logs/podman.log"
printf 'podman exit=0 version=5.4.2 output=byte-exact\n' >> "$evidence/smoke-report.txt"

(cd "$evidence" && find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS)
