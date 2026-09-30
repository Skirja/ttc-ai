#!/bin/sh
set -eu

tool_root=${1:-${M7_TOOL_HOME:-${RUNNER_TEMP:-target}/m7-tools}}
maven_version=3.9.16
gradle_version=9.8.0
maven_sha256=5af3b743dd8b876b5c45da33b676251e5f1687712644abb4ee519ca56e1d89ce
gradle_sha256=bafd5ce9cfaea0fbccfdc8439a1ac42fbd4cd9c89dc9a988228d8a2639a58e6c

mkdir -p "$tool_root/downloads"
maven_archive="$tool_root/downloads/apache-maven-$maven_version-bin.zip"
gradle_archive="$tool_root/downloads/gradle-$gradle_version-bin.zip"

if [ ! -f "$maven_archive" ]; then
  curl --fail --location --silent --show-error --retry 3 \
    "https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven/$maven_version/apache-maven-$maven_version-bin.zip" \
    --output "$maven_archive"
fi
printf '%s  %s\n' "$maven_sha256" "$maven_archive" | sha256sum --check --status
if [ ! -x "$tool_root/apache-maven-$maven_version/bin/mvn" ]; then
  if command -v unzip >/dev/null 2>&1; then
    unzip -q "$maven_archive" -d "$tool_root"
  else
    (cd "$tool_root" && jar xf "$maven_archive")
  fi
fi

if [ ! -x "$tool_root/gradle-$gradle_version/bin/gradle" ]; then
  if [ ! -f "$gradle_archive" ]; then
    curl --fail --location --silent --show-error --retry 3 \
      "https://services.gradle.org/distributions/gradle-$gradle_version-bin.zip" \
      --output "$gradle_archive"
  fi
  printf '%s  %s\n' "$gradle_sha256" "$gradle_archive" | sha256sum --check --status
  if command -v unzip >/dev/null 2>&1; then
    unzip -q "$gradle_archive" -d "$tool_root"
  else
    (cd "$tool_root" && jar xf "$gradle_archive")
  fi
fi

export M7_TOOL_HOME="$tool_root"
export M7_MAVEN_SHA256="$(sha256sum "$maven_archive" | cut -d ' ' -f 1)"
export M7_GRADLE_SHA256="$gradle_sha256"
export MAVEN_HOME="$tool_root/apache-maven-$maven_version"
export GRADLE_HOME="$tool_root/gradle-$gradle_version"
export PATH="$MAVEN_HOME/bin:$GRADLE_HOME/bin:$PATH"

test "$(mvn --version | awk 'NR == 1 {print $3}')" = "$maven_version"
test "$(gradle --version | awk '/^Gradle / {print $2; exit}')" = "$gradle_version"
