# TTC

TTC adalah wrapper command lokal untuk mengurangi output Bash yang masuk ke
konteks coding agent tanpa mengubah perilaku command asli.

Repository ini sedang dibangun ulang secara greenfield sebagai satu binary
crate Rust. Kontrak produk, safety property, dan urutan implementasi berada di
[`ai_docs/SPEC.md`](ai_docs/SPEC.md) dan [`ai_docs/TODO.md`](ai_docs/TODO.md).

## Status

Fondasi executable M1 menyediakan binary `ttc` versi `0.1.0` dan baseline CI.
M2 menambahkan eksekusi direct argv dan shell string dengan output passthrough,
exit code, dan sinyal yang dipertahankan. M3 menambahkan streaming bounded,
config opsional, serta `ttc raw ID`. M4 menambahkan filter konservatif untuk
output JavaScript/TypeScript yang dikenali. Output yang tidak dikenali tetap raw;
kompaksi menyediakan ID untuk replay output asli.

## Pengembangan

Toolchain Rust dipin melalui `rust-toolchain.toml`. Gate repository saat ini:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build
cargo build --release
```

Lihat [`tests/fixtures/README.md`](tests/fixtures/README.md) sebelum menambahkan
fixture atau integration test baru.

## Lisensi

MIT. Lihat [`LICENSE`](LICENSE).
