# Sesi 04 — M4 JavaScript filter engine

**Tanggal:** 2026-09-26 WIB
**Status:** partial

## Yang dikerjakan

M4 menambahkan classifier command, recognizer JS/TS, retain policy, capture dan raw replay, fixture, smoke tool nyata, serta job CI. Review PR #3 menemukan tiga masalah output-safety; ketiganya diperbaiki pada `a0c5b0561912ab7c69274df2e3e5ad8349d4acca`.

`CURRENT_STATE.md` sebelumnya masih menunjuk M3/PR #2; state itu diganti karena branch aktif dan pekerjaan terbaru sudah M4/PR #3. Keputusan M3 yang masih berlaku dibawa ke state baru.

## Perubahan

- Ditambah: parser statis `package.json`, dispatch JS/TS, recognizer per family, filter fail-open, dan fixture/smoke yang dipin.
- Diubah: streaming M3 memakai `JsFilter`; confidence serta blok diagnostic disimpan terpisah per stream.
- Ditambah setelah review: selector workspace yang belum di-resolve tetap raw, flag machine-readable di dalam script manifest memaksa raw, dan lokasi `nama-file:baris` tanpa ekstensi retained.
- Kode tersentuh: `src/core/classification.rs`, `src/core/filters/mod.rs`, `src/core/execution.rs`, `src/core/streaming.rs`, `tests/`, `.github/workflows/ci.yml`, `scripts/`, `README.md`, `ai_docs/TODO.md`.

## Keputusan

- `serde_json = 1.0.145` dipakai untuk parsing manifest; pembacaan manifest dibatasi 1 MiB.
- Output tak dikenal, machine-readable, dan selector workspace yang belum punya manifest package terpilih berjalan raw. Resolusi workspace penuh menunggu M6.
- ANSI SGR hanya dibuang untuk klasifikasi; byte record retained tetap asli. Confidence dan diagnostic state independen antara stdout dan stderr.
- Keputusan M3 tetap berlaku: satu binary crate, command dijalankan sekali, config hanya memperkecil batas capture, capture maksimum 32 MiB dengan retention 24 jam, dan raw replay tidak menjalankan ulang command.

## Verifikasi

- `cargo test --test classifier` — lulus pada kode `a0c5b05`.
- `cargo test --test filter_safety` — lulus pada kode `a0c5b05`.
- `cargo test --test javascript_fixtures` — lulus pada kode `a0c5b05`.
- `cargo test --test javascript_e2e` — lulus pada kode `a0c5b05`.
- `cargo test --test reduction` — lulus pada kode `a0c5b05`.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets --all-features`, dan `cargo build --release` — lulus pada kode `a0c5b05`.
- `sh scripts/m4-reduction.sh target/m4-evidence/reduction.txt` dan `sh scripts/m4-smoke.sh target/release/ttc target/m4-evidence/report.txt` — lulus pada kode `a0c5b05`.
- [CI run 36113907536](https://github.com/Skirja/ttc-ai/actions/runs/36113907536) — empat job lulus untuk `a0c5b0561912ab7c69274df2e3e5ad8349d4acca`; artifact M4 diunduh dan diperiksa.
- CI untuk commit dokumentasi terbaru `229a879` adalah run `36114119552`; status terakhir yang terlihat: pending. Hasil setelah interupsi tidak diperiksa.
- Dites manual: sebagian — smoke membandingkan command langsung dan TTC untuk Vitest, Jest, lint, typecheck, build, format-check, install, serta failure Vitest.

## Review

Direview: delta `master...feat/m4-js-filter` pada PR #3 → NEEDS CHANGES, tiga temuan blocking. Setelah review: ketiga temuan diperbaiki pada `a0c5b05`; tes regresi, gate lokal, dan CI run `36113907536` lulus. Review ulang formal setelah fix belum dilakukan.

## Batasan saat ini

Workspace selectors berjalan raw sampai M6 menyelesaikan resolusi manifest package terpilih. Fallback signature untuk runner tersembunyi, mixed-language, serta ecosystem non-JS/TS belum dikerjakan. PR #3 masih terbuka. Status CI untuk commit dokumentasi terakhir belum diketahui setelah interupsi.

## Next action

Periksa CI run `36114119552` untuk HEAD PR #3, lalu minta review ulang formal atas delta terbaru.
