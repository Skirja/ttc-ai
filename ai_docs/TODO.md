# TTC Implementation TODO

Dokumen ini adalah source of truth untuk urutan implementasi dan status
milestone. Kontrak produk dan perilaku normatif tetap berada di
[`SPEC.md`](./SPEC.md). Jika implementasi membutuhkan perubahan kontrak, ubah
SPEC lebih dulu, lalu sinkronkan TODO dalam commit yang sama.

## Aturan milestone

- Semua checkbox dimulai kosong. Centang hanya berdasarkan bukti yang benar-benar
  dijalankan.
- Milestone selesai hanya jika implementation checklist, acceptance criteria,
  verification commands, dan evidence artifact seluruhnya lengkap.
- Command asli tidak boleh pernah dijalankan ulang untuk recovery atau parsing.
- Perubahan filter wajib fail-open: ketidakpastian mempertahankan output, bukan
  membuangnya.
- Fixture parser dan real-tool smoke test adalah dua bukti berbeda; keduanya
  wajib untuk setiap ecosystem family.
- Versi real tool dipin pada CI. Update versi dilakukan sengaja bersama fixture
  dan review perubahan output.
- M10 tidak boleh dimulai sebelum artifact CI M9 lulus.
- Public release tidak boleh dibuat sebelum E2E Codex M10 lulus.

## Dependency graph

```text
M1 → M2 → M3 → M4 → M5 → M6 ┐
                  ├────→ M7 ├→ M9 → M10
                  └────→ M8 ┘
```

M7 dan M8 boleh berjalan paralel setelah M4. M6 menunggu M5 karena mixed
monorepo membutuhkan filter Rust, Python, dan Go. M9 menunggu seluruh M1–M8.

---

## M1 — Foundation dan kontrak executable

**Outcome:** repository menghasilkan satu Rust binary crate yang sehat,
versioned, dan siap menerima vertical slices tanpa mengunci struktur yang tidak
perlu.

**Dependencies:** tidak ada.

### Implementation checklist

- [x] Inisialisasi satu binary crate bernama `ttc`; jangan membuat workspace
  multi-crate.
- [x] Tetapkan package version `0.1.0` dan pastikan hanya ada satu sumber versi
  untuk package serta `ttc --version`.
- [x] Pin Rust toolchain yang dipakai CI agar build reproducible.
- [x] Susun modul berdasarkan tanggung jawab: CLI, invocation/execution,
  classification, filter families, raw storage, dan harness adapter.
- [x] Jaga dependency satu arah: harness memakai core, core tidak mengetahui
  Codex atau harness lain.
- [x] Implementasikan public `--help` dan `--version`; jangan menambahkan
  `explain`, `doctor`, atau placeholder command.
- [x] Tambahkan `LICENSE` MIT dan README minimum yang menunjuk ke SPEC serta
  status greenfield.
- [x] Tambahkan baseline GitHub Actions untuk format, Clippy, test, dan debug
  build pada branch `master`/pull request.
- [x] Tetapkan convention fixture, integration test, dan temporary test data
  agar test tidak menulis state pengguna.

### Acceptance criteria

- [x] Debug dan release build berhasil pada Linux x86_64.
- [x] `ttc --version` tepat menghasilkan versi `0.1.0` dalam format CLI yang
  stabil.
- [x] Help hanya menampilkan interface publik yang sudah diimplementasikan.
- [x] Tidak ada dependency pada source/arsitektur TTC lama.
- [x] Tidak ada daemon, database, telemetry, atau network client di binary.

### Verification commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build
cargo build --release
./target/release/ttc --version
./target/release/ttc --help
```

### Evidence

- [x] Commit implementasi dicatat.
- [x] Output verification commands dicatat pada commit/CI run.
- [x] Artifact: `target/release/ttc` untuk smoke test lokal.

Bukti lokal 2026-09-15 pada branch `feat/m1-foundation`:

- Seluruh verification command M1 selesai dengan exit status 0.
- `cargo test --all-targets --all-features` menjalankan 3 integration test dan
  seluruhnya lulus.
- `target/release/ttc` terverifikasi sebagai executable ELF x86-64 GNU/Linux;
  `--version` menghasilkan `ttc 0.1.0` dan help hanya memuat opsi yang tersedia.
- M1 ditutup pada sesi ini berdasarkan seluruh verifikasi lokal yang lulus;
  perubahan tetap berada di worktree/staging dan CI GitHub belum dijalankan.

---

## M2 — Process wrapper dan exact passthrough

**Outcome:** TTC dapat membungkus command secara transparan dan menjalankannya
tepat satu kali sebelum filtering apa pun diperkenalkan.

**Dependencies:** M1.

### Implementation checklist

- [x] Implementasikan direct argv: dua atau lebih argument setelah `ttc`
  diteruskan sebagai program dan argv tanpa shell tambahan.
- [x] Implementasikan single-string invocation: tepat satu argument command
  dijalankan satu kali melalui shell POSIX native.
- [x] Wariskan cwd, environment, dan stdin tanpa modifikasi.
- [x] Baca stdout dan stderr secara concurrent dan pertahankan urutan internal
  masing-masing stream.
- [x] Teruskan exit code child tanpa normalisasi.
- [x] Propagasikan SIGINT dan SIGTERM; bila child mati karena signal, TTC harus
  memiliki hasil signal yang setara.
- [x] Buat helper test yang mencatat invocation count tanpa bergantung pada tool
  ecosystem.
- [x] Preclassify command raw/watch/interactive yang diketahui agar stdio dan
  TTY behavior tidak diubah.
- [x] Pastikan error internal setelah spawn tidak pernah menyebabkan rerun.

### Acceptance criteria

- [x] Success, exit 7, SIGINT, dan SIGTERM sama dengan baseline langsung.
- [x] Command direct argv dan shell string masing-masing berjalan tepat sekali.
- [x] Quote, newline, Unicode, environment assignment, pipe, redirect, `&&`,
  dan command substitution tiba utuh di shell.
- [x] Unknown stdout dan stderr byte-exact per stream.
- [x] Raw watch/dev command tetap streaming dan menerima stdin/signal/TTY.

### Verification commands

```bash
cargo test --test execution
cargo test --test passthrough
cargo test --test signals
cargo test --test shell_contract
```

### Evidence

- [x] Commit implementasi dicatat.
- [x] Baseline-versus-TTC byte comparison disimpan sebagai test artifact.
- [x] Invocation-count dan signal test lulus pada Linux.

Bukti lokal M2 pada Linux x86_64:

- Empat command verification M2, `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test --all-targets --all-features`, dan `cargo build --release` lulus.
- `sh scripts/m2-evidence.sh target/m2-evidence/report.txt` membandingkan output
  baseline dan TTC: stdout 120.000 byte serta stderr 140.000 byte identik.
- `tests/execution.rs` dan `tests/shell_contract.rs` membuktikan invocation
  count satu kali, termasuk saat output forwarding gagal. `tests/signals.rs`
  membuktikan SIGINT dan SIGTERM pada Linux.
- Commit implementasi: `57f99546f6034fc8782ab65b7522c8daa0126f97`.
  [PR #1](https://github.com/Skirja/ttc-ai/pull/1) menjalankan
  [CI run 35967544849](https://github.com/Skirja/ttc-ai/actions/runs/35967544849)
  dan kedua job lulus pada commit tersebut.
- Artifact CI `m2-byte-comparison` pada run yang sama menyimpan laporan `cmp`
  baseline-versus-TTC untuk stdout dan stderr.
- Perbaikan background shell pada commit
  `acfca052d91f51a1133386e36aa00fe4cda8f736` diuji dengan shell string,
  `bash -c`, dan pembungkus `env` pada stdio non-TTY. Kedua job pada
  [CI run 35968948114](https://github.com/Skirja/ttc-ai/actions/runs/35968948114)
  lulus dan artifact perbandingan byte tetap diunggah.
- [CI run 35969037570](https://github.com/Skirja/ttc-ai/actions/runs/35969037570)
  pada commit dokumentasi `0c64d90441bc93b7774719cdadad64588fdc1d3a`
  juga lulus kedua job dan menyediakan artifact yang sama. M2 selesai pada
  [PR #1](https://github.com/Skirja/ttc-ai/pull/1) dan sudah di-merge pengguna
  ke `master` sebagai commit `e2c97b5ec43ada7256ff9acd4c0af2c84c1f9bf2`.

---

## M3 — Streaming safety, configuration, dan raw retrieval

**Outcome:** pipeline tetap bounded, dapat memulihkan semua byte yang
dikompaksi, dan aman ketika input atau storage tidak dapat diproses.

**Dependencies:** M2.

### Implementation checklist

- [x] Gunakan bounded channel antara reader stdout/stderr dan output processor.
- [x] Implementasikan framing per stream dengan pending line maksimum 1 MiB.
- [x] Long line, non-UTF-8, dan framing/parser failure beralih ke raw tanpa
  kehilangan byte.
- [x] Implementasikan config optional `~/.config/ttc/config.toml` dengan default
  `max_raw_mb = 32` dan `retention_hours = 24`.
- [x] Invalid config tidak boleh menjalankan ulang command atau menyebabkan
  output yang tidak pasti dibuang.
- [x] Implementasikan capture file biasa dengan random run ID, event ber-tag
  stdout/stderr, dan permission user-only.
- [x] Capture hanya dibuat/ditahan bila minimal satu byte dikompaksi; failure
  tanpa kompaksi tetap exact passthrough tanpa TTC metadata.
- [x] Saat ada kompaksi, capture original stdout/stderr sebelum filtering agar
  replay dapat mengembalikan output asli, subject to batas storage.
- [x] Batasi capture total 32 MiB menggunakan tail terbaru dan metadata jumlah
  byte yang dibuang.
- [x] Gunakan XDG state sebagai lokasi utama dan temporary directory per-user
  permission 0700 sebagai fallback sandbox.
- [x] Jika seluruh storage gagal, hentikan kompaksi berikutnya dan emit raw tanpa
  rerun.
- [x] Hapus capture lebih tua dari retention saat invocation berikutnya.
- [x] Implementasikan `ttc raw ID`, `--stdout`, `--stderr`, dan `--tail N`.
- [x] Replay default mengikuti urutan event yang diamati; selector mengekstrak
  byte stream terkait.
- [x] Tulis summary kompaksi dan raw hint hanya ke stderr.

### Acceptance criteria

- [x] Memory tidak tumbuh sebanding dengan ukuran output besar.
- [x] Capture >32 MiB menyimpan tail terbaru dan dropped-byte count benar.
- [x] Default replay, stream selector, dan tail menghasilkan byte yang benar.
- [x] ID ditemukan pada XDG maupun temporary fallback.
- [x] Permission directory/file hanya untuk user pemilik.
- [x] Cleanup retention tidak menghapus capture yang masih valid.
- [x] XDG denial memakai fallback; kegagalan kedua storage menjadi raw tanpa
  rerun.
- [x] Invocation tanpa kompaksi tidak menghasilkan file, summary, atau hint.

### Verification commands

```bash
cargo test --test streaming
cargo test --test raw_store
cargo test --test raw_cli
cargo test --test storage_failure
cargo test --test config
```

### Evidence

- [x] Commit implementasi final dan CI PR dicatat.
- [x] Peak-memory result untuk large stream dicatat.
- [x] Permission, truncation, fallback, dan cleanup test artifacts dicatat.

Bukti lokal M3 pada Linux x86_64 (branch `feat/m3-streaming-raw`):

- Lima command verification M3, `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test --all-targets --all-features`, dan `cargo build --release` lulus.
- `sh scripts/m3-evidence.sh target/m3-evidence/report.txt` setelah perbaikan
  review mencatat peak RSS 2.900 KiB untuk output 64 MiB dan 2.960 KiB untuk
  output 256 MiB; selisih 60 KiB. Tidak ada capture file pada invocation
  tanpa kompaksi.
- `tests/raw_store.rs` membuktikan file capture maksimum 32 MiB, suffix byte
  asli, dropped-byte count, mode file `0600`, dan direktori `0700`.
  `tests/storage_failure.rs` membuktikan fallback, lookup ID, cleanup, dan
  raw tanpa rerun ketika kedua lokasi gagal. `tests/raw_cli.rs` membuktikan
  replay berurutan, selector, dan byte tail.
- `sh scripts/m2-evidence.sh target/m2-evidence/report.txt` tetap menghasilkan
  perbandingan stdout dan stderr byte-identical.
- Perbaikan review menambah tes `failed_append_keeps_last_committed_capture`,
  `rename_failure_keeps_partial_file_replayable`, lookup fallback pada XDG
  `EACCES`, dan root fallback `/tmp` yang stabil. Lima verification command M3,
  baseline repository gate, serta skrip bukti M2/M3 lulus lagi secara lokal.
- Commit implementasi `1c793b79fa9d5a6dcc15ca3d5f56b7570ba1f558`
  diuji pada [PR #2](https://github.com/Skirja/ttc-ai/pull/2). Ketiga job
  pada [CI run 35974053659](https://github.com/Skirja/ttc-ai/actions/runs/35974053659)
  lulus untuk commit tersebut. Artifact `m3-streaming-evidence` menyimpan
  laporan peak RSS; artifact `m2-byte-comparison` menyimpan regresi passthrough.
- Perbaikan review pada commit `d5c00ca58f4b6e4f3644ffbc782c4372d6fed035`
  diuji oleh [CI run 35976764504](https://github.com/Skirja/ttc-ai/actions/runs/35976764504)
  di PR #2. Ketiga job lulus dan artifact `m3-streaming-evidence` serta
  `m2-byte-comparison` tersedia untuk commit tersebut.

---

## M4 — Filter engine dan JavaScript/TypeScript vertical slice

**Outcome:** satu vertical slice filtering lengkap membuktikan classifier,
safe-retain policy, capture, summary, dan output reduction end-to-end.

**Dependencies:** M3.

### Implementation checklist

- [x] Definisikan filter interface yang menerima original command, manifest
  hints, stream, dan record tanpa memiliki process execution.
- [x] Default seluruh record adalah retain; hanya recognizer ber-confidence
  cukup yang boleh compact.
- [x] Retain warning, error, failure, panic, fatal, assertion/diff, deprecation,
  vulnerability/security, stack trace, path+line/column, dan final summary.
- [x] Kenali ANSI untuk klasifikasi tetapi emit byte asli bagi record retained.
- [x] Implementasikan command classifier dan delayed output-signature confidence;
  record sebelum confidence tetap raw.
- [x] Implementasikan machine-readable flags dan always-raw command policy dari
  SPEC.
- [x] Implementasikan npm, pnpm, yarn, bun, npx/pnpx/bunx, serta nested tool
  dispatch yang tidak menjalankan command tambahan.
- [x] Implementasikan seluruh JS/TS test runner pada SPEC.
- [x] Implementasikan JS/TS lint, typecheck, build, dan format-check families.
- [x] Filter progress/passing/duplicate diagnostic yang terbukti aman; failed
  tests, diffs, stack traces, warnings, dan summaries tetap lengkap.
- [x] Tambahkan success, failure, warning, unknown-format, dan 1.000+ record
  fixture untuk setiap output family.
- [x] Pin versi tool nyata yang dipakai smoke tests.

### Acceptance criteria

- [x] Setiap retained diagnostic byte-identical dengan fixture input.
- [x] Unknown format dan JSON/JSONL/XML/YAML/SARIF/TAP tetap raw kecuali parser
  lossless khusus tersedia.
- [x] Setiap large fixture mengurangi output minimum 80% secara byte.
- [x] Summary menghitung passing/progress records dengan benar dan hanya muncul
  bila ada kompaksi.
- [x] Raw ID mengembalikan original output hingga batas capture; truncation di
  atas batas selalu dinyatakan lewat metadata.
- [x] Minimal satu E2E nyata per JS/TS family lulus dengan exit status baseline.

### Verification commands

```bash
cargo test --test classifier
cargo test --test filter_safety
cargo test --test javascript_fixtures
cargo test --test javascript_e2e
cargo test --test reduction
```

### Evidence

- [x] Commit implementasi dicatat.
- [x] Tabel fixture/tool version dan hasil smoke test dicatat.
- [x] Laporan byte reduction per large fixture disimpan.

Bukti lokal M4 pada Linux x86_64 (`feat/m4-js-filter`):

- Lima verification command M4, `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test --all-targets --all-features`, dan `cargo build --release` lulus.
- `tests/javascript_fixtures.rs` menguji success, failure, warning, unknown,
  serta 1.001 record pada family test, lint, typecheck, build, format, dan
  install. Byte retained diperiksa terhadap input asli. Duplikat diagnostic
  yang mengandung warning/error/lokasi tetap dipertahankan sesuai SPEC.
- `sh scripts/m4-reduction.sh target/m4-evidence/reduction.txt` mengukur byte
  yang benar-benar diteruskan, termasuk summary dan raw hint:

| Family | Input byte | Model-facing byte | Record dikompaksi |
|---|---:|---:|---:|
| Test | 27.027 | 185 | 998 |
| Lint | 31.031 | 197 | 998 |
| Typecheck | 28.028 | 188 | 998 |
| Build | 18.018 | 158 | 998 |
| Format | 33.033 | 203 | 998 |
| Install | 45.045 | 239 | 998 |

- `sh scripts/m4-smoke.sh target/release/ttc target/m4-evidence/report.txt`
  lulus dengan Node 24.21.0, npm 11.19.0, pnpm 9.15.9, Yarn 1.22.22,
  Bun 1.4.2, Vitest 5.0.1, Jest 30.5.2, ESLint 10.11.0, TypeScript 7.0.2,
  Vite 8.3.1, dan Prettier 3.9.9. Seluruh exit status TTC sama dengan baseline;
  case Vitest gagal tetap exit 1 dan diagnostic assertion terlihat.
  Vitest verbose success: 6.916 byte baseline menjadi 559 byte termasuk
  metadata TTC (reduksi 91,9%).
- Full workspace traversal, alias lintas proyek, fallback signature untuk
  runner tersembunyi, dan mixed-language tetap mengikuti M6. Output tool yang
  tidak cocok recognizer spesifik tetap raw. Selama M4, selector workspace
  npm/pnpm/yarn yang belum di-resolve ke manifest package terpilih juga raw;
  manifest root tidak dipakai sebagai hint untuk package lain.
- Implementasi `35d90f9bb40c6ab146cd0fa5fa2044b5fe6e810c` dan perbaikan state per-stream `e7f67586abc1d4b7c4de03c9cf2ed046bacc4a17`
  diuji pada [PR #3](https://github.com/Skirja/ttc-ai/pull/3). Keempat job
  [CI run 36094451943](https://github.com/Skirja/ttc-ai/actions/runs/36094451943)
  lulus untuk commit `e7f67586abc1d4b7c4de03c9cf2ed046bacc4a17`.
  Artifact `m4-javascript-evidence` memuat `reduction.txt` dan `report.txt`;
  keduanya diunduh dan diperiksa. Di CI, Vitest verbose success berkurang dari
  11.336 byte baseline menjadi 1.113 byte termasuk metadata TTC.
- Run PR awal `36094153776` mengungkap state diagnostic stderr yang menahan
  passing stdout bergantung jadwal reader. Perbaikan `e7f6758` memisahkan
  state confidence dan diagnostic per stream; tes interleaving serta run CI
  berikutnya lulus.
- Tiga temuan review PR #3 ditangani pada commit
  `a0c5b0561912ab7c69274df2e3e5ad8349d4acca`: selector workspace tetap
  raw sampai resolusi M6, flag machine-readable dalam script manifest memaksa
  raw, dan lokasi file tanpa ekstensi tetap retained. Tes classifier, safety,
  dan E2E baru mereproduksi pemicunya. Kelima command M4, gate repository,
  build release, reduction, dan smoke lulus lagi secara lokal. Keempat job
  [CI run 36113907536](https://github.com/Skirja/ttc-ai/actions/runs/36113907536)
  lulus untuk commit tersebut; artifact `m4-javascript-evidence` diunduh dan
  isinya diperiksa. Review ulang formal setelah fix belum dijalankan.

---

## M5 — Rust, Python, dan Go

**Outcome:** seluruh ecosystem core non-JavaScript pada SPEC memiliki filter aman
dan smoke test nyata.

**Dependencies:** M4.

### Implementation checklist

- [x] Implementasikan Cargo test/nextest/build/check/clippy/fmt/doc termasuk
  workspace dan package selection.
- [x] Compact passing tests serta Compiling/Checking progress; retain compiler
  diagnostics, warnings, failure output, dan summary.
- [x] Implementasikan Python/python3, uv, Poetry, dan Pipenv wrapper detection.
- [x] Implementasikan pytest, unittest, tox/nox, Ruff, mypy, pyright, pylint,
  Black, coverage, dan install families pada SPEC.
- [x] Generic Python application tetap raw bila signature tidak dikenali.
- [x] Implementasikan Go test/build/vet/generate, golangci-lint, dan staticcheck.
- [x] Implementasikan lossless-aware parser khusus `go test -json`; JSON generic
  tetap raw.
- [x] Pertahankan byte asli semua event Go JSON selain frame passing yang cocok
  parser; lifecycle tidak pernah dikompaksi, sedangkan key duplikat, field/action
  baru, tipe invalid, dan malformed JSON membuat sisa invocation raw.
- [x] Retain failed test output, panic, race detector, vet/build diagnostic, dan
  package summary.
- [x] Tambahkan success/failure/warning/unknown/large fixtures, raw replay, dan
  baseline direct-versus-TTC untuk ketiga ecosystem.
- [x] Tambahkan project smoke terisolasi untuk Cargo/libtest/nextest, pytest,
  `go test`, dan `go test -json`; pin Rust 1.98.1, cargo-nextest 0.9.108,
  Python 3.14.7, pytest 9.1.1, dan Go 1.27.1.
- [x] Tambahkan CI Linux M5 dengan Actions ber-SHA dan upload report versi,
  retention, reduction, serta real-tool smoke setelah semua gate lulus.

### Acceptance criteria

- [x] Seluruh command Rust, Python, dan Go pada SPEC terpetakan ke recognizer atau
  explicit raw behavior.
- [x] Generic app output tetap byte-exact.
- [x] Diagnostic dan exit/signal sama dengan baseline untuk success dan failure.
- [x] Generic app dan Go JSON retained event byte-exact; seluruh lifecycle Go
  JSON tetap ada dan passing frame cocok saja yang dapat dihapus.
- [x] Large text fixture tiap ecosystem mengurangi byte minimum 80% termasuk
  summary dan raw hint. Rasio tersebut tidak diterapkan ke fixture Go JSON.
- [x] Cargo, pytest, dan Go real-tool smoke E2E lulus pada versi pin.

### Verification commands

```bash
cargo test --test rust_fixtures
cargo test --test python_fixtures
cargo test --test go_fixtures
cargo test --test core_ecosystem_e2e
cargo test --test reduction
cargo test --test classifier
cargo test --test filter_safety
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
sh scripts/m5-smoke.sh target/release/ttc target/m5-evidence/report.txt
```

### Evidence

- [x] Commit implementasi `2c66acf` dicatat.
- [x] Versi pinned tool dan baseline-versus-TTC real-project smoke output
  disimpan sebagai artifact `m5-core-ecosystem-evidence`.
- [x] Laporan fixture, retention, raw replay, dan reduction per ecosystem
  disimpan pada artifact lokal `target/m5-evidence/` dan dikonfigurasi untuk
  diunggah dari CI.
- [x] Commit branch dan CI run yang lulus dicatat.

Bukti M5 pada branch `feat/m5-core-ecosystems`, PR
[#4](https://github.com/Skirja/ttc-ai/pull/4):

- Commit `9228fb50dff7b22c32f2becc270de9d162a2f347` memuat implementasi M5,
  dokumentasi, dan perbaikan isolasi toolchain untuk smoke. Seluruh 12 command
  verification M5 serta gate repository lulus lokal.
- Seluruh lima job pada [CI run 36216068862](https://github.com/Skirja/ttc-ai/actions/runs/36216068862)
  lulus pada commit tersebut, termasuk Rust baseline dan M2–M5. PR masih
  terbuka untuk review dan merge oleh pengguna.
- Artifact `m5-core-ecosystem-evidence` diunduh dan diperiksa. SHA-256:
  `fixtures.txt` `7e4fbbc9b884c119a0711ed2697a853c55fca2cbed58a2eda12e67a3ac8ad32b`,
  `reduction.txt` `ea7d79252ed72f0be41002d35d7d6ce2a30d069fa75c190971af17fe6ebb7f66`,
  `report.txt` `150cc3665bca0f778264f6fbbb6b9e20526b6d66277ca832ba0b75c99a4f33bb`.
- Smoke CI memakai Rust 1.98.1, cargo-nextest 0.9.108, Python 3.14.7,
  pytest 9.1.1, dan Go 1.27.1. Contoh baseline → TTC: Cargo/libtest
  32.385 → 497 byte; nextest 77.411 → 669; pytest success 80.415 → 679;
  `go test -v` 64.102 → 28.278. Failure mempertahankan exit status serta
  diagnostic byte-for-byte; pada Go JSON lifecycle tetap utuh.
- Run awal [36215736543](https://github.com/Skirja/ttc-ai/actions/runs/36215736543)
  menemukan smoke kehilangan default rustup setelah `HOME` diisolasi. Script
  kini mempertahankan lokasi toolchain CI dan memilih `1.98.1` eksplisit;
  run sukses di atas memverifikasi perbaikannya.
- Empat temuan review M5 diperbaiki pada commit
  `d582ea2a1f8fca7253ca2e82d1b2806467e8492a`: output failure tetap raw
  setelah baris kosong, `uv run` membaca nilai opsi sebelum memilih tool,
  `coverage run` berhenti membaca opsi saat mencapai script, dan pipeline test
  CI memakai `pipefail`. Test regresi classifier, fixture Rust, dan E2E
  generic application ditambahkan. Seluruh command verification M5, gate
  repository, serta smoke binary release lulus lokal setelah perbaikan.
- Semua lima job [CI run 36223832943](https://github.com/Skirja/ttc-ai/actions/runs/36223832943)
  lulus pada commit tersebut. Artifact `m5-core-ecosystem-evidence` diunduh
  dan diperiksa: SHA-256 `fixtures.txt`
  `cf93f2d886bc4ca3030c9913c7b33501804d658b4a13a9f4f383a750123c62c0`,
  `reduction.txt`
  `185983e5c704fdf1c95b07f5e86d412ffcfd41d4453461add8c796a94037ce7d`,
  dan `report.txt`
  `b401132de6fd558ba8e44838751878d12847931cb39f48624bbd8e8da8ed07f0`.

---

## M6 — Package scripts dan monorepo core

**Outcome:** TTC memahami intent root command dan manifest tanpa mengganti
runner, memecah execution, atau kehilangan output mixed-language.

**Dependencies:** M5.

### Implementation checklist

- [x] Tambahkan bounded lazy manifest discovery untuk package JSON, pnpm YAML,
  Cargo TOML, go.work, Turbo/Nx/Lerna JSON, dan Moon YAML; reject duplicate JSON
  keys, duplicate YAML keys, alias/tag, malformed content, symlink escape, dan
  resource overflow sebagai raw.
- [x] Enforce batas SPEC: 1 MiB/manifest, 16 MiB total, 4.096 project, 16.384
  directory entries, 64 traversal levels, dan 16 nested aliases/targets.
- [x] Parse shell expression hanya untuk token/command boundaries yang aman;
  support quote, escape, assignment, `cd`, `&&`, `;`, newline; fail raw untuk
  pipeline, background, substitution, redirection, dan control flow.
- [x] Resolve npm/pnpm/Yarn/Bun script, manager-specific lifecycle, nested
  aliases, wrapper flags, forwarded machine/watch options, selected workspace,
  recursive workspace, dan selector dependency/path/name.
- [x] Bedakan command `bun test` dari package script `bun run test`; script
  custom yang command body-nya unknown membuat seluruh invocation raw.
- [x] Implementasikan adapters Turbo, Nx `project.json`/package targets,
  run-many/affected, Lerna, Lage, dan Moon tanpa mengeksekusi discovery command.
- [x] Tambahkan Cargo members/excludes/globs dan go.work `use` discovery tanpa
  mengganti atau memecah command runner asli.
- [x] Satukan family dari compound/mixed-language root scripts dan parse runner
  prefix hanya untuk project yang ditemukan; pertahankan byte asli dan diagnostic.
- [x] Pisahkan confidence per stream, source, dan parser; batasi 4.096 source.
  Fallback untuk target tersembunyi hanya memakai family yang diketahui serta
  prefix project yang terdaftar.
- [x] Tambahkan fixture success/failure/warning/unknown/large, baseline-versus-
  TTC, invocation counter, cache, selection, cwd, ordering/concurrency, serta
  mixed JS-Go dan nested Python/Rust.
- [x] Tambahkan pinned real-tool smoke untuk npm, pnpm, Yarn Classic/modern, Bun,
  Turbo, Nx, Lerna, Lage, Moon, Cargo workspace, Go workspace, root mixed
  JavaScript-Go, dan nested Python-Rust; integrasikan job M6 Linux CI dan upload
  evidence hanya setelah semua gate lulus. Script mendukung group
  `package-managers`, `runners`, dan `systems`, serta `all` sebagai default.
- [x] Pin smoke M6 pada Rust 1.98.1, Node 24.21.0/npm 11.19.0, pnpm 9.15.9,
  Yarn Classic 1.22.22, Yarn modern 4.18.1, Bun 1.4.2, Turbo 2.11.4,
  Nx 23.2.1, Lerna 10.0.1, Lage 2.17.0, Moon 2.5.5, Vitest 5.0.1,
  Vite 8.3.1, Python 3.14.7/pytest 9.1.1, dan Go 1.27.1.
- [x] Catat alasan dependency `serde`, `globset`, dan `yaml-rust2`; commit
  Cargo.lock yang mengunci resolusi mereka.

Alasan dependency M6: `serde` dipakai langsung untuk visitor JSON yang menolak
duplicate key sebelum nilai masuk ke `serde_json`; `globset` menyediakan
pencocokan workspace glob yang tervalidasi tanpa evaluasi shell; `yaml-rust2`
menyediakan event parser agar alias/tag YAML dapat ditolak sebelum manifest
Moon/pnpm dibaca sebagai hint statis. Ketiganya dipin dan resolusinya dicatat di
`Cargo.lock` demi hasil discovery dan CI yang dapat direproduksi.

### Acceptance criteria

- [x] Root command berjalan sekali dan setiap package terpilih berjalan sebanyak
  baseline; package yang tidak terpilih tidak dijalankan TTC.
- [x] Dependency graph/order, concurrency, cache, cwd, stdin, dan environment
  tetap milik runner asli.
- [x] Large deterministic monorepo success fixture dengan ≥1.000
  passing/progress records berkurang ≥80% total stdout+stderr termasuk metadata.
- [x] Warning, diagnostic, assertion diff, stack trace, summary, stream bytes
  gagal, dan exit status sama dengan baseline; no rerun saat parsing gagal.
- [x] Cycle, depth 17, missing/malformed/duplicate manifest, prefix unknown,
  unsupported option, dan overflow fail open raw.
- [x] Custom app, machine-readable, generic runner output, watch/dev, unknown
  workspace/task, dan unprefixed hidden-target record tetap retained.
- [x] Semua manager/runner smoke memakai versi pin; tidak ada tool yang hilang
  atau job yang diskip.

### Verification commands

```bash
cargo test --test manifests
cargo test --test package_scripts
cargo test --test monorepo
cargo test --test mixed_monorepo
cargo test --test classifier
cargo test --test filter_safety
cargo test --test javascript_e2e
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
sh scripts/m6-smoke.sh target/release/ttc target/m6-evidence
```

### Evidence

- [x] Commit implementasi dan seluruh verification commands dicatat;
  dokumentasi evidence disimpan pada commit evidence sesi ini.
- [x] Pinned tool versions serta invocation count root/package/task tiap fixture
  dicatat di report smoke.
- [x] Baseline/filter comparison, exit/signal, diagnostic retention, replay
  checksum, dan reduction report disimpan pada artifact CI.
- [x] CI M6 pada PR lulus seluruh job; artifact `m6-monorepo-evidence` diunduh
  dan diperiksa, lalu URL run dan checksum artifact dicatat.

Bukti lokal M6 pada branch `feat/m6-package-monorepo`:

- Tujuh command `cargo test --test` yang tercantum di atas, format check,
  Clippy dengan warnings denied, semua target/feature test, dan release build
  lulus setelah perbaikan review lokal.
- `cargo test --test monorepo -- --nocapture` mencatat 2.000 record, satu
  invocation root, dua invocation package, serta 95.780 → 381 byte total
  (99,6% berkurang, termasuk metadata TTC).
- `sh scripts/m6-smoke.sh target/release/ttc target/m6-evidence` lulus 19 kasus
  pada tool pin yang tercatat di report lokal. Marker package sama dengan
  baseline; Turbo mempertahankan overlap concurrency dan dependency order.
  Failure Turbo exit 1 serta diagnostic tetap terlihat; lima kasus
  memiliki ID dan checksum raw replay. npm/Yarn multi-workspace tanpa prefix
  dibiarkan raw sesuai SPEC.
- Report lokal berada di `target/m6-evidence/report.txt`; artifact CI final
  berasal dari PR [#5](https://github.com/Skirja/ttc-ai/pull/5), run
  [36336219189](https://github.com/Skirja/ttc-ai/actions/runs/36336219189).
  Seluruh enam job lulus pada PR head `7dec9ae626a7b7976550e20314f59425d4c68bc8`;
  report mencatat merge checkout `bd255cecbcd3a5b54218416bef460750d8b9c6fb`.
- SHA-256 artifact: `fixtures.txt`
  `76711862989824be71738e6f02e06884b33ad03c59543d37f0556e38ac143dd5`,
  `report.txt`
  `64d49a4637cfd46d4d172f781c85f6c504a87d3f5195d56a933b27308d321a92`,
  `SHA256SUMS`
  `f54b2f299d1afe783d89435b880234a665156c7d752062f23b2cf1725b802a42`.
  Checksum kedua report cocok dengan nilai pada `SHA256SUMS` yang diunduh.

Bukti tindak lanjut review pada 2026-09-28:

- Commit `bad9830fb69dde7b09c547edaadd8eb36bb5beda` memperbaiki lima temuan
  review: prefix warning tidak lagi dianggap task, target Nx dipisahkan dari
  package scripts, `npm --prefix` memilih project efektif, `npm run install`
  tetap di-resolve sebagai script, dan ampersand pada nested shell memakai
  passthrough. Regresi ditambahkan ke `monorepo`, `mixed_monorepo`,
  `package_scripts`, dan `shell_contract`.
- Commit `8e11a870adf41a967c8f3971028573d030a4b60e` membuat smoke Yarn
  deterministik saat GitHub Actions menetapkan `CI=true`; smoke tetap menguji
  output multi-package tanpa prefix, sedangkan fixture monorepo menguji prefix
  project yang terdaftar.
- Seluruh tujuh test target M6, `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test --all-targets --all-features`, dan `cargo build --release` lulus
  secara lokal. `CI=true sh scripts/m6-smoke.sh target/release/ttc
  target/m6-evidence/report.txt` lulus seluruh group pada commit `8e11a87`;
  SHA-256 report lokal:
  `1a05f655a5808449f7bf5463ed254286a98a6e592e40ad41be2d030698e01a4f`.
- PR #5 head `8e11a870adf41a967c8f3971028573d030a4b60e` lulus keenam required
  job pada [CI run 36364865946](https://github.com/Skirja/ttc-ai/actions/runs/36364865946).
  Report smoke CI mencatat merge checkout
  `457d554bbaaa911d9b2f3e558de4eee3fe79a7e2` dan tool pin M6. Artifact
  `m6-monorepo-evidence` diunduh dan checksum diverifikasi:
  `fixtures.txt` `186941afe2a112258f618ae3381c0d33d4a031c35bc91eac4226be1780a2c7f4`,
  `report.txt` `057b5f180c816ae377b5668249eb15dfdae4b1797f1844a108c0da8f7cf92e12`,
  `SHA256SUMS` `aea99a5176611ed0b602900d8cd28fb003d9a0fcfaa0eadf6a7d1aa6cc79d3ef`.
- Commit `332d8cdbaab31231c5639a4c98016ee836b0befa` menormalisasi ANSI SGR
  pada pembandingan record smoke, karena output berwarna di CI menyisipkan
  kode ANSI pada durasi test. Full smoke lokal lulus dengan
  `CI=true FORCE_COLOR=1 sh scripts/m6-smoke.sh target/release/ttc
  target/m6-evidence/report.txt`; report bersih mencatat commit `332d8cd` dan
  SHA-256 `045244863845e3ba459a2a24f9525948e9ddf8a1b12d3f4be4d54c88909f52e3`.
- Pada PR head `332d8cdbaab31231c5639a4c98016ee836b0befa`, job M6 lulus pada
  [CI run 36366060564](https://github.com/Skirja/ttc-ai/actions/runs/36366060564).
  Artifact `m6-monorepo-evidence` diunduh; checksum terverifikasi:
  `fixtures.txt` `8d8facdde79d8b58f3a9e0e7241645423724a14a41edae649c8f1d6ef575ed3a`,
  `report.txt` `ce8b156408099020432887072313b34dbc83054f0e45593697105df2f8815b29`,
  `SHA256SUMS` `c2c7c0e58f6dec1ba84753923607ac34feac56aca4b964bdd9d2fce06891e74f`.
  Report mencatat merge checkout `698854d7bab01bfe9515d39a7810333c7b9eba12`.
  Saat sesi dihentikan, baseline Rust dan job M2–M4 serta M6 lulus, sedangkan
  M5 masih `in_progress`; hasil akhirnya belum dikonfirmasi.

---

## M7 — PHP, JVM, dan .NET

**Outcome:** ecosystem server/enterprise utama memiliki parser aman, manifest
hints, fixture lengkap, dan smoke test nyata.

**Dependencies:** M4. Dapat berjalan paralel dengan M5/M6/M8 menggunakan
manifest-hint interface yang sudah dikunci M4.

### Implementation checklist

- [x] Implementasikan PHPUnit, Pest, Artisan test, PHPStan, Psalm, PHPCS,
  php-cs-fixer dry-run, dan Composer command pada SPEC.
- [x] Implementasikan Maven/mvnw dan Gradle/gradlew test/build/check serta javac
  dan JUnit console output.
- [x] Compact download/task/passing progress; retain compiler error, failed test,
  stack trace, warning, dan build summary.
- [x] Implementasikan dotnet test/build/restore/publish/format verification.
- [x] Retain diagnostic code, failed-test output, stack trace, warning, dan
  summary .NET.
- [x] Hubungkan Composer/Maven/Gradle manifest discovery ke classifier tanpa
  menambah child execution.
- [x] Tambahkan success/failure/warning/unknown/large fixtures dan pinned smoke
  project/tool untuk setiap family.

### Acceptance criteria

- [x] Semua command PHP/JVM/.NET pada SPEC dikenali atau explicit raw.
- [x] Failure diagnostic dan exit status sama dengan baseline.
- [x] Large fixture setiap family mengurangi byte minimum 80%.
- [x] PHPUnit/Pest representative, Maven dan Gradle, serta dotnet real-tool smoke
  tests lulus pada versi pin.
- [x] Unknown generic PHP/Java/.NET application output tetap raw.

### Verification commands

```bash
cargo test --test php_fixtures
cargo test --test jvm_fixtures
cargo test --test dotnet_fixtures
cargo test --test additional_ecosystem_e2e
```

### Evidence

- [x] Commit implementasi `a8eba611fe9488c3097cc264102ce8c09d70384c` dicatat.
- [x] Tool version matrix dan smoke logs lokal dicatat.
- [x] Retention/reduction report lokal disimpan.
- [ ] Required PR CI lulus dan artifact `m7-php-jvm-dotnet-evidence` tersedia.

Bukti lokal pada branch `feat/m7-php-jvm-dotnet`, PR
[#6](https://github.com/Skirja/ttc-ai/pull/6):

- `cargo test --test classifier --test manifests --test filter_safety --test php_fixtures --test jvm_fixtures --test dotnet_fixtures --test additional_ecosystem_e2e --test reduction`, `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets --all-features`, dan `cargo build --release` lulus pada commit `a8eba611fe9488c3097cc264102ce8c09d70384c`.
- Pinned smoke nyata lulus pada lingkungan terisolasi. Versi ada di
  `scripts/m7-tool-versions.txt`; log dan report lokal berada di
  `target/m7-evidence/smoke.log` dan `target/m7-evidence/report.txt`.
  SHA-256 report: `76a150affbc59c04a4355d9638d46878bb7611b225c20e439e570488a097c335`.
- Byte direct/TTC dari large real-tool smoke, termasuk metadata TTC:

| Family | Direct byte | TTC byte | Reduksi |
|---|---:|---:|---:|
| PHPUnit | 66.656 | 757 | 98,86% |
| JUnit Console | 64.916 | 835 | 98,71% |
| .NET VSTest | 38.280 | 563 | 98,53% |

- Large fixture tambahan mengukur Composer/PHPCS, Maven/Gradle, dan .NET
  restore/build; setiap grammar progress memenuhi ambang 80% setelah metadata
  TTC dihitung. Kegagalan, warning, diagnostic, stream terpisah, exit status,
  invocation count, cwd, environment, stdin, signal, dan raw replay juga diuji.
- [CI run 36550214966](https://github.com/Skirja/ttc-ai/actions/runs/36550214966)
  dibuat untuk commit yang sama, tetapi GitHub tidak mengalokasikan runner
  karena billing/account spending limit; semua job memiliki 0 step. Run ini
  bukan hasil CI hijau dan belum menghasilkan artifact M7. Required CI tetap
  pending sampai runner GitHub dapat dialokasikan lalu workflow di-rerun.
- Push dokumentasi memicu [CI run 36550562994](https://github.com/Skirja/ttc-ai/actions/runs/36550562994)
  pada PR head `5eeef81598d4fc43849608821cef3f1d80fc192e`; semua job kembali
  berhenti sebelum step dengan alasan billing yang sama.
- Setelah grammar Gradle diperketat, [CI run 36551949796](https://github.com/Skirja/ttc-ai/actions/runs/36551949796)
  dibuat untuk commit kode `a8eba611fe9488c3097cc264102ce8c09d70384c`; semua job
  kembali berhenti sebelum step akibat billing yang sama.

---

## M8 — Build tools, Ruby, Swift, container, dan infrastructure

**Outcome:** seluruh command tersisa pada SPEC memiliki perilaku filter atau raw
yang eksplisit dan teruji.

**Dependencies:** M4. Dapat berjalan paralel dengan M5/M7.

### Implementation checklist

- [x] Implementasikan CMake build, CTest, Ninja, Make test/check, dan dynamic
  target fallback.
- [x] Implementasikan Swift build/test.
- [x] Implementasikan RSpec, RuboCop, dan Rake test.
- [x] Implementasikan limited filtering untuk metadata build Docker/Compose yang
  dikenal; output Podman yang tidak cocok grammar tetap raw. Terraform validate
  diagnostic-only; Helm lint hanya compact banner lint chart.
- [x] Selalu retain plan, diff, resource changes, warning, error, dan security
  output.
- [x] Pastikan Docker Compose up, kubectl logs -f, dan seluruh interactive/watch
  list raw streaming.
- [x] Lengkapi always-raw command matrix, termasuk file/source readers, network,
  SSH, database clients, application-specific CLI, git diff, dan git show.
- [x] Generalisasi signature confidence tanpa memungkinkan satu noisy record
  mengaktifkan filter family.
- [x] Tambahkan success/failure/warning/unknown/large fixtures untuk setiap
  family filterable serta pinned tool matrix dan smoke project.
- [ ] Jalankan pinned real-tool smoke tests pada Ubuntu CI dan periksa output,
  status, retention, serta evidence artifact.

### Acceptance criteria

- [x] Setiap command pada SPEC section 8.10–8.12 terpetakan oleh matriks
  `scripts/m8-command-coverage.md` dan test terkait.
- [x] Dynamic/unknown target dan application-specific output tetap byte-exact.
- [x] Infrastructure diff/change/security content tidak pernah dikompaksi.
- [x] Large repetitive fixture tiap filterable family mengurangi byte minimum
  80%.
- [ ] Representative build, Ruby, Swift, container, dan infrastructure smoke
  tests lulus pada versi pin.

### Verification commands

```bash
cargo test --test classifier
cargo test --test filter_safety
cargo test --test build_tool_fixtures
cargo test --test ruby_swift_fixtures
cargo test --test infrastructure_fixtures
cargo test --test raw_command_matrix
cargo test --test remaining_ecosystem_e2e
cargo test --test reduction
```

Pinned tool matrix dicatat di `scripts/m8-tool-versions.txt`; archive checksum
ada di `scripts/m8-tool-checksums.txt`, dan image Podman dikunci digest-nya di
`scripts/m8-tool-images.txt`. `scripts/m8-smoke.sh` membandingkan status direct
dan TTC pada fixture CMake/CTest/Ninja/Make, Ruby, Swift, Docker/Compose,
Terraform, Helm, serta Podman. Konfigurasi, cache, project, raw capture, dan
log smoke memakai direktori temporary atau `target/m8-evidence/`.

Verifikasi lokal 2026-09-30 pada `feat/m8-remaining-ecosystems`:

- Fixture/classifier, `filter_safety`, raw command matrix, M8 E2E, dan reduction
  lulus. Baseline `cargo fmt --all -- --check`, Clippy seluruh target/fitur,
  `cargo test --all-targets --all-features`, dan `cargo build --release` juga
  lulus. Log gate dan seluruh-target test: `target/m8-evidence/repository-tests.log`;
  fixture retention/reduction: `target/m8-evidence/fixtures.log`.
- `M8_TOOL_HOME=... sh scripts/m8-install-tools.sh` memverifikasi checksum
  CMake, Ninja, Make, Terraform, dan Helm sebelum install; semua versinya cocok
  dengan matriks.
- Smoke tool nyata lokal lulus untuk CMake/CTest 3.31.6, Ninja 1.12.1, Make
  4.4.1, Terraform 1.11.4, Helm 3.17.3, dan image Podman 5.4.2 terpin digest.
  Status direct/TTC dan stream CMake/Ninja/CTest/Make, Terraform validate
  success/failure, Helm lint, serta diagnostic `security.capability` Podman
  terverifikasi. Build fixture C lokal lulus; fixture C++ ada tetapi host tidak
  menyediakan `g++`. Log berada di `target/m8-evidence/`.
- Ukuran TTC di bawah sudah termasuk metadata. Capture menyimpan input asli;
  fixture failure/warning/summary menguji konten yang dipertahankan.

| Family | Input byte | TTC byte | Reduksi |
|---|---:|---:|---:|
| CMake | 58.128 | 348 | 99,40% |
| Ninja | 34.104 | 276 | 99,19% |
| Make/CTest | 39.109 | 291 | 99,26% |
| RSpec/Rake | 5.056 | 170 | 96,64% |
| RuboCop | 7.058 | 176 | 97,51% |
| Swift build | 40.112 | 296 | 99,26% |
| Swift test | 57.129 | 347 | 99,39% |
| BuildKit metadata | 52.140 | 348 | 99,33% |
| Helm lint banner | 24.133 | 285 | 98,82% |

Smoke penuh belum dijalankan. Host tidak memiliki `g++` untuk kasus C++; Ruby
dan Swift terpin juga belum tersedia, dan Docker lokal `29.8.1` berbeda dari
smoke pin `28.1.1`. Compose build beserta smoke gabungan Ruby/Swift menunggu
job M8 di Ubuntu CI sebelum acceptance terakhir atau artifact CI dicentang.

### Evidence

- [ ] Commit implementasi dicatat.
- [x] Command coverage matrix dilampirkan di `scripts/m8-command-coverage.md`.
- [x] Tool version pins, checksum, retention, dan local reduction results
  dicatat di files serta report di atas.
- [ ] Pinned real-tool smoke log dan checksum tersedia pada artifact
  `m8-remaining-ecosystem-evidence` untuk PR CI yang berhasil.

---

## M9 — Production binary dan distribution gate

**Outcome:** standalone TTC binary selesai 100%, dapat dipasang sebagai release
candidate, dan dibuktikan oleh artifact CI sebelum pekerjaan Codex dimulai.

**Dependencies:** M1–M8 selesai.

### Implementation checklist

- [ ] Jalankan full unit, integration, fixture, monorepo, real-tool, safety, dan
  reduction suite dalam pinned CI matrix.
- [ ] Audit seluruh requirement SPEC terhadap test/implementation; tidak boleh
  ada command atau failure mode tanpa mapping.
- [ ] Tambahkan release workflow target `x86_64-unknown-linux-gnu` pada branch
  `master` dan tag `v*`.
- [ ] Pastikan tag version harus sama dengan Cargo package version.
- [ ] Build release binary, jalankan smoke test di luar source tree, dan upload
  workflow artifact.
- [ ] Hasilkan executable release asset dan SHA-256.
- [ ] Implementasikan `install.sh` latest-only untuk Linux x86_64 dengan HTTPS,
  checksum verification, atomic replacement, dan executable permission.
- [ ] Uji installer M9 terhadap local/mock release endpoint dengan override
  khusus test; interface pengguna production tetap latest-only tanpa version
  selector.
- [ ] Implementasikan idempotent managed PATH block untuk Bash beserta backup,
  `source` instruction, dan manual fallback.
- [ ] Implementasikan installation metadata yang membedakan file TTC-owned dari
  existing unrelated binary.
- [ ] Implementasikan `ttc uninstall` global; tolak bila harness masih aktif dan
  pertahankan PATH bila `~/.local/bin` dipakai program lain.
- [ ] Dokumentasikan quick install, manual binary install, update dengan rerun
  installer, PATH reload, dan global uninstall.
- [ ] Jangan memublikasikan GitHub Release pada milestone ini.

### Acceptance criteria

- [ ] Seluruh M1–M8 acceptance tetap lulus dalam satu clean CI run.
- [ ] Artifact release-mode Linux dapat berjalan tanpa repository/source tree.
- [ ] Fresh install, same-version reinstall, dan upgrade replacement atomik.
- [ ] Checksum mismatch dan unsupported platform gagal sebelum binary diganti.
- [ ] Installer tidak otomatis memasang integrasi Codex.
- [ ] PATH edit idempotent; kegagalan edit tetap menyisakan binary sehat dan
  instruksi manual yang benar.
- [ ] Global uninstall tidak menghapus unrelated binary/config/PATH usage.
- [ ] User telah merge/push ke `master` dan artifact CI Linux berhasil. Ini
  adalah hard gate M10.

### Verification commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release --target x86_64-unknown-linux-gnu
./scripts/test-install.sh
./scripts/test-release-artifact.sh
```

### Evidence

- [ ] Commit production hardening dicatat.
- [ ] Successful `master` CI run URL dicatat.
- [ ] Downloaded workflow artifact checksum dicatat.
- [ ] Full command coverage dan reduction reports dilampirkan.

---

## M10 — Integrasi Codex dan public v0.1.0

**Outcome:** production TTC terintegrasi dengan Codex melalui PreToolUse,
terbukti pada CLI nyata dan seluruh sandbox mode, lalu dirilis publik.

**Dependencies:** M9 selesai dan artifact CI Linux lulus.

### Implementation checklist

- [ ] Implementasikan internal hidden entrypoint `ttc hook codex` yang membaca
  satu event JSON stdin dan menulis satu response JSON stdout.
- [ ] Abaikan field input tambahan, tetapi tolak payload wajib yang invalid
  tanpa menghasilkan partial wrapper.
- [ ] Untuk setiap Bash command selain recursive TTC invocation, emit official
  `permissionDecision: allow` dan `updatedInput.command`.
- [ ] Quote absolute executable path dan original command sebagai satu POSIX
  argument; cover spasi, quote, dollar, newline, Unicode, operators,
  substitution, dan environment assignment.
- [ ] Implementasikan recursive prevention untuk PATH invocation, absolute path,
  dan already-wrapped shell command.
- [ ] Implementasikan `ttc install codex` sebagai config-only operation:
  verifikasi `~/.local/bin/ttc`, Codex CLI minimum 0.154.0, dan hook support.
- [ ] Tambahkan tepat satu matcher `^Bash$`, absolute hook path, backup config,
  ownership metadata, dan trust instruction tanpa mengubah model/settings lain.
- [ ] Implementasikan idempotent reinstall dan update hook ownership.
- [ ] Implementasikan `ttc uninstall codex` yang hanya menghapus hook milik TTC
  serta mempertahankan binary dan config unrelated.
- [ ] Sediakan interface adapter minimum agar harness baru dapat ditambah kelak;
  jangan implementasikan Claude pada MVP.
- [ ] Buat automated adapter/config tests tanpa login.
- [ ] Buat local Codex CLI E2E memakai login terpasang, production release-mode
  binary, temporary fixture workspace, dan `codex exec --ephemeral`.
- [ ] Backup/restore config untuk setiap run dan hapus seluruh fixture/artifact;
  jangan meninggalkan session history atau test hook.
- [ ] Jalankan E2E pada read-only, workspace-write, dan danger-full-access tanpa
  menambah permission dibanding baseline.
- [ ] Ukur bytes serta token pada request model berikutnya.
- [ ] Setelah seluruh gate lulus, tag `v0.1.0`; workflow membangun ulang,
  memverifikasi checksum/version, dan memublikasikan executable, checksum, serta
  `install.sh` ke GitHub Release.
- [ ] Smoke-test public latest installer dan `ttc install codex` dari clean
  temporary environment.

### Acceptance criteria

- [ ] Setiap Bash command dibungkus sekali; TTC command tidak recursive.
- [ ] npm, pnpm monorepo, Cargo, dan mixed JS-Go model-facing output lebih kecil.
- [ ] Large output berkurang minimum 80% secara byte dan token lebih rendah dari
  baseline.
- [ ] cat, unknown, failure tanpa kompaksi, stdout/stderr, dan exit status sama
  dengan baseline.
- [ ] Watch/dev raw streaming dan shell semantics tidak berubah.
- [ ] Seluruh sandbox mode dapat membaca/mengeksekusi installed binary dan tidak
  mendapat permission tambahan.
- [ ] Install/uninstall idempotent dan config pengguna pulih tanpa unrelated
  diff.
- [ ] E2E ephemeral tidak meninggalkan Codex session, hook, fixture, atau
  artifact test.
- [ ] GitHub Release `v0.1.0` tersedia pada `skirja/ttc-ai` dan quick-install
  command berhasil memasang binary latest.

### Verification commands

```bash
cargo test --test hook
cargo test --test codex_install
cargo test --test codex_config_ownership
cargo test --test codex_sandbox
./scripts/test-codex-e2e.sh
./scripts/test-public-install.sh
```

### Evidence

- [ ] Commit integrasi Codex dicatat.
- [ ] Codex CLI version dan sanitized ephemeral E2E report dicatat.
- [ ] Sandbox matrix serta model-facing byte/token report dilampirkan.
- [ ] Successful tag workflow dan GitHub Release URL dicatat.
