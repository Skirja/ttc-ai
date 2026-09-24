# Current State

**Diperbarui:** 2026-09-24 14:25 WIB

## Sedang dikerjakan

- M2 — Process wrapper dan exact passthrough — completed pada branch
  `feat/m2-process-wrapper`; PR #1 menunggu merge pengguna.

## Terakhir selesai

- Direct argv dan single-string POSIX shell execution berjalan tepat sekali
  dengan passthrough per stream, status/sinyal, stdin, cwd, environment, dan
  TTY. Regresi background shell telah diperbaiki.
- Kode tersentuh: `src/cli.rs`, `src/core/execution.rs`, `tests/`,
  `scripts/m2-evidence.sh`, `.github/workflows/ci.yml`, `AGENTS.md`, dan
  `ai_docs/TODO.md`.

## Keputusan yang dikunci

- Satu binary crate, tanpa workspace multi-crate. Versi binary berasal dari
  `CARGO_PKG_VERSION` dan saat ini `0.1.0`.
- Production dependency `nix` dan `signal-hook` dipakai untuk propagasi sinyal.
- Branch implementasi memakai `feat/<topik>`, push dan PR ke `master`; pengguna
  melakukan merge. Repository `Skirja/ttc-ai` tetap privat saat M2.
- Command dengan `&` terlihat pada argv memakai jalur `exec` untuk menjaga
  waktu selesai proses background.

## Temuan / blocker terbuka

- Tidak ada blocker teknis yang diketahui. Review awal NEEDS CHANGES sudah
  ditindaklanjuti pada `acfca05`, tetapi kode setelah fix belum direview ulang.
- PR #1 masih menunggu review dan merge pengguna.

## Batasan yang diketahui

- `target/release/ttc` adalah artifact lokal dan tidak di-commit.
- Filtering belum tersedia pada M2; analisis shell arbitrer di luar SPEC dan
  routing background mengenali `&` yang terlihat pada argv.

## Next action

- Pengguna meninjau dan menggabungkan [PR #1](https://github.com/Skirja/ttc-ai/pull/1).

## Arsip terakhir

- `ai_docs/steps_done/02-m2-process-wrapper.md`
