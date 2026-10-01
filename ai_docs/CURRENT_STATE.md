# Current State

**Diperbarui:** 2026-10-01 WIB

## Sedang dikerjakan

- Prasyarat M9 — partial. Branch `feat/m7-m8-ci-prerequisites` memperbaiki
  setup Java/Swift CI dan test direct-argv signal M7.
- Implementasi M9 belum dimulai; rencana pengguna mensyaratkan bukti M1–M8
  lengkap terlebih dahulu.

## Terakhir selesai

- M7 dan M8 telah merged ke `master` melalui PR #6/#7. Head master
  `f55d434d1d9157a62dfd810b402cac1861d29adf`.
- Rerun master CI `36699671621`, attempt 2, memperoleh runner pada 2026-10-01.
  Billing tidak lagi menolak run ini. Job M7/M8 gagal pada setup tool, dan
  baseline Rust gagal pada test signal M7 yang memakai single-string shell.
- Perbaikan lokal lulus format, Clippy, seluruh-target/fitur Rust test,
  release build, targeted M7 E2E/signal/shell, dan parse YAML workflow.
  Log baseline: `target/m7-m8-ci-evidence/local-gates.log`.
- Kode tersentuh: `.github/workflows/ci.yml`, `scripts/m7-tool-versions.txt`,
  dan `tests/additional_ecosystem_e2e.rs`.

## Keputusan yang dikunci

- TTC tetap satu Rust binary crate; command asli dijalankan tepat satu kali.
- Output default dipertahankan. Output ambigu, machine-readable tanpa parser
  lossless, custom app, dan record multi-package tanpa prefix terdaftar raw.
- Discovery manifest statis dan bounded; kegagalan parse, cycle, ambiguity,
  atau overflow membuat invocation raw.
- Runner asli mengatur selection, dependency order, concurrency, cache, cwd,
  stdin, dan environment.
- Jangan mulai M9 sebelum evidence wajib M1–M8 lengkap; jangan mulai Codex
  integration M10 sebelum M1–M9 dan evidence CI M9 lengkap.
- Pengguna melakukan merge PR ke `master`; agent tidak melakukan merge,
  membuat tag, atau memublikasikan release pada tugas M9.

## Temuan / blocker terbuka

- M7/M8 acceptance CI dan artifact masih belum lengkap. Pin Java action
  diperbaiki ke `21.0.12+1.0.1`; runtime tetap `21.0.12.1+1`.
- Action Swift dipin ke SHA v2.4.0 untuk tetap memakai Swift `6.1.2`.
- Perbaikan setup dan smoke penuh harus diverifikasi melalui PR CI.

## Batasan yang diketahui

- State lama yang menyebut PR #6 terbuka dan tidak ada runner sudah tidak
  sesuai repo/run saat ini; PR #6/#7 telah merged dan rerun memperoleh runner.
- Evidence lokal M7/M8 tetap berada pada direktori ignored `target/`;
  keberhasilan lokal belum melengkapi gate CI.
- Instalasi, uninstall global, dan distribusi M9 belum diimplementasikan.

## Next action

- Jalankan dan periksa PR CI perbaikan prasyarat sampai evidence lengkap;
  setelah pengguna merge, verifikasi master sebelum mulai M9.

## Arsip terakhir

- `ai_docs/steps_done/07-m7-php-jvm-dotnet-local-implementation.md`
