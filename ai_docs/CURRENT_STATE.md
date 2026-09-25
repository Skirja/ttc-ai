# Current State

**Diperbarui:** 2026-09-24 WIB

## Sedang dikerjakan

- M3 — Streaming safety, configuration, dan raw retrieval — completed pada
  branch `feat/m3-streaming-raw`; [PR #2](https://github.com/Skirja/ttc-ai/pull/2)
  terbuka dan menunggu review/merge pengguna.

## Terakhir selesai

- Bounded streaming, framing 1 MiB, config opsional, raw capture 32 MiB, serta
  replay `ttc raw` selesai. Tiga temuan review ditangani pada `d5c00ca`.
- Kode tersentuh: `src/core/streaming.rs`, `src/core/raw_store.rs`,
  `src/core/config.rs`, `src/core/execution.rs`, `src/cli.rs`, `tests/`,
  `scripts/`, `.github/workflows/ci.yml`, `ai_docs/SPEC.md`, dan
  `ai_docs/TODO.md`.
- CI terakhir [run 35976973682](https://github.com/Skirja/ttc-ai/actions/runs/35976973682)
  lulus untuk HEAD `70d6d21edc5f318470603e10a527ca60524f5fad`.

## Keputusan yang dikunci

- TTC tetap satu binary crate versi `0.1.0` dari `CARGO_PKG_VERSION`.
  `nix`/`signal-hook` menangani sinyal; `toml` memparse config.
- Branch implementasi memakai `feat/<topik>`, push dan PR ke `master`;
  pengguna melakukan merge. Repository `Skirja/ttc-ai` privat saat M2 dan
  visibilitasnya tidak diubah pada sesi ini.
- Command dengan `&` yang terlihat pada argv tetap memakai jalur `exec` agar
  waktu selesai proses background terjaga.
- Config hanya boleh memperkecil batas 32 MiB/24 jam; config invalid berjalan
  raw. `--tail N` menghitung byte dan selector raw menulis ke stdout.
- Fallback capture stabil di `/tmp/ttc-<uid>/runs`; capture terakhir yang valid
  dapat direplay dari jurnal/file `.part` setelah penulisan atau finalisasi
  gagal selama file masih dapat dibaca.

## Temuan / blocker terbuka

- Tidak ada blocker teknis yang diketahui. Review awal PR #2 berstatus
  NEEDS CHANGES; tiga temuannya telah diperbaiki dan CI lulus, tetapi kode
  setelah fix belum direview ulang secara formal.
- PR #2 menunggu review/merge pengguna. PR #1 sudah di-merge ke `master`.

## Batasan yang diketahui

- Filtering produksi belum tersedia sampai M4; M3 tetap raw dan memakai
  filter sintetis hanya di tes.
- `target/release/ttc` adalah artifact lokal dan tidak di-commit. Analisis
  shell arbitrer tetap di luar SPEC; routing background mengenali `&` yang
  terlihat pada argv.
- Catatan sesi ini dan `CURRENT_STATE.md` belum di-commit.

## Next action

- Commit dan push kedua catatan state lokal ini ke branch `feat/m3-streaming-raw`.

## Arsip terakhir

- `ai_docs/steps_done/03-m3-streaming-raw-retrieval.md`
