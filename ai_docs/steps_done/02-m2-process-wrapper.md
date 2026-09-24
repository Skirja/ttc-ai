# Sesi 02 — M2 Process Wrapper dan Exact Passthrough

**Tanggal:** 2026-09-24 14:25 WIB
**Status:** completed

## Yang dikerjakan

Binary `ttc` kini menjalankan direct argv atau satu command string POSIX tepat
sekali, meneruskan byte stdout/stderr per stream, stdin, cwd, environment,
exit code, dan sinyal. Repository GitHub privat `Skirja/ttc-ai` dibuat; M2 ada
di branch `feat/m2-process-wrapper` dan PR #1 menunggu review serta merge
pengguna.

## Perubahan

- Ditambah: empat suite M2 di `tests/`, helper temporary directory, dan
  `scripts/m2-evidence.sh` untuk perbandingan byte baseline-versus-TTC.
- Diubah: `src/cli.rs`, `src/core/execution.rs`, `Cargo.toml`, `Cargo.lock`,
  `.github/workflows/ci.yml`, `AGENTS.md`, `README.md`, dan `ai_docs/TODO.md`.
- Dihapus: tidak ada.

## Keputusan

- Satu binary crate dan versi `0.1.0` dari `CARGO_PKG_VERSION` tetap berlaku.
  `nix` dan `signal-hook` menjadi production dependency M2 untuk propagasi
  sinyal; keputusan M1 tanpa production dependency tidak lagi berlaku.
- Branch `feat/m1-foundation`, CLI tanpa eksekusi, dan status M1 belum committed
  sudah usang: M1 di-commit (`7c403fc`) lalu di-merge ke `master` (`20df407`).
- Pekerjaan implementasi memakai branch `feat/<topik>`, push, dan PR ke `master`;
  pengguna melakukan merge. Repo tetap privat sampai keputusan distribusi.
- Argumen yang memuat `&` memakai `exec` secara konservatif agar background
  shell tidak menahan pipe TTC setelah shell utama selesai.

## Verifikasi

- `cargo test --test execution`, `cargo test --test passthrough`,
  `cargo test --test signals`, dan `cargo test --test shell_contract` — lulus.
- `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test --all-targets --all-features`, dan `cargo build --release` — lulus.
- `sh scripts/m2-evidence.sh target/m2-evidence/report.txt` — lulus; stdout
  120.000 byte dan stderr 140.000 byte identik dengan baseline.
- [CI run 35969037570](https://github.com/Skirja/ttc-ai/actions/runs/35969037570)
  pada commit `0c64d90` — job Rust baseline dan M2 passthrough safety lulus;
  artifact `m2-byte-comparison` tersedia.
- Dites manual: sebagian — `./target/release/ttc --version` menghasilkan
  `ttc 0.1.0`; perilaku M2 diverifikasi lewat test otomatis dan script byte.

## Review

Direview: delta M2 terhadap `master` sebelum perbaikan → NEEDS CHANGES,
satu temuan MED: `bash -c` dengan proses background dapat menahan pipe TTC.
Setelah review: `acfca05` memperluas routing `exec` untuk ampersand pada argv
dan menambah regresi non-TTY untuk shell string, `bash -c`, dan `env`.
Kode setelah perbaikan belum menjalani `/review-code` ulang; test lokal dan CI
telah lulus.

## Batasan saat ini

- PR #1 belum di-merge; `master` belum memuat M2.
- `target/release/ttc` dan laporan lokal di `target/` bukan file yang di-commit.
- M2 belum mengaktifkan filtering. Analisis shell arbitrer berada di luar SPEC;
  routing background saat ini mengenali `&` yang terlihat pada argv.

## Next action

- Pengguna meninjau dan menggabungkan [PR #1](https://github.com/Skirja/ttc-ai/pull/1).
