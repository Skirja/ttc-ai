# TTC

TTC adalah wrapper command lokal untuk mengurangi output test/build yang masuk
ke konteks coding agent. Command asli dijalankan sekali; diagnostic, warning,
failure, exit status, dan signal dipertahankan. Output yang tidak dikenali tetap
raw. Kompaksi menyediakan ID untuk membaca kembali output asli.

## Status

TTC v0.1.0 sudah dipublikasikan sebagai GitHub Release. Rilis ini menyediakan
binary Linux dan integrasi Codex melalui hook Bash. Platform rilis saat ini
hanya Linux x86_64 dengan GNU runtime; Windows native dan macOS belum didukung.

Kontrak ada pada [`SPEC`](ai_docs/SPEC.md), urutan/evidence pada
[`TODO`](ai_docs/TODO.md), dan audit standalone pada
[`coverage M9`](ai_docs/M9_COVERAGE.md).

## Instalasi

### Quick install

Pasang stable release terbaru:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://github.com/Skirja/ttc-ai/releases/latest/download/install.sh | sh
```

Installer membutuhkan Linux x86_64 dengan GNU runtime, Bash, `curl`,
`sha256sum`, `awk`, dan utilitas POSIX. Installer memilih stable latest,
mengunci executable/checksum pada tag yang sama, memverifikasi checksum serta
versi, lalu memasang `~/.local/bin/ttc`. Tidak ada version selector. Periksa
instalasi dengan:

```sh
~/.local/bin/ttc --version
```

Jika bin belum di PATH, installer membackup `.bashrc` dan menambahkan satu blok
terkelola. Ikuti instruksi `source ~/.bashrc` atau buka shell baru. Bila config
tidak dapat diedit, binary tetap terpasang; gunakan:

```sh
export PATH="$HOME/.local/bin${PATH:+:$PATH}"
```

### Instalasi manual dengan checksum

Unduh executable `ttc-x86_64-unknown-linux-gnu` dan `SHA256SUMS` dari **tag
release yang sama** melalui HTTPS. Untuk v0.1.0, periksa checksum sebelum
menjalankan:

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

## Integrasi Codex

Instalasi binary tidak mengubah konfigurasi Codex. Untuk memasang hook Bash:

```sh
ttc install codex
```

Lalu buka Codex, jalankan `/hooks`, dan review/trust hook TTC. Hook mulai bekerja
setelah dipercaya. Ketika Codex menjalankan command Bash, hook otomatis
membungkus command dengan binary TTC. Command yang dijalankan langsung di
terminal biasa tidak otomatis melalui hook; untuk menjalankannya melalui TTC,
gunakan bentuk standalone seperti `ttc 'npm test'`.

Hapus integrasi Codex dengan:

```sh
ttc uninstall codex
```

Uninstall binary menolak bila masih ada integrasi harness aktif. Hapus integrasi
Codex terlebih dahulu, lalu jalankan `ttc uninstall` bila ingin menghapus TTC.

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

Binary dan metadata juga memeriksa entry aktual setelah commit atomik.
Archive transaksi disimpan dalam direktori privat `.ttc-distribution` di
direktori bin/data, dengan receipt ownership. Direktori existing tanpa receipt
valid ditolak. Archive dipertahankan setelah uninstall untuk recovery, dan
direktori administrasi TTC ini tidak dihitung sebagai program lain di PATH.

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
Help menampilkan command publik yang tersedia pada rilis ini.

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

CI untuk pull request ke `master` dan push `master` menjalankan gate yang sama,
termasuk seluruh pinned ecosystem smoke. Stable tag harus cocok dengan versi
Cargo dan commit-nya berada dalam `master`; workflow rilis menjalankan ulang
gate dari checkout bersih sebelum menerbitkan asset GitHub Release.

Lihat [`fixture instructions`](tests/fixtures/README.md) sebelum menambah test.

## Lisensi

MIT. Lihat [`LICENSE`](LICENSE).
