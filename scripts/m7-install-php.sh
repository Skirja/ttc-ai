#!/bin/sh
set -eu

# setup-php resolves a minor release on Linux, so build the exact smoke pin.
tool_root=${1:-${M7_TOOL_HOME:-${RUNNER_TEMP:-target}/m7-tools}}
case "$tool_root" in /*) ;; *) tool_root="$(pwd)/$tool_root" ;; esac
version=8.4.25
checksum=dc1ad8b4109898d9db49744450403874858c23efc685b1032a50bd1e83906848
prefix="$tool_root/php-$version"
archive="$tool_root/downloads/php-$version.tar.xz"
mkdir -p "$tool_root/downloads"
if [ ! -f "$archive" ]; then
  curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error --retry 3 \
    "https://www.php.net/distributions/php-$version.tar.xz" --output "$archive"
fi
printf '%s  %s\n' "$checksum" "$archive" | sha256sum --check --status

scratch=$(mktemp -d "$tool_root/php-build.XXXXXX")
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
tar -xJf "$archive" -C "$scratch"
(
  cd "$scratch/php-$version"
  ./configure --prefix="$prefix" --disable-all --enable-cli --disable-cgi \
    --enable-ctype --enable-dom --enable-fileinfo --enable-filter \
    --enable-mbstring --enable-pcntl --enable-pdo --enable-phar --enable-posix \
    --enable-session --enable-simplexml --enable-tokenizer --enable-xml \
    --enable-xmlreader --enable-xmlwriter --with-curl --with-iconv --with-libxml \
    --with-openssl --with-zip --with-zlib
  make -j "${M7_BUILD_JOBS:-2}"
  make install
)
test "$("$prefix/bin/php" -r 'echo PHP_VERSION;')" = "$version"
printf 'PHP %s source SHA-256 %s\n' "$version" "$checksum"
