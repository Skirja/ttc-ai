# TTC

TTC adalah wrapper command lokal untuk mengurangi output test/build yang masuk
ke konteks coding agent. Command asli dijalankan sekali; diagnostic, warning,
failure, exit status, dan signal dipertahankan. Output yang tidak dikenali tetap
raw. Kompaksi menyediakan ID untuk membaca kembali output asli.

## Status

M1–M8 menyediakan execution, streaming, raw replay, recognizer ecosystem, dan
monorepo. M9 menambahkan installer serta kandidat binary Linux
`x86_64-unknown-linux-gnu` dalam artifact GitHub Actions. M9 selesai setelah
pengguna merge PR dan artifact dari clean successful `master` run diperiksa.
Integrasi Codex dan publikasi release berada pada M10.

Kontrak ada pada [`SPEC`](ai_docs/SPEC.md), urutan/evidence pada
[`TODO`](ai_docs/TODO.md), dan audit standalone pada
[`coverage M9`](ai_docs/M9_COVERAGE.md).

## Instalasi

### Quick install setelah publikasi M10

Command berikut baru tersedia setelah GitHub Release pertama dipublikasikan
pada M10:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://github.com/Skirja/ttc-ai/releases/latest/download/install.sh | sh
```

Installer membutuhkan Linux x86_64 dengan GNU runtime, Bash, `curl`,
`sha256sum`, `awk`, dan utilitas POSIX. Installer memilih stable latest,
mengunci executable/checksum pada tag yang sama, memverifikasi checksum serta
versi, lalu memasang `~/.local/bin/ttc`. Tidak ada version selector.

Jika bin belum di PATH, installer membackup `.bashrc` dan menambahkan satu blok
terkelola. Ikuti instruksi `source ~/.bashrc` atau buka shell baru. Bila config
tidak dapat diedit, binary tetap terpasang; gunakan:

```sh
export PATH="$HOME/.local/bin${PATH:+:$PATH}"
```

### Instalasi manual dengan checksum

Sebelum publikasi M10, unduh artifact kandidat dari successful CI run. Sesudah
publikasi, unduh executable `ttc-x86_64-unknown-linux-gnu` dan `SHA256SUMS` dari
**tag release yang sama** melalui HTTPS. Periksa checksum sebelum menjalankan:

```sh
sha256sum --check SHA256SUMS
chmod +x ttc-x86_64-unknown-linux-gnu
mkdir -p "$HOME/bin"
cp ttc-x86_64-unknown-linux-gnu "$HOME/bin/ttc"
"$HOME/bin/ttc" --version
```

Untuk artifact CI, file `install.sh` juga harus tersedia ketika memeriksa
`SHA256SUMS`. Instalasi manual tidak membuat metadata; installer dan global
uninstall tidak mengadopsi atau menimpa binary manual. Pilih path lain seperti
contoh di atas; hapus binary manual sendiri ketika tidak diperlukan.

### Update dan uninstall

Update dilakukan dengan menjalankan ulang installer latest. Reinstall dan
upgrade mempertahankan ownership PATH dan daftar integrasi aktif. Binary
existing dengan checksum berbeda, metadata rusak/schema asing, symlink, atau
binary yang tidak terdaftar menyebabkan penolakan tanpa overwrite.

```sh
ttc uninstall
```

Uninstall menolak bila metadata masih mencatat integrasi aktif. Binary dan
metadata milik TTC dihapus; blok PATH hanya dihapus bila masih identik dan bin
tidak berisi program lain. Config, raw capture, backup Bash, dan konten pengguna
dipertahankan. Lock privat tetap ada untuk serialisasi operasi berikutnya.
Metadata: `${XDG_DATA_HOME:-$HOME/.local/share}/ttc/install.toml`.

Jika proses terputus dan meninggalkan `install.pending`, operasi berikutnya
menolak state ambigu. Bandingkan binary, metadata, checksum, dan file staging/
backup secara manual sebelum recovery; jangan menghapus marker tanpa memeriksa
state. Finalizer lokal tidak mengakses jaringan.

Edit dan rollback `.bashrc` mempertahankan file aktual yang tergeser melalui
pertukaran atomik. Jika save pengguna beradu dengan commit, kedua versi tetap
tersimpan dan pesan error menunjukkan lokasi recovery. Marker tetap ada sampai
state diperiksa secara manual. Jika pertukaran atomik tidak tersedia, config
existing dipertahankan dan installer memberi instruksi PATH manual.

## Penggunaan standalone

```sh
ttc cargo test
ttc 'npm run test && go test ./...'
ttc raw ID --stdout
ttc raw ID --stderr --tail 50
ttc --help
```

Output structured, watch/dev, interactive/TTY, atau format yang tidak dikenal
diteruskan raw. Detail batas raw capture dan config opsional ada dalam SPEC.
Help menampilkan command yang sudah tersedia pada tahap ini.

## Pengembangan

Rust dipin melalui `rust-toolchain.toml`; dependency dikunci oleh `Cargo.lock`.
Gate baseline dan distribusi:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --locked --release --target x86_64-unknown-linux-gnu
./scripts/test-install.sh
./scripts/test-release-artifact.sh
python3 scripts/test-release-policy.py
python3 scripts/check-spec-coverage.py
```

CI PR ke `master`, push `master`, dan stable tag menjalankan gate yang sama,
termasuk seluruh pinned ecosystem smoke. Tag harus cocok dengan versi Cargo dan
commit-nya berada dalam `master`. Workflow M9 mengunggah kandidat setelah semua
gate lulus; publikasi GitHub Release ditambahkan pada M10. Pengguna mengotorisasi
tag/release dan melakukan merge PR.

Lihat [`fixture instructions`](tests/fixtures/README.md) sebelum menambah test.

## Lisensi

MIT. Lihat [`LICENSE`](LICENSE).
