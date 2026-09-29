# Sesi 07 — Implementasi lokal M7 PHP, JVM, dan .NET

**Tanggal:** 2026-09-29 WIB
**Status:** partial

## Yang dikerjakan

- Menyelesaikan classifier, recognizer, manifest hints statis, fixtures, pinned
  isolated smoke, SPEC, dan job CI M7.
- Membuat commit `71462d4c271092c26d975b28128d6699082c080e`, push branch
  `feat/m7-php-jvm-dotnet`, dan membuka PR #6 ke `master`.
- Memperbarui TODO dengan bukti lokal dan menandai gate hosted CI masih pending.

## Perubahan

- Ditambah: filter PHP/JVM/.NET, pinned smoke project dan scripts, checksum
  toolchain, fixture dan e2e tests, M7 CI job, serta bukti/state sesi.
- Diubah: classifier/manifest discovery, SPEC, TODO, dan CURRENT_STATE.
- Dihapus: artefak build lokal `bin/obj`, `.gradle`, dan `build` dari smoke
  project; tidak ada project source yang dihapus.

## Keputusan

- Maven POM memakai parser pull `quick-xml = 0.42.0` yang dipin: bounded,
  struktural, menolak DTD/external entity, dan tidak mengambil dependency atau
  parent melalui jaringan.
- Hanya grammar progress/pass yang dikenal yang dapat dikompaksi; command,
  reporter, diagnostic, atau manifest ambigu tetap raw.
- M6 PR #5 telah merged ke master pada 2026-09-28; status lama yang menunggu M5
  dan PR #5 tidak berlaku lagi.

## Verifikasi

- `cargo test --test classifier --test manifests --test filter_safety --test php_fixtures --test jvm_fixtures --test dotnet_fixtures --test additional_ecosystem_e2e --test reduction` — lulus.
- `cargo fmt --all -- --check` — lulus.
- `cargo clippy --all-targets --all-features -- -D warnings` — lulus.
- `cargo test --all-targets --all-features` — lulus.
- `cargo build --release` — lulus.
- `sh -n scripts/m7-smoke.sh`, `sh -n scripts/m7-install-jvm-tools.sh`, dan parse YAML workflow — lulus.
- Pinned PHP/Composer/PHPUnit/Pest/PHPStan/Psalm/PHPCS/php-cs-fixer, Java/JUnit/Maven/Gradle, dan .NET smoke di container terisolasi — lulus. Report `target/m7-evidence/report.txt`; checksum `e314572467670f803c5243619450bf1844b0a865f327fafc6e7a75a15d732bd9`.
- GitHub Actions run `36550214966` pada commit `71462d4` — gagal sebelum job mendapat runner karena GitHub menolak alokasi akibat billing/spending limit. Semua job memiliki nol step; ini bukan hasil test dan tidak ada artifact.
- Dites manual: ya — direct-versus-TTC status/output/diagnostic pada tool nyata; synthetic e2e membandingkan stdout/stderr, invocation count, cwd, environment, stdin, signal, dan raw capture.

## Review

Direview: diff M7 sampai commit `71462d4` → APPROVE WITH SUGGESTIONS. Pemeriksaan
menemukan flag `.NET --logger:trx` belum memaksa raw; classifier dan tes
regresinya diperbaiki sebelum commit. CI hosted masih pending karena blocker
billing di atas.

## Batasan saat ini

- Hosted required CI belum hijau dan artifact `m7-php-jvm-dotnet-evidence`
  belum dibuat. TODO mempertahankan checkbox gate CI dalam keadaan unchecked.
- Log/report smoke lokal tersimpan di direktori ignored `target/`.

## Next action

- Setelah billing GitHub dipulihkan, rerun workflow pada PR #6, verifikasi
  artifact checksums, lalu catat run/head/artifact aktual di TODO dan state.
