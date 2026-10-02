# Current State

**Diperbarui:** 2026-10-02 WIB

## Sedang dikerjakan

M9 — production binary dan distribution gate pada
`feat/m9-production-distribution`, dibuat dari `master` dengan perbaikan
prasyarat CI yang sudah teruji. Installer/latest finalizer, metadata ownership,
transaksi atomik dengan lock/rollback/pending marker, PATH backup/fallback,
global uninstall, audit SPEC dan workflow kandidat GNU telah diimplementasikan.
Seluruh gate lokal perbaikan terakhir lulus. CI kandidat sebelumnya lulus
10 job dan artifact terverifikasi; perbaikan TOCTOU commit config terbaru
menunggu push/CI dan downloaded artifact. PR #9 tetap terbuka; M9 belum lengkap
karena master artifact gate belum terpenuhi.

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
- Pengguna merge PR; agent tidak merge, membuat tag atau publish pada M9.
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
CI/artifact untuk fix terbaru belum diperiksa.

## Batasan yang diketahui

Parallel fixture test lokal pernah menemui `Text file busy`; log tersimpan
pada `target/m7-m8-ci-evidence/`. Suite serial lokal dan standard parallel Ubuntu
CI lulus pada prasyarat. Full real-tool smoke M7/M8 dibuktikan CI, bukan host
lokal yang tidak menyediakan seluruh build prerequisite.

## Next action

Push fix atomic exchange ke PR #9, periksa seluruh CI dan downloaded artifact,
lalu pengguna merge PR #9.
Periksa clean successful master run dan downloaded GNU artifact; catat exact
commit/run/checksum dan lengkapi M9. M10 tetap menunggu gate master tersebut.

## Arsip terakhir

`ai_docs/steps_done/07-m7-php-jvm-dotnet-local-implementation.md`
