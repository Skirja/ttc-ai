#!/bin/sh
# Installer stable latest-only untuk Linux x86_64 GNU.
set -eu

fail() { printf 'ttc install: %s\n' "$*" >&2; exit 1; }
test "$#" -eq 0 || fail 'Installer tidak menerima version selector atau argumen.'
for utility in curl sha256sum awk uname chmod mktemp; do
  command -v "$utility" >/dev/null 2>&1 || fail "Prerequisite tidak tersedia: $utility"
done
test "$(uname -s)" = Linux && test "$(uname -m)" = x86_64 \
  || fail 'Platform tidak didukung; diperlukan Linux x86_64 GNU.'
case "${HOME:-}" in /*) ;; *) fail 'HOME harus absolute dan tersedia.' ;; esac

release_base=https://github.com/Skirja/ttc-ai
protocols='=https'
if [ -n "${TTC_INTERNAL_TEST_RELEASE_BASE_URL:-}" ]; then
  test "${TTC_INTERNAL_TEST_INSTALL:-}" = 1 || fail 'Override endpoint memerlukan opt-in flow test.'
  case "$TTC_INTERNAL_TEST_RELEASE_BASE_URL" in
    http://127.0.0.1:*)
      test_port=${TTC_INTERNAL_TEST_RELEASE_BASE_URL#http://127.0.0.1:}
      case "$test_port" in ''|*[!0-9]*) fail 'Endpoint test harus HTTP loopback dengan port numerik.' ;; esac
      ;;
    *) fail 'Endpoint test harus HTTP loopback.' ;;
  esac
  release_base=$TTC_INTERNAL_TEST_RELEASE_BASE_URL
  protocols='=http,https'
fi

ttc_stage=$(mktemp -d "${TMPDIR:-/tmp}/ttc-install.XXXXXX")
trap 'rm -rf "$ttc_stage"' EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

latest_url=$(curl --proto "$protocols" --proto-redir "$protocols" --tlsv1.2 \
  --fail --location --silent --show-error --output /dev/null --write-out '%{url_effective}' \
  "$release_base/releases/latest") || fail 'Tidak dapat menemukan release stable terbaru.'
release_tag=${latest_url##*/}
test "$latest_url" = "$release_base/releases/tag/$release_tag" || fail 'Redirect latest tidak valid.'
case "$release_tag" in v*) release_version=${release_tag#v} ;; *) fail 'Tag release bukan stable SemVer.' ;; esac
printf '%s\n' "$release_version" | awk -F. '
  NF != 3 { exit 1 }
  { for (i = 1; i <= 3; i++) if ($i !~ /^[0-9]+$/ || (length($i) > 1 && substr($i, 1, 1) == "0")) exit 1 }
' || fail 'Tag release bukan stable SemVer.'

asset=ttc-x86_64-unknown-linux-gnu
download_base="$release_base/releases/download/$release_tag"
curl --proto "$protocols" --proto-redir "$protocols" --tlsv1.2 --fail --location --silent --show-error \
  "$download_base/$asset" --output "$ttc_stage/$asset" || fail 'Download executable gagal.'
curl --proto "$protocols" --proto-redir "$protocols" --tlsv1.2 --fail --location --silent --show-error \
  "$download_base/SHA256SUMS" --output "$ttc_stage/SHA256SUMS" || fail 'Download checksum gagal.'
expected=$(awk -v asset="$asset" '
  $2 == asset || $2 == "*" asset {
    if (NF != 2 || length($1) != 64 || $1 !~ /^[0-9a-fA-F]+$/) exit 1
    count++; checksum = tolower($1)
  }
  END { if (count != 1) exit 1; print checksum }
' "$ttc_stage/SHA256SUMS") || fail 'Checksum executable hilang, invalid, atau ambigu.'
actual=$(sha256sum < "$ttc_stage/$asset" | awk '{print $1}')
test "$actual" = "$expected" || fail 'SHA-256 tidak cocok; binary existing dipertahankan.'
chmod 755 "$ttc_stage/$asset"
binary_version=$("$ttc_stage/$asset" --version) || fail 'Executable GNU tidak dapat dijalankan pada lingkungan ini.'
test "$binary_version" = "ttc $release_version" || fail 'Versi executable tidak cocok dengan tag release.'
"$ttc_stage/$asset" __install --sha256 "$expected"
