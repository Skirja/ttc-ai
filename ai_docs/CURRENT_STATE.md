# Current State

**Diperbarui:** 2026-10-02 WIB

## Sedang dikerjakan

M9 — production binary dan distribution gate pada
`feat/m9-production-distribution`, dibuat dari `master` dengan perbaikan
prasyarat CI yang sudah teruji. Installer/latest finalizer, metadata ownership,
transaksi atomik dengan lock/rollback/pending marker, PATH backup/fallback,
global uninstall, audit SPEC dan workflow kandidat GNU telah diimplementasikan.
Seluruh gate lokal fix ownership binary/metadata terbaru lulus (226 Rust test,
17 installer E2E dan standalone smoke). CI fix config sebelumnya sudah lulus;
fix ownership terbaru masih menunggu PR CI. Pengguna mengotorisasi squash merge
PR #8/#9 ke branch utama repo (`master`) pada 2026-10-02. M9 belum lengkap
karena clean master run/artifact masih harus diperiksa setelah merge.

## Prasyarat yang sudah terbukti

M7/M8 telah merged melalui PR #6/#7; master `f55d434`. Perbaikan prasyarat dalam
[PR #8](https://github.com/Skirja/ttc-ai/pull/8) lulus seluruh delapan job pada
[run 36817828230](https://github.com/Skirja/ttc-ai/actions/runs/36817828230),
head `b52fef6a7cdce4b88ed60e7b1f3f7cd5e68683b8`. Billing sudah memungkinkan
runner. Full pinned smoke M7/M8 lulus; artifact diunduh dan seluruh checksum
M7 (4 entry) serta M8 (91 entry) diperiksa. Checksum dan reduction detail ada
pada TODO M9. PR #8 masih menunggu merge pengguna.

## Verifikasi M9

- 11 unit test distribusi lulus, termasuk actual SIGKILL setelah replacement,
  penolakan marker berikutnya, kegagalan rename/commit metadata, rollback,
  foreign binary/config dan integrasi aktif.
- Smoke production ELF di luar repository lulus untuk argv/shell byte-exact
  stdout/stderr, env/cwd/stdin, exit13/7, SIGINT/SIGTERM, invocation count1,
  TTY, raw replay, diagnostic dan representative reduction >=80%.
- Installer E2E memakai HTTP loopback opt-in dan temporary HOME/XDG. Gate
  terakhir mencakup concurrency installer/uninstaller serta default XDG; 17 E2E lulus.
- Format/Clippy/full Rust test paralel/release GNU --locked lulus. Log lengkap
  `target/m9-evidence/local-gates-final.log`; checksum/reduction pada TODO.
- Gate tag menguji stable SemVer, mismatch dan containment master dalam repo
  temporary. Audit coverage mengunci digest SPEC, semua section/clauses,
  source/test references dan CI jobs; tidak mengklaim bukti semantik dari
  keberadaan mapping saja.

- [PR #9](https://github.com/Skirja/ttc-ai/pull/9), production commit `bea54a5`,
  lulus 10 job pada [run 36824224429](https://github.com/Skirja/ttc-ai/actions/runs/36824224429).
  Artifact GNU SHA-256 `f1b6dd184ab31b0331055fe76e3a4370e4fa07f30d3b10199312a6d507700301`;
  checksum seluruh 112 evidence entries dan standalone/installer smoke ulang
  memakai downloaded binary lulus. Exact head/merge SHA dan seluruh checksum
  dicatat pada TODO. PR #9 membawa seluruh perbaikan PR #8.

## Keputusan yang dikunci

- Satu Rust binary crate; child asli sekali, output default retain.
- Static manifest bounded; cycle/overflow/ambiguous/unknown/structured output
  raw. Runner asli menentukan selection, dependency order, cache, concurrency,
  cwd/stdin/environment.
- Distribusi terpisah dari core; `sha2 = =0.10.9` untuk checksum ownership
  native tanpa menjalankan binary existing. Finalizer tidak mengakses jaringan.
- Installer production latest-only; binary manual tidak diadopsi. Lock file
  tetap ada setelah uninstall. State transaksi ambigu menolak mutasi berikutnya.
- Squash merge PR #8/#9 diotorisasi pengguna pada 2026-10-02. Tidak ada
  otorisasi tag/release atau implementasi M10 pada tugas ini.
- M10 menunggu clean successful master run M9 dan downloaded GNU artifact.
  Belum ada implementasi/instruksi integrasi Codex yang tersedia pada help M9.

## Regresi gate yang diperbaiki

Run evidence `36825391722` pada `52dd9be` menemukan race pidfile M2: file sudah
ada tetapi isinya masih kosong. Test sekarang menunggu PID valid; regresi empty
file dan 50 pengulangan signal suite lulus lokal. Perbaikan hanya readiness test,
execution/filter core tetap sama. CI terakhir harus hijau sebelum merge.

Review kode M9 kemudian menemukan `.bashrc` berubah selama backup/staging bisa
ditimpa rename dari snapshot lama. `src/distribution/path.rs` sekarang
memvalidasi identity dan byte config lagi setelah staging; konflik memakai
fallback PATH manual tanpa mengganti config. Test baru meliputi save in-place
dan atomic rename. Semua gate lokal M9 lulus pada perubahan ini:
`target/m9-evidence/path-race-fix.log`; binary lokal SHA-256
`7b22354748f1cebbe5707c0e32cae25f0c788029478f29e65e3740c4b287df98`.
Fix commit `6a5627c` lulus 10 job pada
[run 36955336106](https://github.com/Skirja/ttc-ai/actions/runs/36955336106).
Artifact final diunduh; 112 evidence entries dan kedua checksum manifest cocok.
Binary SHA-256
`2ceac28857697143a5f67bf96950be2252238daa56228f7517846fc2d635ef63`.
Standalone smoke dan 17 installer E2E lulus lagi pada binary unduhan. Checksum
lain dan detail replay dicatat pada TODO M9.

Review berikutnya menemukan save setelah validasi terakhir masih bisa
tertimpa. Fix terbaru memakai atomic exchange dan memeriksa file aktual yang
tergeser, yang tetap disimpan sebagai backup. Konflik/sync gagal menyisakan
kedua versi dan marker recovery. Rollback config memakai exchange/capture;
uninstall ambigu mempertahankan tombstone binary/metadata dan marker.
Feature fs dari nix terpin menyediakan renameat2 tanpa unsafe code baru.
20 unit distribusi, semua 214 test Rust, baseline dan seluruh gate M9 lulus:
`target/m9-evidence/path-exchange-fix.log`. Binary lokal GNU SHA-256
`fe665b880f521e2976447a6a0f6c42aa76465c40c12eed2cef7ac22cff7493cd`.
Fix commit `8277e31b51fc4a29f5cbebca1b03822aa7c5eb82` lulus 10 job pada
[run 36959752141](https://github.com/Skirja/ttc-ai/actions/runs/36959752141).
Artifact checkout merge `e75efe9393c4a71ebfef8c712c0ea126fdfb7c14` diunduh;
kedua manifest checksum dan 112 evidence entries cocok. Binary SHA-256
`773a36057a958699598bb9613667c60fef79d49166c58eded57e4daf06709b48`.
Standalone smoke dan 17 installer E2E lulus pada binary unduhan; log
`target/m9-evidence/artifact-36959752141.log`. Detail checksum/command pada TODO.

Review penuh PR #9 kemudian menemukan race yang sama pada binary. Fix terbaru
memakai transaksi binary/metadata dengan archive aktual, validasi identity/hash
setelah commit dan rollback, capture uninstall, serta cleanup marker atomik.
Archive privat `.ttc-distribution` memakai receipt ownership dan tetap tersedia
setelah uninstall, termasuk untuk write lewat FD lama. Namespace existing yang
tidak terbukti dimiliki TTC ditolak. 226 test Rust dan semua gate lokal M9
lulus; log `target/m9-evidence/ownership-race-fix-final.log` dan
`target/m9-evidence/ownership-race-regressions-final.log`.

## Batasan yang diketahui

Parallel fixture test lokal pernah menemui `Text file busy`; log tersimpan
pada `target/m7-m8-ci-evidence/`. Suite serial lokal dan standard parallel Ubuntu
CI lulus pada prasyarat. Full real-tool smoke M7/M8 dibuktikan CI, bukan host
lokal yang tidak menyediakan seluruh build prerequisite.

## Next action

PR #8 sudah squash merged menjadi `2ad265f970f758b603dc9b706afe0b9b6351d559`
pada 2026-10-02; base PR #9 disinkronkan tanpa perubahan kode produksi.
Periksa CI fix ownership PR #9, lalu squash merge #9 sesuai otorisasi pengguna.
Periksa clean successful master run dan downloaded GNU artifact; catat exact
commit/run/checksum dan lengkapi M9. M10 tetap menunggu gate master tersebut.

## Arsip terakhir

`ai_docs/steps_done/07-m7-php-jvm-dotnet-local-implementation.md`
