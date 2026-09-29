# Current State

**Diperbarui:** 2026-09-29 WIB

## Sedang dikerjakan

- M7 — partial. Implementasi lengkap dan local verification lulus pada commit
  `71462d4c271092c26d975b28128d6699082c080e`; branch
  `feat/m7-php-jvm-dotnet`, PR #6 terbuka ke `master`.
- Required GitHub CI belum berjalan: run `36550214966` gagal sebelum runner
  dialokasikan karena notifikasi billing/spending limit akun. Tidak ada job
  step atau artifact; jangan menandai M7 selesai sampai workflow berhasil.

## Terakhir selesai

- M6 sudah merged ke `master` melalui PR #5 pada 2026-09-28.
- Implementasi M7 mencakup PHP/Composer, Maven/Gradle/JUnit, .NET, bounded
  static manifest hints, fixtures, pinned isolated smoke project, dan job CI.
- Format, Clippy, seluruh Rust test target/fitur, release build, serta pinned
  smoke nyata M7 lulus lokal. Smoke report SHA-256:
  `e314572467670f803c5243619450bf1844b0a865f327fafc6e7a75a15d732bd9`.
- Kode tersentuh: `src/core/classification.rs`, `src/core/filters/`,
  `src/core/manifests.rs`, `tests/`, `scripts/m7-*`, `.github/workflows/ci.yml`,
  dan `ai_docs/SPEC.md`.

## Keputusan yang dikunci

- TTC tetap satu Rust binary crate; command asli dijalankan tepat satu kali.
- Output default dipertahankan. Output ambigu, machine-readable tanpa parser
  lossless, custom app, dan record multi-package tanpa prefix terdaftar raw.
- Discovery manifest statis dan bounded; kegagalan parse, cycle, ambiguity,
  atau overflow membuat invocation raw.
- Runner asli mengatur selection, dependency order, concurrency, cache, cwd,
  stdin, dan environment.
- Jangan mulai Codex integration M10 sebelum M1–M9 dan evidence CI M9 lengkap.
- Branch aktif `feat/m7-php-jvm-dotnet`, target PR `master`; pengguna yang
  melakukan merge.

## Temuan / blocker terbuka

- Runner GitHub Actions tidak dialokasikan karena masalah billing/spending
  limit akun; tidak ada temuan code failure dari run tersebut.
- Artifact `m7-php-jvm-dotnet-evidence` belum tersedia. Local smoke/report ada
  di `target/m7-evidence/` dan tidak di-commit.

## Batasan yang diketahui

- PR #6 masih terbuka; commit `71462d4` sudah dipush.
- M7 belum lengkap sampai required CI hijau dan artifact CI diperiksa.
- Local M7 smoke report/log berada pada direktori ignored `target/`.

## Next action

- Setelah masalah billing GitHub dipulihkan, rerun workflow PR #6; periksa semua
  job dan artifact, lalu perbarui TODO/state dengan run serta checksum aktual.

## Arsip terakhir

- `ai_docs/steps_done/07-m7-php-jvm-dotnet-local-implementation.md`
