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

---

## M3 — Streaming safety, configuration, dan raw retrieval

**Outcome:** pipeline tetap bounded, dapat memulihkan semua byte yang
dikompaksi, dan aman ketika input atau storage tidak dapat diproses.

**Dependencies:** M2.

### Implementation checklist

- [ ] Gunakan bounded channel antara reader stdout/stderr dan output processor.
- [ ] Implementasikan framing per stream dengan pending line maksimum 1 MiB.
- [ ] Long line, non-UTF-8, dan framing/parser failure beralih ke raw tanpa
  kehilangan byte.
- [ ] Implementasikan config optional `~/.config/ttc/config.toml` dengan default
  `max_raw_mb = 32` dan `retention_hours = 24`.
- [ ] Invalid config tidak boleh menjalankan ulang command atau menyebabkan
  output yang tidak pasti dibuang.
- [ ] Implementasikan capture file biasa dengan random run ID, event ber-tag
  stdout/stderr, dan permission user-only.
- [ ] Capture hanya dibuat/ditahan bila minimal satu byte dikompaksi; failure
  tanpa kompaksi tetap exact passthrough tanpa TTC metadata.
- [ ] Saat ada kompaksi, capture original stdout/stderr sebelum filtering agar
  replay dapat mengembalikan output asli, subject to batas storage.
- [ ] Batasi capture total 32 MiB menggunakan tail terbaru dan metadata jumlah
  byte yang dibuang.
- [ ] Gunakan XDG state sebagai lokasi utama dan temporary directory per-user
  permission 0700 sebagai fallback sandbox.
- [ ] Jika seluruh storage gagal, hentikan kompaksi berikutnya dan emit raw tanpa
  rerun.
- [ ] Hapus capture lebih tua dari retention saat invocation berikutnya.
- [ ] Implementasikan `ttc raw ID`, `--stdout`, `--stderr`, dan `--tail N`.
- [ ] Replay default mengikuti urutan event yang diamati; selector mengekstrak
  byte stream terkait.
- [ ] Tulis summary kompaksi dan raw hint hanya ke stderr.

### Acceptance criteria

- [ ] Memory tidak tumbuh sebanding dengan ukuran output besar.
- [ ] Capture >32 MiB menyimpan tail terbaru dan dropped-byte count benar.
- [ ] Default replay, stream selector, dan tail menghasilkan byte yang benar.
- [ ] ID ditemukan pada XDG maupun temporary fallback.
- [ ] Permission directory/file hanya untuk user pemilik.
- [ ] Cleanup retention tidak menghapus capture yang masih valid.
- [ ] XDG denial memakai fallback; kegagalan kedua storage menjadi raw tanpa
  rerun.
- [ ] Invocation tanpa kompaksi tidak menghasilkan file, summary, atau hint.

### Verification commands

```bash
cargo test --test streaming
cargo test --test raw_store
cargo test --test raw_cli
cargo test --test storage_failure
cargo test --test config
```

### Evidence

- [ ] Commit implementasi dicatat.
- [ ] Peak-memory result untuk large stream dicatat.
- [ ] Permission, truncation, fallback, dan cleanup test artifacts dicatat.

---

## M4 — Filter engine dan JavaScript/TypeScript vertical slice

**Outcome:** satu vertical slice filtering lengkap membuktikan classifier,
safe-retain policy, capture, summary, dan output reduction end-to-end.

**Dependencies:** M3.

### Implementation checklist

- [ ] Definisikan filter interface yang menerima original command, manifest
  hints, stream, dan record tanpa memiliki process execution.
- [ ] Default seluruh record adalah retain; hanya recognizer ber-confidence
  cukup yang boleh compact.
- [ ] Retain warning, error, failure, panic, fatal, assertion/diff, deprecation,
  vulnerability/security, stack trace, path+line/column, dan final summary.
- [ ] Kenali ANSI untuk klasifikasi tetapi emit byte asli bagi record retained.
- [ ] Implementasikan command classifier dan delayed output-signature confidence;
  record sebelum confidence tetap raw.
- [ ] Implementasikan machine-readable flags dan always-raw command policy dari
  SPEC.
- [ ] Implementasikan npm, pnpm, yarn, bun, npx/pnpx/bunx, serta nested tool
  dispatch yang tidak menjalankan command tambahan.
- [ ] Implementasikan seluruh JS/TS test runner pada SPEC.
- [ ] Implementasikan JS/TS lint, typecheck, build, dan format-check families.
- [ ] Filter progress/passing/duplicate diagnostic yang terbukti aman; failed
  tests, diffs, stack traces, warnings, dan summaries tetap lengkap.
- [ ] Tambahkan success, failure, warning, unknown-format, dan 1.000+ record
  fixture untuk setiap output family.
- [ ] Pin versi tool nyata yang dipakai smoke tests.

### Acceptance criteria

- [ ] Setiap retained diagnostic byte-identical dengan fixture input.
- [ ] Unknown format dan JSON/JSONL/XML/YAML/SARIF/TAP tetap raw kecuali parser
  lossless khusus tersedia.
- [ ] Setiap large fixture mengurangi output minimum 80% secara byte.
- [ ] Summary menghitung passing/progress records dengan benar dan hanya muncul
  bila ada kompaksi.
- [ ] Raw ID mengembalikan original output hingga batas capture; truncation di
  atas batas selalu dinyatakan lewat metadata.
- [ ] Minimal satu E2E nyata per JS/TS family lulus dengan exit status baseline.

### Verification commands

```bash
cargo test --test classifier
cargo test --test filter_safety
cargo test --test javascript_fixtures
cargo test --test javascript_e2e
cargo test --test reduction
```

### Evidence

- [ ] Commit implementasi dicatat.
- [ ] Tabel fixture/tool version dan hasil smoke test dicatat.
- [ ] Laporan byte reduction per large fixture disimpan.

---

## M5 — Rust, Python, dan Go

**Outcome:** seluruh ecosystem core non-JavaScript pada SPEC memiliki filter aman
dan smoke test nyata.

**Dependencies:** M4.

### Implementation checklist

- [ ] Implementasikan Cargo test/nextest/build/check/clippy/fmt/doc termasuk
  workspace dan package selection.
- [ ] Compact passing tests serta Compiling/Checking progress; retain compiler
  diagnostics, warnings, failure output, dan summary.
- [ ] Implementasikan Python/python3, uv, Poetry, dan Pipenv wrapper detection.
- [ ] Implementasikan pytest, unittest, tox/nox, Ruff, mypy, pyright, pylint,
  Black, coverage, dan install families pada SPEC.
- [ ] Generic Python application tetap raw bila signature tidak dikenali.
- [ ] Implementasikan Go test/build/vet/generate, golangci-lint, dan staticcheck.
- [ ] Implementasikan lossless-aware parser khusus `go test -json`; JSON generic
  tetap raw.
- [ ] Retain failed test output, panic, race detector, vet/build diagnostic, dan
  package summary.
- [ ] Tambahkan success/failure/warning/unknown/large fixtures dan pinned real
  smoke project untuk ketiga ecosystem.

### Acceptance criteria

- [ ] Seluruh command Rust, Python, dan Go pada SPEC terpetakan ke recognizer atau
  explicit raw behavior.
- [ ] Generic app output tetap byte-exact.
- [ ] Diagnostic dan exit/signal sama dengan baseline untuk success dan failure.
- [ ] Large fixture tiap ecosystem mengurangi byte minimum 80%.
- [ ] Cargo, pytest, dan Go real-tool smoke E2E lulus pada versi pin.

### Verification commands

```bash
cargo test --test rust_fixtures
cargo test --test python_fixtures
cargo test --test go_fixtures
cargo test --test core_ecosystem_e2e
cargo test --test reduction
```

### Evidence

- [ ] Commit implementasi dicatat.
- [ ] Versi tool dan real-project smoke output dicatat.
- [ ] Laporan retention serta reduction per ecosystem disimpan.

---

## M6 — Package scripts dan monorepo core

**Outcome:** TTC memahami intent root command dan manifest tanpa mengganti
runner, memecah execution, atau kehilangan output mixed-language.

**Dependencies:** M5.

### Implementation checklist

- [ ] Parse package scripts npm/pnpm/yarn/bun secara statis tanpa mengeksekusi
  discovery command.
- [ ] Resolve lifecycle, nested alias, compound script, dan nested tool family
  dengan recursion depth maksimum 16 serta cycle detection.
- [ ] Parsing manifest gagal berarti raw.
- [ ] Honor npm workspace, pnpm recursive/filter/dir, Yarn workspace/foreach,
  dan Bun script behavior pada SPEC.
- [ ] Implementasikan Turbo, Nx, Lerna, Lage, dan Moon command recognition.
- [ ] Baca package/workspace/Turbo/Nx/Lerna manifests hanya sebagai hints;
  original root command tetap authoritative.
- [ ] Tambahkan Cargo workspace dan Go workspace discovery adapters tanpa
  mengubah execution graph runner.
- [ ] Gabungkan beberapa filter family untuk compound/mixed-language root script.
- [ ] Pastikan output-signature fallback dapat mengaktifkan family yang child
  command-nya tidak terlihat.
- [ ] Buat seluruh required monorepo fixture di SPEC, termasuk nested script ke
  Python/Rust dan mixed JavaScript-Go.

### Acceptance criteria

- [ ] Root command dan setiap expected package berjalan tepat satu kali.
- [ ] Dependency graph, concurrency, cache, cwd, dan environment tetap milik
  runner asli.
- [ ] Passing/progress berkurang minimum 80% pada large monorepo success fixture.
- [ ] Error dari package atau ecosystem mana pun terlihat lengkap.
- [ ] Exit code sama dengan baseline.
- [ ] Depth overflow, cycle, missing manifest, dan malformed manifest aman raw.

### Verification commands

```bash
cargo test --test manifests
cargo test --test package_scripts
cargo test --test monorepo
cargo test --test mixed_monorepo
```

### Evidence

- [ ] Commit implementasi dicatat.
- [ ] Invocation-count report tiap fixture dicatat.
- [ ] Baseline/filter comparison dan reduction report disimpan.

---

## M7 — PHP, JVM, dan .NET

**Outcome:** ecosystem server/enterprise utama memiliki parser aman, manifest
hints, fixture lengkap, dan smoke test nyata.

**Dependencies:** M4. Dapat berjalan paralel dengan M5/M6/M8 menggunakan
manifest-hint interface yang sudah dikunci M4.

### Implementation checklist

- [ ] Implementasikan PHPUnit, Pest, Artisan test, PHPStan, Psalm, PHPCS,
  php-cs-fixer dry-run, dan Composer command pada SPEC.
- [ ] Implementasikan Maven/mvnw dan Gradle/gradlew test/build/check serta javac
  dan JUnit console output.
- [ ] Compact download/task/passing progress; retain compiler error, failed test,
  stack trace, warning, dan build summary.
- [ ] Implementasikan dotnet test/build/restore/publish/format verification.
- [ ] Retain diagnostic code, failed-test output, stack trace, warning, dan
  summary .NET.
- [ ] Hubungkan Composer/Maven/Gradle manifest discovery ke classifier tanpa
  menambah child execution.
- [ ] Tambahkan success/failure/warning/unknown/large fixtures dan pinned smoke
  project/tool untuk setiap family.

### Acceptance criteria

- [ ] Semua command PHP/JVM/.NET pada SPEC dikenali atau explicit raw.
- [ ] Failure diagnostic dan exit status sama dengan baseline.
- [ ] Large fixture setiap family mengurangi byte minimum 80%.
- [ ] PHPUnit/Pest representative, Maven dan Gradle, serta dotnet real-tool smoke
  tests lulus pada versi pin.
- [ ] Unknown generic PHP/Java/.NET application output tetap raw.

### Verification commands

```bash
cargo test --test php_fixtures
cargo test --test jvm_fixtures
cargo test --test dotnet_fixtures
cargo test --test additional_ecosystem_e2e
```

### Evidence

- [ ] Commit implementasi dicatat.
- [ ] Tool version matrix dan smoke logs dicatat.
- [ ] Retention/reduction report disimpan.

---

## M8 — Build tools, Ruby, Swift, container, dan infrastructure

**Outcome:** seluruh command tersisa pada SPEC memiliki perilaku filter atau raw
yang eksplisit dan teruji.

**Dependencies:** M4. Dapat berjalan paralel dengan M5/M7.

### Implementation checklist

- [ ] Implementasikan CMake build, CTest, Ninja, Make test/check, dan dynamic
  target fallback.
- [ ] Implementasikan Swift build/test.
- [ ] Implementasikan RSpec, RuboCop, dan Rake test.
- [ ] Implementasikan limited filtering untuk Docker/Podman build, Docker
  Compose build, Terraform validate, dan Helm lint.
- [ ] Selalu retain plan, diff, resource changes, warning, error, dan security
  output.
- [ ] Pastikan Docker Compose up, kubectl logs -f, dan seluruh interactive/watch
  list raw streaming.
- [ ] Lengkapi always-raw command matrix, termasuk file/source readers, network,
  SSH, database clients, application-specific CLI, git diff, dan git show.
- [ ] Generalisasi signature confidence tanpa memungkinkan satu noisy record
  mengaktifkan filter family.
- [ ] Tambahkan fixture lengkap untuk setiap family dan pinned real-tool smoke
  test yang representatif.

### Acceptance criteria

- [ ] Setiap command pada SPEC section 8 terpetakan oleh test matrix.
- [ ] Dynamic/unknown target dan application-specific output tetap byte-exact.
- [ ] Infrastructure diff/change/security content tidak pernah dikompaksi.
- [ ] Large repetitive fixture tiap filterable family mengurangi byte minimum
  80%.
- [ ] Representative build, Ruby, Swift, container, dan infrastructure smoke
  tests lulus pada versi pin.

### Verification commands

```bash
cargo test --test build_tool_fixtures
cargo test --test ruby_swift_fixtures
cargo test --test infrastructure_fixtures
cargo test --test raw_command_matrix
cargo test --test remaining_ecosystem_e2e
```

### Evidence

- [ ] Commit implementasi dicatat.
- [ ] Command coverage matrix dilampirkan.
- [ ] Tool versions, retention, dan reduction results dicatat.

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
