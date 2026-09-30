#!/bin/sh
set -eu

tool_home=${M8_TOOL_HOME:?M8_TOOL_HOME must name a task-scoped directory}
mkdir -p "$tool_home/downloads" "$tool_home/bin" "$tool_home/src"
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
checksums="$script_dir/m8-tool-checksums.txt"

download() {
  url=$1
  name=$2
  curl --fail --location --silent --show-error --retry 3 "$url" --output "$tool_home/downloads/$name"
  (cd "$tool_home/downloads" && grep "  $name$" "$checksums" | sha256sum --check --status)
}

download https://cmake.org/files/v3.31/cmake-3.31.6-linux-x86_64.tar.gz cmake-3.31.6-linux-x86_64.tar.gz
tar -xzf "$tool_home/downloads/cmake-3.31.6-linux-x86_64.tar.gz" -C "$tool_home"

download https://github.com/ninja-build/ninja/releases/download/v1.12.1/ninja-linux.zip ninja-linux.zip
mkdir -p "$tool_home/src/ninja"
unzip -qo "$tool_home/downloads/ninja-linux.zip" -d "$tool_home/src/ninja"
install -m 0755 "$tool_home/src/ninja/ninja" "$tool_home/bin/ninja"

download https://ftp.gnu.org/gnu/make/make-4.4.1.tar.gz make-4.4.1.tar.gz
tar -xzf "$tool_home/downloads/make-4.4.1.tar.gz" -C "$tool_home/src"
(cd "$tool_home/src/make-4.4.1" && ./configure --prefix="$tool_home" && make -j2 && make install)

download https://releases.hashicorp.com/terraform/1.11.4/terraform_1.11.4_linux_amd64.zip terraform_1.11.4_linux_amd64.zip
unzip -qo "$tool_home/downloads/terraform_1.11.4_linux_amd64.zip" -d "$tool_home/bin"

download https://get.helm.sh/helm-v3.17.3-linux-amd64.tar.gz helm-v3.17.3-linux-amd64.tar.gz
tar -xzf "$tool_home/downloads/helm-v3.17.3-linux-amd64.tar.gz" -C "$tool_home/src"
install -m 0755 "$tool_home/src/linux-amd64/helm" "$tool_home/bin/helm"

export PATH="$tool_home/bin:$tool_home/cmake-3.31.6-linux-x86_64/bin:$PATH"
