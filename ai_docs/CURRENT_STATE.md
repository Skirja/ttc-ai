# Current State

**Diperbarui:** 2026-09-28 WIB

## Sedang dikerjakan

- M6 — partial untuk verifikasi branch terbaru. Lima temuan review sudah
  diperbaiki; local gate dan full pinned smoke lulus. Branch
  `feat/m6-package-monorepo`, PR #5 masih terbuka. Head terakhir:
  `332d8cdbaab31231c5639a4c98016ee836b0befa`.
- Pada CI run `36366060564`, baseline Rust dan job M2–M4/M6 lulus. M5 masih
  `in_progress` ketika sesi dihentikan, sehingga seluruh required CI pada head
  terakhir belum terkonfirmasi.

## Terakhir selesai

- Memperbaiki lima finding branch review dan menambah regresi untuk prefix
  warning, konflik Nx/package scripts, `npm --prefix`, `npm run install`, dan
  nested shell background.
- Menstabilkan smoke Yarn pada CI dan membetulkan normalisasi ANSI pada
  pembanding record.
- Gate Rust lengkap dan full smoke lokal lulus. Artifact M6 dari run
  `36366060564` diunduh dan checksum-nya diverifikasi; detail ada di
  `ai_docs/TODO.md`.
- Kode tersentuh: `src/core/classification.rs`, `src/core/manifests.rs`,
  `src/core/filters/mod.rs`, `scripts/m6-smoke.sh`, serta regresi di `tests/`.

## Keputusan yang dikunci

- TTC tetap satu Rust binary crate; command asli dijalankan tepat satu kali.
- Output default dipertahankan. Output ambigu, machine-readable tanpa parser
  lossless, custom app, dan record multi-package tanpa prefix terdaftar raw.
- Discovery manifest statis dan bounded; kegagalan parse, cycle, ambiguity,
  atau overflow membuat invocation raw.
- Runner asli mengatur selection, dependency order, concurrency, cache, cwd,
  stdin, dan environment.
- Jangan mulai Codex integration M10 sebelum M1–M9 dan evidence CI M9 lengkap.
- Branch kerja `feat/m6-package-monorepo`; target PR `master`; pengguna yang
  melakukan merge.

## Temuan / blocker terbuka

- Hasil akhir job M5 pada run `36366060564` belum diperiksa setelah interupsi.
- Review formal awal branch mendapat `NEEDS CHANGES · 5 blocking`; semua lima
  finding sudah diperbaiki, tetapi final diff belum direview ulang.

## Batasan yang diketahui

- PR #5 masih terbuka dan belum di-merge.
- Artifact smoke dan CI berada di `target/`, bukan di-commit.
- Smoke lokal dengan `CI=true FORCE_COLOR=1` lulus pada commit `332d8cd`; report
  SHA-256 `045244863845e3ba459a2a24f9525948e9ddf8a1b12d3f4be4d54c88909f52e3`.

## Next action

- Periksa penyelesaian M5 pada run `36366060564`; setelah semua CI hijau,
  lakukan review ulang final diff dan sinkronkan status M6. Pengguna merge PR #5.

## Arsip terakhir

- `ai_docs/steps_done/06-m6-review-fixes-and-smoke.md`
