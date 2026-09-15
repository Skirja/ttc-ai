# Sesi 01 — M1 foundation

**Tanggal:** 2026-09-15 14:41 WIB
**Status:** completed

## Yang dikerjakan

Membuat fondasi satu binary crate Rust `ttc` versi `0.1.0` pada branch
`feat/m1-foundation`, termasuk kontrak CLI minimum, batas modul core/harness,
README, lisensi, konvensi test, dan workflow CI baseline.

## Perubahan

- Ditambah: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.gitignore`,
  `LICENSE`, `README.md`, `src/`, `tests/`, dan `.github/workflows/ci.yml`.
- Diubah: checklist serta evidence M1 di `ai_docs/TODO.md`.
- Dihapus: tidak ada.

## Keputusan

- Branch yang digunakan adalah `feat/m1-foundation`.
- CLI M1 hanya menyediakan `--help`/`-h`, `--version`/`-V`, dan help untuk
  invocation tanpa argumen; command execution ditunda ke M2.
- Versi package dan binary memakai satu sumber `CARGO_PKG_VERSION`.
- Tidak ada production dependency, daemon, database, telemetry, atau network
  client.
- M1 ditutup setelah seluruh verifikasi lokal lulus; commit dan CI remote tidak
  menjadi pekerjaan sesi ini.

## Verifikasi

- `cargo fmt --all -- --check` — lulus.
- `cargo clippy --all-targets --all-features -- -D warnings` — lulus.
- `cargo test --all-targets` — lulus; 3 integration test.
- `cargo test --all-targets --all-features` — lulus; 3 integration test.
- `cargo build` — lulus.
- `cargo build --release` — lulus.
- `./target/release/ttc --version` — menghasilkan `ttc 0.1.0`.
- `./target/release/ttc --help` — hanya menampilkan interface M1.
- `cargo metadata --format-version 1 --no-deps` — satu package dan satu binary.
- `cargo tree --edges normal` — tidak ada production dependency.
- `git diff --check` — lulus.
- Dites manual: ya — smoke test release binary dan penolakan argumen
  unsupported dengan exit code 2.

## Review

Direview: final worktree dan hasil gate lokal → APPROVE WITH SUGGESTIONS.
Setelah review: tidak ada perubahan tambahan pada kode; commit dan CI masih
menjadi tindak lanjut.

## Batasan saat ini

- Eksekusi command dan filtering belum diimplementasikan; itu scope M2+.

## Next action

Mulai M2 — process wrapper dan exact passthrough.
