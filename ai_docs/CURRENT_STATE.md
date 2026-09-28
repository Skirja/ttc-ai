# Current State

**Diperbarui:** 2026-09-28 WIB

## Sedang dikerjakan

- M6 — Package scripts dan monorepo core selesai serta semua gate CI lulus.
  Branch `feat/m6-package-monorepo`; [PR #5](https://github.com/Skirja/ttc-ai/pull/5)
  terbuka untuk merge pengguna. Temuan review M6 telah diperbaiki pada commit
  `bad9830fb69dde7b09c547edaadd8eb36bb5beda`; smoke CI distabilkan pada
  `8e11a870adf41a967c8f3971028573d030a4b60e`. Keenam required job lulus pada
  [CI run 36364865946](https://github.com/Skirja/ttc-ai/actions/runs/36364865946).

## Terakhir selesai

- Discovery manifest bounded dan statis, resolver scripts/lifecycle manager,
  adapter runner, filter mixed-language, serta smoke pinned M6.
- Fixture 2.000 record mempertahankan satu root invocation dan dua package
  invocation; output total turun 95.780 → 381 byte (99,6%).
- Seluruh enam job PR CI lulus. Artifact `m6-monorepo-evidence` diunduh dan
  checksum-nya diverifikasi di `ai_docs/TODO.md`, termasuk evidence sesudah
  perbaikan review.
- Regresi meliputi prefix warning, konflik target Nx dengan package scripts,
  pemilihan manifest `npm --prefix`, `npm run install`, dan background job
  pada nested shell. Gate Rust penuh dan smoke M6 pinned lulus setelah fix.
- Perubahan utama: `src/core/classification.rs`, `src/core/manifests.rs`,
  `src/core/filters/`, `scripts/m6-smoke.sh`, `.github/workflows/ci.yml`,
  serta fixture dan dokumentasi M6.

## Keputusan yang dikunci

- TTC tetap satu Rust binary crate; command asli selalu dijalankan tepat sekali.
- Output default dipertahankan. Kompaksi memakai recognizer spesifik; output
  ambigu, machine-readable tanpa parser lossless, dan aplikasi generik raw.
- Manifest dibaca statis dengan batas SPEC, tanpa menjalankan discovery command.
  Kegagalan parse, cycle, ambiguity, atau overflow membuat invocation raw.
- Runner asli mengatur package selection, dependency graph, urutan, concurrency,
  cache, cwd, stdin, dan environment. Record multi-package tanpa prefix sumber
  yang dikenal tetap raw.
- Diagnostic tanpa identitas sumber mempertahankan sisa stream terkait. Raw
  replay membaca capture invocation TTC sendiri.
- Implementasi memakai branch `feat/<topik>` dan PR ke `master`; pengguna
  melakukan merge. Codex integration M10 menunggu M1–M9 dan evidence CI M9.

## Temuan / blocker terbuka

- Tidak ada blocker implementasi atau CI yang diketahui.
- Lima temuan review branch sudah diperbaiki; tidak ada finding yang masih
  terbuka dari review tersebut.

## Batasan yang diketahui

- PR #5 masih terbuka; merge dilakukan pengguna.
- Artifact CI dan report lokal berada di `target/` dan tidak di-commit.
- Output multi-package tanpa prefix yang dapat diidentifikasi dipertahankan raw,
  termasuk sebagian smoke npm/Yarn/Lerna.

## Next action

- Pengguna merge PR #5.

## Arsip terakhir

- `ai_docs/steps_done/05-m5-review-fixes.md`
