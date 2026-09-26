# Sesi 05 — Perbaikan review M5

**Tanggal:** 2026-09-26 WIB
**Status:** completed

## Yang dikerjakan

Memperbaiki empat temuan review pada M5 Rust/Python/Go dan memperbarui bukti
milestone. PR #4 tetap terbuka; tidak ada merge.

## Perubahan

- Diubah: diagnostic block tidak lagi dibuka kembali oleh baris kosong, agar
  captured output kegagalan yang menyerupai passing record tetap utuh.
- Diubah: parser `uv run` membaca opsi bernilai yang dikenal beserta nilainya;
  opsi tidak dikenal tetap raw.
- Diubah: parser `coverage run` hanya mengenali `-m` sebelum target script.
- Diubah: job fixture M5 memakai `set -o pipefail` supaya error test tidak
  tertutup oleh `tee`.
- Ditambah: test classifier, raw byte-exact untuk generic Python app, dan
  fixture regresi output kegagalan setelah baris kosong.
- Diperbarui: bukti review fix, CI, dan SHA-256 artifact di `ai_docs/TODO.md`;
  state sesi di `ai_docs/CURRENT_STATE.md`.
- Dihapus: tidak ada.

## Keputusan

- Setelah diagnostic terdeteksi, pertahankan sisa output stream tersebut; baris
  kosong tidak membuktikan blok diagnostic berakhir.
- Wrapper hanya memilih parser dari opsi argv yang dapat diparse statis dan
  tidak ambigu. Target script beserta argumennya bukan opsi coverage.
- M5 tetap pada branch `feat/m5-core-ecosystems`; pengguna melakukan merge PR.

## Verifikasi

- `cargo fmt --all -- --check` — lulus.
- `cargo clippy --all-targets --all-features -- -D warnings` — lulus.
- `cargo test --test rust_fixtures` — lulus.
- `cargo test --test classifier` — lulus.
- `cargo test --test filter_safety` — lulus.
- `cargo test --test python_fixtures` — lulus.
- `cargo test --test go_fixtures` — lulus.
- `cargo test --test core_ecosystem_e2e` — lulus.
- `cargo test --test reduction` — lulus.
- `cargo test --all-targets --all-features` — lulus.
- `cargo build --release` — lulus.
- `M5_NEXTEST_BIN="$PWD/target/m5-tool-root/bin/cargo-nextest" sh scripts/m5-smoke.sh target/release/ttc target/m5-evidence/review-fixes-smoke.txt` — lulus dengan versi pin Rust 1.98.1, nextest 0.9.108, Python 3.14.7, pytest 9.1.1, Go 1.27.1.
- `git diff --check` — lulus.
- CI run `36223832943` pada commit `d582ea2` lulus seluruh lima job; artifact M5 diunduh dan SHA-256 dicatat di TODO.
- CI run terbaru `36224167272` pada commit dokumentasi `dc0f885` lulus seluruh lima job.
- Dites manual: tidak.

## Review

Direview: `master...dc0f885` → **NEEDS CHANGES** (empat temuan: diagnostic
capture, batas argumen `uv`, batas script coverage, dan `pipefail` CI).
Setelah review: temuan diperbaiki di commit `d582ea2`; regression tests dan CI
lulus. Review formal ulang untuk diff final belum dilakukan.

## Batasan saat ini

- PR #4 masih terbuka.
- Review formal ulang atas perubahan setelah fix belum dilakukan.
- Resolusi workspace/monorepo dan mixed-language tetap mengikuti M6.

## Next action

Jalankan review-code ulang pada diff terbaru PR #4 sebelum merge.
