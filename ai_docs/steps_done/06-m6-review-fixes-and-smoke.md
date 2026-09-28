# Sesi 06 — M6 review fixes dan smoke

**Tanggal:** 2026-09-28 WIB
**Status:** partial

## Yang dikerjakan

- Meninjau seluruh delta branch `feat/m6-package-monorepo` terhadap `master`,
  bukan hanya perubahan yang belum di-commit.
- Memperbaiki lima temuan: parsing prefix warning, tumpang tindih Nx target
  dengan package script, pemilihan project pada `npm --prefix`, deteksi
  `npm run install`, dan nested shell background job.
- Menstabilkan smoke Yarn pada `CI=true`, lalu menormalkan ANSI SGR sebelum
  membandingkan durasi record agar output CI berwarna tidak memberi mismatch.
- Memperbarui bukti M6 dan state sesi; membuka PR #5 tetap untuk merge pengguna.

## Perubahan

- Diubah: `src/core/classification.rs`, `src/core/manifests.rs`,
  `src/core/filters/mod.rs`, `scripts/m6-smoke.sh`.
- Ditambah regresi: `tests/monorepo.rs`, `tests/mixed_monorepo.rs`,
  `tests/package_scripts.rs`, `tests/shell_contract.rs`.
- Diubah dokumentasi: `ai_docs/TODO.md`, `ai_docs/CURRENT_STATE.md`.
- Ditambah: arsip sesi ini.
- Commit implementasi: `bad9830`; commit smoke Yarn: `8e11a87`; commit
  normalisasi ANSI smoke: `332d8cd`; commit TODO/state sebelumnya:
  `e3300bd`.

## Keputusan

- Target Nx disimpan terpisah dari `package.json` scripts; setiap manager
  memilih definisi yang sesuai command yang dijalankan.
- `--prefix` npm menentukan direktori manifest efektif. Operasi install
  ditentukan dari posisi subcommand. Prefix output task hanya dikupas untuk
  task yang dikenal.
- Ampersand tunggal pada shell command diperlakukan konservatif sebagai
  kemungkinan background syntax, termasuk jika berada di string nested shell.
- Smoke M6 menormalisasi escape ANSI dan durasi sebelum membandingkan record;
  runner tetap dijalankan langsung untuk baseline dan TTC.
- Keputusan sebelumnya tetap berlaku: binary satu crate Rust, command asli
  dijalankan sekali, output tidak pasti raw, discovery manifest statis dan
  bounded, runner asli memegang selection/cwd/order/cache/concurrency, dan
  pengguna melakukan merge PR.

## Verifikasi

- `cargo test --test package_scripts --test monorepo --test mixed_monorepo --test shell_contract --test manifests` — lulus.
- `cargo test --test manifests` — lulus.
- `cargo test --test package_scripts` — lulus.
- `cargo test --test monorepo` — lulus.
- `cargo test --test mixed_monorepo` — lulus.
- `cargo test --test classifier` — lulus.
- `cargo test --test filter_safety` — lulus.
- `cargo test --test javascript_e2e` — lulus.
- `cargo fmt --all -- --check` — lulus.
- `cargo clippy --all-targets --all-features -- -D warnings` — lulus.
- `cargo test --all-targets --all-features` — lulus.
- `cargo build --release` — lulus.
- `CI=true FORCE_COLOR=1 sh scripts/m6-smoke.sh target/release/ttc target/m6-evidence/report.txt` — lulus untuk seluruh group pada commit `332d8cd`; report bersih SHA-256 `045244863845e3ba459a2a24f9525948e9ddf8a1b12d3f4be4d54c88909f52e3`.
- CI run `36364865946` pada head `8e11a87` lulus seluruh enam job. CI run terbaru yang terlihat, `36366060564`, melaporkan baseline Rust dan M2–M4/M6 lulus; M5 masih `in_progress` ketika sesi dihentikan.
- Artifact `m6-monorepo-evidence` dari run `36366060564` diunduh. `sha256sum -c` berhasil untuk `fixtures.txt` dan `report.txt` setelah nama path manifest checksum disesuaikan ke direktori unduh. Hash artifact dicatat di `ai_docs/TODO.md`.
- Dites manual: ya — pinned smoke membandingkan baseline dan TTC; seluruh 19 kasus di report lokal selesai dengan exit 0.

## Review

Direview: delta branch `master...4500e70` → **NEEDS CHANGES · 5 blocking**.
Setelah review: kelima temuan diperbaiki pada `bad9830`; smoke CI diperbaiki
lanjut pada `8e11a87` dan `332d8cd`. Final diff setelah perbaikan belum menjalani
review formal ulang.

## Batasan saat ini

- PR #5 masih terbuka dan belum di-merge.
- Status M5 pada run terbaru tidak diketahui setelah interupsi; status terakhir
  yang diamati adalah `in_progress`.
- Review formal ulang untuk final diff belum dilakukan.
- Artifact smoke lokal dan CI berada di `target/`, bukan di-commit.

## Next action

Periksa hasil akhir M5 pada CI run `36366060564`; jika seluruh job lulus, lakukan
review ulang terhadap delta terbaru dan perbarui status M6. Pengguna yang merge
PR #5.
