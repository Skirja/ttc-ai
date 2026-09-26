# Current State

**Diperbarui:** 2026-09-26 WIB

## Sedang dikerjakan

- M4 — Filter engine dan JavaScript/TypeScript vertical slice — implementasi dan CI lulus pada commit `a0c5b0561912ab7c69274df2e3e5ad8349d4acca`; [PR #3](https://github.com/Skirja/ttc-ai/pull/3) terbuka. Commit dokumentasi terbaru `229a879` memiliki CI run `36114119552` yang terakhir terlihat pending.

## Terakhir selesai

- Filter fail-open JS/TS, capture/raw replay, fixture family, smoke tool nyata, dan bukti reduksi M4 selesai.
- Tiga temuan review tentang workspace hint, flag machine-readable di manifest, dan path tanpa ekstensi diperbaiki serta dites.
- Kode tersentuh: `src/core/classification.rs`, `src/core/filters/mod.rs`, `src/core/execution.rs`, `src/core/streaming.rs`, `tests/`, `scripts/`, `.github/workflows/ci.yml`, `README.md`, `ai_docs/TODO.md`.
- CI run `36113907536` lulus pada commit `a0c5b0561912ab7c69274df2e3e5ad8349d4acca`; artifact M4 diperiksa.

## Keputusan yang dikunci

- TTC tetap satu binary crate versi `0.1.0` dari `CARGO_PKG_VERSION`; `nix`/`signal-hook` menangani sinyal dan `toml` memparse config.
- Branch implementasi memakai `feat/<topik>`, push dan PR ke `master`; pengguna melakukan merge. Remote repo tetap `Skirja/ttc-ai`; visibilitas tidak diubah sebagai bagian pekerjaan ini.
- Command asli dijalankan tepat sekali. Command dengan `&` yang terlihat pada argv memakai jalur `exec` agar proses background tidak menahan wrapper.
- Config hanya memperkecil batas capture 32 MiB/24 jam; config invalid berjalan raw. `--tail N` menghitung byte dan selector raw menulis ke stdout.
- Capture fallback stabil di `/tmp/ttc-<uid>/runs`; capture terakhir yang valid bisa direplay dari jurnal/file `.part` setelah kegagalan tulis/finalisasi selama file masih terbaca.
- Filter JS/TS mempertahankan output default; ANSI SGR hanya dipakai untuk klasifikasi. Confidence dan blok diagnostic per stream. Manifest dibaca statis dengan batas 1 MiB menggunakan `serde_json = 1.0.145`.
- Flag machine-readable yang ada di argv atau package script memaksa raw. Workspace selector yang belum di-resolve ke manifest package terpilih juga raw sampai M6.

## Temuan / blocker terbuka

- Tidak ada blocker teknis yang diketahui dari verifikasi lokal dan CI run `36113907536`.
- Review PR #3 menemukan tiga temuan blocking yang telah diperbaiki; review formal ulang atas diff sesudah fix belum dilakukan.
- Hasil run CI `36114119552` untuk commit dokumentasi terbaru belum diperiksa setelah interupsi.

## Batasan yang diketahui

- Resolusi workspace penuh, alias lintas proyek, fallback signature untuk runner tersembunyi, dan mixed-language mengikuti M6.
- Filter ecosystem non-JS/TS M5, M7, dan M8 belum dikerjakan.
- `target/release/ttc` dan evidence CI yang diunduh adalah artifact lokal; tidak di-commit.

## Next action

- Periksa CI run `36114119552`, lalu minta review ulang formal PR #3 sebelum pengguna merge.

## Arsip terakhir

- `ai_docs/steps_done/04-m4-javascript-filter-engine.md`
