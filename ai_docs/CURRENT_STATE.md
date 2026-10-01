# Current State

**Diperbarui:** 2026-10-01 WIB

## Sedang dikerjakan

M9 — production binary dan distribution gate pada
`feat/m9-production-distribution`, dibuat dari `master` dengan perbaikan
prasyarat CI yang sudah teruji. Installer/latest finalizer, metadata ownership,
transaksi atomik dengan lock/rollback/pending marker, PATH backup/fallback,
global uninstall, audit SPEC dan workflow kandidat GNU telah diimplementasikan.
Seluruh gate lokal M9 lulus; PR CI sedang diselesaikan. M9 belum ditandai lengkap.

## Prasyarat yang sudah terbukti

M7/M8 telah merged melalui PR #6/#7; master `f55d434`. Perbaikan prasyarat dalam
[PR #8](https://github.com/Skirja/ttc-ai/pull/8) lulus seluruh delapan job pada
[run 36817828230](https://github.com/Skirja/ttc-ai/actions/runs/36817828230),
head `b52fef6a7cdce4b88ed60e7b1f3f7cd5e68683b8`. Billing sudah memungkinkan
runner. Full pinned smoke M7/M8 lulus; artifact diunduh dan seluruh checksum
M7 (4 entry) serta M8 (91 entry) diperiksa. Checksum dan reduction detail ada
pada TODO M9. PR #8 masih menunggu merge pengguna.

## Verifikasi lokal M9

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

## Batasan yang diketahui

Parallel fixture test lokal pernah menemui `Text file busy`; log tersimpan
pada `target/m7-m8-ci-evidence/`. Suite serial lokal dan standard parallel Ubuntu
CI lulus pada prasyarat. Full real-tool smoke M7/M8 dibuktikan CI, bukan host
lokal yang tidak menyediakan seluruh build prerequisite.

## Next action

Selesaikan seluruh verifikasi M9, commit, push branch, buka PR ke master dan
periksa seluruh required jobs serta downloaded candidate checksum. Setelah
pengguna merge, periksa clean successful master artifact; baru lengkapi M9 dan
mulai M10 pada tugas berikutnya.

## Arsip terakhir

`ai_docs/steps_done/07-m7-php-jvm-dotnet-local-implementation.md`
