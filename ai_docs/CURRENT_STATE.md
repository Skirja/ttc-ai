# Current State

**Diperbarui:** 2026-09-26 WIB

## Sedang dikerjakan

- M5 — Filter Rust, Python, dan Go: implementasi serta perbaikan review selesai
  pada branch `feat/m5-core-ecosystems`; [PR #4](https://github.com/Skirja/ttc-ai/pull/4)
  terbuka. Kode terakhir `d582ea2`; dokumentasi bukti `dc0f885`. Semua lima job
  CI lulus pada run `36224167272`.

## Terakhir selesai

- Classifier dan filter khusus Cargo/Rust, Python, dan Go; parser `go test -json`
  mempertahankan lifecycle byte-exact dan hanya mengompaksi frame PASS yang
  cocok. Empat temuan review diperbaiki dan test regresi ditambahkan.
- Kode tersentuh pada sesi terakhir: `src/core/classification.rs`,
  `src/core/filters/ecosystems.rs`, `.github/workflows/ci.yml`,
  `tests/classifier.rs`, `tests/rust_fixtures.rs`,
  `tests/core_ecosystem_e2e.rs`, dan `ai_docs/TODO.md`.
- Artifact `m5-core-ecosystem-evidence` dari CI run `36223832943` diunduh dan
  diperiksa; versi tool serta SHA-256 tercatat di `ai_docs/TODO.md`.

## Keputusan yang dikunci

- TTC tetap satu Rust binary crate; command asli dijalankan tepat sekali.
- Output default dipertahankan. Kompaksi hanya memakai recognizer spesifik yang
  dikenal; parsing ambigu, machine-readable tanpa parser lossless, dan generic
  application berjalan raw. Diagnostic yang membuka blok retensi membuat sisa
  stream terkait tetap raw, termasuk melewati baris kosong.
- `go test -json` mempertahankan seluruh event lifecycle byte-exact. Hanya
  event output frame PASS dengan identitas test yang cocok boleh dikompaksi;
  event ambiguous/malformed membuat sisa invocation raw.
- Classifier wrapper `uv run` hanya melewati opsi yang dikenal beserta
  argumennya; opsi ambigu berjalan raw. `coverage run` hanya memilih parser
  module sebelum target script.
- Config hanya memperkecil batas capture 32 MiB/24 jam; config invalid berjalan
  raw. Capture fallback stabil di `/tmp/ttc-<uid>/runs` dan raw replay memakai
  capture invocation TTC sendiri.
- Manifest dibaca statis dengan batas 1 MiB. Resolusi workspace penuh, alias
  lintas proyek, fallback runner tersembunyi, dan mixed-language mengikuti M6.
- Implementasi memakai branch `feat/<topik>` dan PR ke `master`; pengguna
  melakukan merge. Codex integration M10 menunggu M1–M9 dan evidence CI M9.

## Temuan / blocker terbuka

- Tidak ada blocker kode atau CI yang diketahui. Empat temuan review pada
  `dc0f885` diperbaiki di `d582ea2` dan gate lokal/CI lulus.
- Review formal ulang atas diff setelah perbaikan belum dilakukan.

## Batasan yang diketahui

- Resolusi monorepo/workspace dan fallback runner tersembunyi belum termasuk
  M5; ikuti urutan milestone di `ai_docs/TODO.md`.
- Artifact smoke dan evidence CI yang diunduh berada di `target/` dan tidak
  di-commit.
- PR #4 masih terbuka; pengguna yang melakukan merge.

## Next action

- Jalankan review-code ulang pada diff terbaru PR #4 sebelum merge.

## Arsip terakhir

- `ai_docs/steps_done/05-m5-review-fixes.md`
