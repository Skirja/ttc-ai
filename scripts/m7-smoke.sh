#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
binary=target/release/ttc
report=target/m7-evidence/report.txt
if [ "$#" -ge 1 ]; then binary=$1; fi
if [ "$#" -ge 2 ]; then report=$2; fi
case "$binary" in /*) ;; *) binary="$repository_root/$binary" ;; esac
case "$report" in /*) ;; *) report="$repository_root/$report" ;; esac
mkdir -p "$(dirname "$report")"
test -x "$binary"

scratch=$(mktemp -d "${TMPDIR:-/tmp}/ttc-m7.XXXXXX")
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$scratch/home" "$scratch/state" "$scratch/config" "$scratch/data" "$scratch/cache" "$scratch/tmp"
export HOME="$scratch/home" XDG_STATE_HOME="$scratch/state" XDG_CONFIG_HOME="$scratch/config"
export XDG_DATA_HOME="$scratch/data" TMPDIR="$scratch/tmp" TTC_INTERNAL_TEST_TMP_ROOT="$scratch/tmp"
export COMPOSER_HOME="$scratch/composer-home" COMPOSER_CACHE_DIR="$scratch/cache/composer"
export DOTNET_CLI_HOME="$scratch/dotnet-home" DOTNET_MULTILEVEL_LOOKUP=0
export DOTNET_NOLOGO=1 DOTNET_SKIP_FIRST_TIME_EXPERIENCE=1 NUGET_PACKAGES="$scratch/cache/nuget"
export GRADLE_USER_HOME="$scratch/cache/gradle" MAVEN_OPTS="-Dmaven.repo.local=$scratch/cache/maven"

php_version=$(php -r 'echo PHP_VERSION;')
composer_version=$(composer --version --no-ansi | awk 'NR == 1 {print $3}')
java_version=$(java -version 2>&1 | awk 'NR == 1 {gsub(/"/, "", $3); print $3}')
maven_version=$(mvn --version | awk 'NR == 1 {print $3}')
gradle_version=$(gradle --version | awk '/^Gradle / {print $2; exit}')
dotnet_version=$(dotnet --version)
test "$php_version" = 8.4.25
test "$composer_version" = 2.10.3
test "$java_version" = 21.0.12.1
test "$maven_version" = 3.9.16
test "$gradle_version" = 9.8.0
test "$dotnet_version" = 10.0.401
{
  cat "$repository_root/scripts/m7-tool-versions.txt"
  printf 'php-runtime=%s composer=%s\n' "$php_version" "$composer_version"
  printf 'java-runtime=%s maven=%s gradle=%s dotnet-sdk=%s\n' \
    "$java_version" "$maven_version" "$gradle_version" "$dotnet_version"
} > "$report"

curl --fail --location --silent --show-error --retry 3 \
  https://repo.maven.apache.org/maven2/org/junit/platform/junit-platform-console-standalone/6.0.2/junit-platform-console-standalone-6.0.2.jar \
  --output "$scratch/junit-platform-console-standalone-6.0.2.jar"
printf '%s  %s\n' \
  e4689bae97d9282b044871edf9d7f0b28466bac95048a85bf75e3c65430bcd50 \
  "$scratch/junit-platform-console-standalone-6.0.2.jar" | sha256sum --check --status

diagnostics() {
  grep -Eai '(^|[^[:alpha:]])(warnings?|errors?|fail(ed|ure|ures)?|assert(ions?)?|expected|actual|exception|stack trace|caused by|security|deprecat[a-z]*|vulnerab[a-z]*)([^[:alpha:]]|$)' "$1" \
    | grep -Eavi '^[[:space:]]*(/[^[:space:]]*/csc([[:space:]]|$)|BuildResponseFile[[:space:]]*=)' \
    | sed -E -e 's/[0-9]+(\.[0-9]+)? ms/<elapsed> ms/g' \
        -e 's/Time elapsed: [0-9]+(\.[0-9]+)? s/Time elapsed: <elapsed> s/g' \
        -e 's/Time: .*/Time: <elapsed>/g' > "$2" || true
}

compare() {
  label=$1
  reduce=$2
  expected_status=$3
  shift 3
  set +e
  "$@" > "$scratch/$label-direct.out" 2> "$scratch/$label-direct.err"
  direct_status=$?
  "$binary" "$@" > "$scratch/$label-ttc.out" 2> "$scratch/$label-ttc.err"
  ttc_status=$?
  set -e
  if [ "$direct_status" -ne "$ttc_status" ]; then
    printf '%s exit mismatch direct=%s ttc=%s\n' "$label" "$direct_status" "$ttc_status" >&2
    return 1
  fi
  if [ "$direct_status" -ne "$expected_status" ]; then
    printf '%s unexpected exit status direct=%s expected=%s\n' \
      "$label" "$direct_status" "$expected_status" >&2
    return 1
  fi
  for stream in out err; do
    diagnostics "$scratch/$label-direct.$stream" "$scratch/$label-direct-$stream.diagnostics"
    diagnostics "$scratch/$label-ttc.$stream" "$scratch/$label-ttc-$stream.diagnostics"
    cmp "$scratch/$label-direct-$stream.diagnostics" "$scratch/$label-ttc-$stream.diagnostics"
  done
  direct_out=$(wc -c < "$scratch/$label-direct.out")
  direct_err=$(wc -c < "$scratch/$label-direct.err")
  ttc_out=$(wc -c < "$scratch/$label-ttc.out")
  ttc_err=$(wc -c < "$scratch/$label-ttc.err")
  direct_total=$((direct_out + direct_err))
  ttc_total=$((ttc_out + ttc_err))
  if [ "$reduce" = yes ] && [ "$((ttc_total * 5))" -ge "$direct_total" ]; then
    printf '%s reduction below 80 percent: direct=%s ttc=%s\n' "$label" "$direct_total" "$ttc_total" >&2
    return 1
  fi
  printf '%s exit=%s bytes=%s/%s stdout=%s/%s stderr=%s/%s\n' \
    "$label" "$ttc_status" "$direct_total" "$ttc_total" \
    "$direct_out" "$ttc_out" "$direct_err" "$ttc_err" >> "$report"
}
php_project="$scratch/php"
cp -R "$repository_root/scripts/m7-smoke/php" "$php_project"
cd "$php_project"
composer install --no-interaction --no-scripts --no-progress --prefer-dist
phpunit_version=$(php vendor/bin/phpunit --version | awk 'NR == 1 {print $2}')
pest_version=$(php vendor/bin/pest --version \
  | grep -Eo '[0-9]+\.[0-9]+\.[0-9]+' \
  | head -n 1)
phpstan_version=$(php vendor/bin/phpstan --version | awk '{print $NF}')
psalm_version=$(php vendor/bin/psalm --version | awk 'NR == 1 {split($2, part, "@"); print part[1]}')
phpcs_version=$(php vendor/bin/phpcs --version | awk '{print $3}')
php_cs_fixer_version=$(php vendor/bin/php-cs-fixer --version | awk '{print $4}')
test "$phpunit_version" = 13.3.4
test "$pest_version" = 5.2.1
test "$phpstan_version" = 2.2.16
test "$psalm_version" = 6.18.1
test "$phpcs_version" = 4.0.4
test "$php_cs_fixer_version" = 3.95.27
printf 'php-tools phpunit=%s pest=%s phpstan=%s psalm=%s phpcs=%s php-cs-fixer=%s\n' \
  "$phpunit_version" "$pest_version" "$phpstan_version" "$psalm_version" \
  "$phpcs_version" "$php_cs_fixer_version" >> "$report"
compare phpunit-large yes 0 composer test
compare pest-pass no 0 composer test-pest
compare phpstan no 0 composer analyse
compare psalm no 0 composer psalm
compare phpcs no 0 composer sniff
compare php-cs-fixer no 0 composer format-check
compare php-failure no 1 php vendor/bin/phpunit --testdox --no-configuration tests/FailureTest.php
compare composer-install no 0 composer install --no-interaction --no-scripts --prefer-dist

php_raw_id=$(sed -n 's/^raw: ttc raw //p' "$scratch/phpunit-large-ttc.err" | head -n 1)
test -n "$php_raw_id"
"$binary" raw "$php_raw_id" > "$scratch/php-raw.out" 2> "$scratch/php-raw.err"
for stream in out err; do
  sed -E -e 's/[0-9]+(\.[0-9]+)? ms/<elapsed> ms/g' -e 's/Time: .*/Time: <elapsed>/g' \
    "$scratch/phpunit-large-direct.$stream" > "$scratch/php-direct-normalized.$stream"
  sed -E -e 's/[0-9]+(\.[0-9]+)? ms/<elapsed> ms/g' -e 's/Time: .*/Time: <elapsed>/g' \
    "$scratch/php-raw.$stream" > "$scratch/php-raw-normalized.$stream"
  cmp "$scratch/php-direct-normalized.$stream" "$scratch/php-raw-normalized.$stream"
done
printf 'php-raw-replay=exact-after-runtime-normalization\n' >> "$report"

junit_classes="$scratch/junit-classes"
mkdir -p "$junit_classes"
javac -cp "$scratch/junit-platform-console-standalone-6.0.2.jar" \
  -d "$junit_classes" "$repository_root/scripts/m7-smoke/jvm/junit/src/"*.java
junit_jar="$scratch/junit-platform-console-standalone-6.0.2.jar"
compare junit-large yes 0 java -jar "$junit_jar" execute --class-path "$junit_classes" \
  --select-class=JunitSmokeTests --details=tree
compare junit-failure no 1 java -jar "$junit_jar" execute --class-path "$junit_classes" \
  --select-class=JunitFailureTests --details=tree

maven_project="$scratch/maven"
cp -R "$repository_root/scripts/m7-smoke/jvm/maven" "$maven_project"
cd "$maven_project"
mvn -N org.apache.maven.plugins:maven-wrapper-plugin:3.3.4:wrapper \
  -Dmaven=3.9.16 -Dtype=bin -DdistributionSha256Sum="$M7_MAVEN_SHA256"
compare maven-test no 0 mvn -B -pl smoke-module test
compare maven-wrapper-test no 0 ./mvnw -B -pl smoke-module test

gradle_project="$scratch/gradle"
cp -R "$repository_root/scripts/m7-smoke/jvm/gradle" "$gradle_project"
cd "$gradle_project"
gradle wrapper --gradle-version=9.8.0 --distribution-type=bin
printf 'distributionSha256Sum=%s\n' \
  bafd5ce9cfaea0fbccfdc8439a1ac42fbd4cd9c89dc9a988228d8a2639a58e6c \
  >> gradle/wrapper/gradle-wrapper.properties
gradle --write-locks test
compare gradle-test no 0 gradle test
compare gradle-wrapper-build no 0 ./gradlew build

dotnet_project="$scratch/dotnet"
mkdir -p "$dotnet_project"
tar -C "$repository_root/scripts/m7-smoke/dotnet" \
  --exclude='*/bin' --exclude='*/obj' -cf "$scratch/dotnet-project.tar" .
tar -C "$dotnet_project" -xf "$scratch/dotnet-project.tar"
cd "$dotnet_project"
compare dotnet-restore no 0 dotnet restore Tests/Tests.csproj --locked-mode
dotnet build Tests/Tests.csproj --no-restore --verbosity quiet
printf 'dotnet-test-project=prebuilt-for-isolated-output-comparison\n' >> "$report"
compare dotnet-test-large yes 0 dotnet test Tests/Tests.csproj --no-restore \
  --no-build --verbosity normal --filter 'FullyQualifiedName!~FailureTests'
compare dotnet-test-failure no 1 dotnet test Tests/Tests.csproj --no-restore \
  --no-build --verbosity normal --filter 'FullyQualifiedName~FailureTests'
compare dotnet-build no 0 dotnet build App/App.csproj --no-restore
compare dotnet-publish no 0 dotnet publish App/App.csproj --no-restore
compare dotnet-format no 0 dotnet format App/App.csproj --verify-no-changes --no-restore

printf 'toolchain=isolated; smoke projects copied into task temp storage\n' >> "$report"
