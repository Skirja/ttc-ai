# TTC-AI — Status Implementasi, Gap, Masalah, dan Roadmap

Dokumen ini mencatat kondisi aktual TTC-AI berdasarkan plan implementasi yang
telah disepakati. Statusnya adalah **functional early implementation**: core
engine sudah dapat digunakan secara eksplisit, tetapi proyek belum memenuhi
seluruh target production/full-parity pada plan awal. Secara khusus, milestone
integrasi Codex berstatus **partial**, bukan completed: hook sudah terpasang tetapi
belum berada di jalur eksekusi yang memfilter model-facing output.

## Status singkat

| Area | Status | Keterangan |
|---|---|---|
| Rust project dan CLI `ttc` | Selesai untuk MVP | Binary debug dan release berhasil dibangun |
| Pemahaman command dan script | Selesai sebagian | Parser, wrapper, package script, workspace, dan static task resolver tersedia |
| Eksekusi command asli | Selesai untuk jalur yang diuji | Argumen, cwd, environment, lifecycle package manager, exit code, dan signal diuji |
| Semantic filtering eksplisit | Selesai sebagian | Filter konservatif tersedia melalui `ttc run`; full RTK parity belum selesai |
| Recovery/recall | Selesai untuk MVP | SQLite metadata, zstd capture, selector recall, quota, TTL, dan permission tersedia |
| Runtime execution evidence | Selesai untuk proses top-level | Spawn/finish, exit/signal, durasi, dan counter stream dicatat tanpa inferensi stage nested |
| Metrics/gain | Selesai untuk MVP | Statistik lokal dan estimasi token berbasis byte tersedia |
| Infrastruktur hook Codex | Terpasang, bukan selesai | `PreToolUse` matcher `^Bash$` sudah ditambahkan dan trusted, tetapi handler hanya mengembalikan `{}` |
| Codex automatic rewrite + output filtering | Belum berjalan — P0 partial | Bash belum dirutekan melalui TTC; model-facing output belum difilter dan pengurangan token belum terbukti end-to-end |
| CI | Konfigurasi selesai | Linux/macOS/Windows matrix ditulis; hanya Linux yang sudah dijalankan lokal |
| Full ecosystem/RTK parity | Belum selesai | Support matrix membedakan partial, recognition-only, dan passthrough |

### Definition of Done integrasi Codex

Integrasi Codex hanya boleh disebut **selesai dan berjalan** jika seluruh kondisi
berikut terbukti dalam pengujian end-to-end:

1. command Bash yang eligible di-rewrite otomatis agar dieksekusi melalui TTC;
2. command asli tetap authoritative dan hanya dieksekusi sekali dengan approval,
   shell, cwd, exit status, dan signal yang ekuivalen;
3. output yang diterima model benar-benar merupakan output TTC yang sudah terfilter;
4. pengurangan context/token diukur, bukan disimpulkan dari hook yang registered atau
   trusted.

Hook yang hanya mengembalikan `{}` dan penggunaan manual `ttc run` tidak memenuhi
Definition of Done ini.

## Yang sudah dilakukan

### Fondasi dan arsitektur

- Membuat package Rust `ttc-ai` dengan executable `ttc`.
- Memisahkan modul command, config, execution, filtering, integrations,
  recovery, resolver, dan project discovery.
- Mendefinisikan tipe inti seperti `Invocation`, `CommandAst`, `WorkloadGraph`,
  `Classification`, `Risk`, `OutputEvent`, dan `ProcessOutcome`.
- Menentukan lisensi dual `MIT OR Apache-2.0`.
- Menambahkan toolchain Rust stabil yang dipin ke `1.98.1`.
- Menambahkan GitHub Actions untuk `fmt`, Clippy, test, dan release build pada
  Linux, macOS, serta Windows.

### Parsing dan command understanding

- Menggunakan Tree-sitter Bash untuk parsing POSIX, dengan source span dan
  deteksi syntax error.
- Tidak menggunakan `.split_whitespace()` sebagai parser POSIX.
- Mempertahankan command asli terpisah dari hasil klasifikasi workload.
- Menandai pipeline, redirection, command substitution, function, loop,
  dynamic expansion, dan shell syntax yang tidak didukung sebagai uncertain atau
  exact-output.
- Menyediakan grammar literal terpisah untuk PowerShell dan `cmd.exe`; bentuk
  kompleks masih passthrough.
- Menangani environment prefix dan wrapper seperti `env`, `cross-env`,
  `timeout`, `command`, `uv`, `poetry`, `bundle`, `python -m`, serta beberapa
  bentuk `sudo`, `xargs`, dan container wrapper.

### Resolusi project script dan workload

- Membaca `package.json`, `composer.json`, `pnpm-workspace.yaml`, serta static
  `Makefile`, `justfile`, dan `Taskfile`.
- Menangani npm, pnpm, Yarn, Bun, npx, pnpx, bunx, `pnpm dlx`, dan executor
  package-backed seperti Turbo, Nx, dan Lerna pada bentuk yang didukung.
- Membedakan script bernama `lint`, `test`, `check`, atau `build` dari tool yang
  sebenarnya dijalankan.
- Menangani script nested, pre/post lifecycle yang terdefinisi, selector
  workspace dasar, recursive workspace, dan cycle detection.
- Membatasi resolusi ke 32 level dan 4.096 node.
- Menambahkan persistent graph cache berbasis hash isi manifest/config serta
  validasi perubahan anggota workspace.
- Menyimpan confidence dan notes pada workload graph agar uncertainty dapat
  diaudit melalui `ttc explain`.

### Semantik eksekusi dan keamanan

- `ttc run -- <argv...>` menjalankan executable asli dengan argv asli.
- Mode shell menyerahkan command text apa adanya ke shell yang dipilih.
- Cwd, environment, stdin, lifecycle, forwarding argumen, dan urutan shell
  tidak diganti dengan hasil inferensi workload.
- Exit code asli dipertahankan, termasuk kode `254` dan kode tool-specific.
- Signal termination Unix dibedakan dari numeric exit code dan diteruskan ke
  child process group.
- Command interaktif, watch mode, machine-readable output, pipeline,
  redirection, exact file content, dan output binary diperlakukan konservatif.
- Command destruktif/privileged tidak otomatis di-wrap.
- `TTC_BYPASS=1`, `TTC_RECOVERY=0`, dan `ttc raw -- ...` tersedia untuk bypass.

### Capture, filtering, dan recovery

- Capture stdout/stderr dilakukan melalui reader thread dan bounded channel.
- Raw event diberi channel tag dan disimpan dalam stream zstd.
- Filter memproses output secara incremental; memory tidak bertambah sebanding
  dengan ukuran total log.
- Output tidak dikenal dan diagnostic tetap dipertahankan.
- Tersedia recognizer konservatif untuk test/build/package/container progress,
  Git status, search result dengan file/line, serta duplicate
  location-bearing diagnostics.
- Warning, error, panic, fatal, expected/actual, source location, dan teks
  diagnostik baru mencegah suppression.
- Capture limit atau storage failure mengubah sisa output menjadi raw tanpa
  menjalankan command ulang.
- Recovery memakai random 128-bit ID, private directory/file permissions, atomic
  publication, SQLite metadata, quota reservation, TTL, max entries, dan cleanup.
- `ttc recall` mendukung full recall, `--head`, `--tail`, `--grep`, `--lines`,
  `--stdout`, `--stderr`, dan `--raw`.
- `ttc gain` memberi bytes, estimasi token, reduction, dan history lokal.
- Metadata recovery menyimpan `ExecutionEvidence` untuk proses top-level yang
  benar-benar dijalankan; stage nested tetap eksplisit `unavailable`.

### Codex integration

- Menambahkan installer Codex yang memilih `hooks.json` atau inline
  `config.toml`.
- Installer membuat backup, mempertahankan konfigurasi/comment yang tidak terkait,
  idempotent, memiliki ownership receipt, checksum binary, dry-run, dan surgical
  uninstall.
- Binary release dicopy ke `/home/skirja/.codex/bin/ttc`.
- Hook terdaftar di `/home/skirja/.codex/config.toml`.
- `ttc doctor` memeriksa binary, receipt, config, hook registration, hook
  executable, recovery directory, dan versi Codex.
- Hook malformed/unknown/unsupported selalu fail-open.
- Adapter memakai event bertipe dan compatibility report versioned; tidak ada
  serializer rewrite dorman atau profile yang dapat diaktifkan lewat konfigurasi.
- Matrix langsung menggunakan Codex terpasang dan mock Responses server sudah
  dijalankan dalam temporary Codex home.
- Seluruh item di atas adalah infrastruktur dan compatibility evidence; belum ada
  jalur automatic filtering yang mengurangi output/token Codex.

### Dokumentasi dan research

- README, architecture, configuration, security/recovery, contributing, support
  matrix, Codex compatibility, benchmark, implementation report, dan research
  ledger sudah ditambahkan.
- RTK release `v0.49.0`, develop revision, issue references, source hashes, dan
  interface inventory dicatat di `docs/research/`.
- Tidak ada source RTK yang divendor atau disalin secara substantial.
- Konflik nama crate `ttc` yang sudah dipakai project lain didokumentasikan;
  package tetap bernama `ttc-ai` dan executable tetap `ttc`.

## Hasil validasi saat ini

- `cargo fmt --check`: lulus.
- `cargo clippy --locked --all-targets -- -D warnings`: lulus.
- `cargo test --locked`: **66 test lulus** pada Linux.
- Golden suite: **115 fixture**, diuji pada tiga ukuran chunk, total 345
  perbandingan output.
- Property tests mencakup quoting, malformed shell/JSON, printable input, dan
  recovery ID.
- Real npm/pnpm test: empat kasus success/failure mempertahankan lifecycle trace,
  argv, dan exit code `0`/`7`.
- Release build: lulus.
- Installed binary validation: lulus; test suite proyek berhasil dijalankan dan
  raw recall diverifikasi.
- Benchmark synthetic: reduction output sekitar **99,62%–99,96%** pada fixture
  noisy test/build; startup median **1,252 ms**, classification median **1,451
  ms**, composite resolution median **1,557 ms**.
- Peak RSS synthetic sekitar **9 MiB** pada input 1, 16, dan 80 MiB.
- Angka benchmark adalah hasil fixture lokal, bukan jaminan untuk semua project
  dan bukan klaim penghematan billing provider.

Artefak hasil tersedia di:

- [implementation-report.md](implementation-report.md)
- [benchmarks.json](benchmarks.json)
- [real-manager-results.json](real-manager-results.json)
- [installed-validation.json](installed-validation.json)
- [local-installation.json](local-installation.json)

## Yang belum selesai

### Belum selesai secara prinsip

- Automatic Codex command rewrite dan model-facing output filtering belum aktif;
  belum ada pengurangan token Codex yang dapat diklaim dari hook terpasang.
- Full RTK feature parity belum tercapai.
- Filter semantic untuk banyak tool baru berupa recognition atau passthrough.
- Native macOS dan Windows belum diuji dalam environment ini.
- Approval/sandbox equivalence antara command asli dan command yang di-wrap belum
  terbukti.

### Gap coverage utama

- Git diff/log/show dan search compaction masih jauh dari parity penuh.
- ESLint, Biome, oxlint, Prettier, TypeScript, Ruff, Mypy, dan Go linter sudah
  dikenali serta diagnostic tertentu dapat dideduplicate, tetapi belum memiliki
  parser semantic sedalam target awal.
- Structured output seperti JSON, SARIF, JUnit, compiler JSON, dan Go JSON belum
  memiliki jalur lengkap per ecosystem.
- PHP, JVM, .NET, Ruby, C/C++, Swift, cloud, Kubernetes, Terraform, dan network
  baru mencakup subset konservatif.
- Composer nested behavior, pnpm selector semantics kompleks, Nx executor
  configuration, inherited task dependencies, dan dynamic task definitions belum
  lengkap.
- Container command dapat dikenali, tetapi filesystem di dalam container tidak
  di-introspect.
- PowerShell/cmd baru memiliki parsing literal konservatif.
- Harness adapter baru Codex; Claude, Gemini, Cursor, Copilot, OpenCode, dan
  harness lain belum diimplementasikan.
- Persistent discovery cache sudah ada, tetapi belum memiliki `ttc cache` CLI
  terpisah untuk inspeksi/purge cache discovery.
- Metrics belum menghitung provider tokenizer aktual; estimasi masih bytes/4.
- Windows ACL, console signal, command quoting, dan installer behavior belum
  divalidasi native.

## Masalah dan risiko yang ditemukan

### 1. Risiko terbesar: automatic rewrite dapat mengubah approval semantics

Codex `PreToolUse` mendukung `permissionDecision: "allow"` dengan
`updatedInput.command`, tetapi kontrak itu tidak membuktikan bahwa command asli
akan tetap dinilai oleh approval/prefix policy setelah diganti menjadi wrapper.
Wrapping command berbahaya dapat menyamarkan operasi pada classifier Codex.

Keputusan saat ini: `compatibility_verified()` bernilai false dan hook hanya
mengembalikan `{}`. Jangan mengaktifkannya dengan sekadar mengganti boolean.

### 2. Matrix membuktikan prefix denial dapat dilewati candidate rewrite

Pada Codex `0.154.0-alpha.6.2`, baseline tanpa rewrite memblokir executable
sintetis sesuai execpolicy, tetapi executable yang sama berjalan setelah candidate
`PreToolUse` rewrite mengembalikan `allow`. Command probe hanya mencetak sentinel
dan tidak destruktif. Automatic rewrite TTC tetap nonaktif.

### 3. Hook tidak menerima semua metadata eksekusi

Probe Codex terpasang menunjukkan event `PostToolUse` menerima raw text, tetapi
tidak menerima exit status command. Probe juga menunjukkan hook `cwd` dapat berbeda
dari subdirectory yang dipakai execution tool. Oleh karena itu PostToolUse belum
aman dijadikan pengganti automatic pre-execution wrapper.

### 4. Filtering di level hook tidak dapat memulihkan truncation sebelumnya

Jika Codex sudah memotong output sebelum `PostToolUse`, TTC tidak dapat mengembalikan
bytes yang tidak pernah diterima hook. Recovery TTC berlaku pada explicit
`ttc run` atau integration path yang menerima raw output penuh.

### 5. Workload graph bukan bukti stage benar-benar dijalankan

Resolver hanya membaca kemungkinan command dari script. Ia tidak boleh menyatakan
semua stage berhasil, terutama ketika `&&` berhenti di stage awal atau output
interleaved. `ExecutionEvidence` sekarang hanya mencatat proses top-level yang
teramati dan menandai atribusi nested sebagai `unavailable`.

### 6. Penghematan sangat tinggi pada fixture tertentu

Reduction 99% lebih adalah synthetic result untuk output repetitif. Hasil nyata
akan lebih rendah pada failure-heavy output, structured output, pipeline, atau
command yang sengaja meminta detail penuh. Angka itu tidak boleh dipakai sebagai
janji billing.

### 7. Platform coverage belum seimbang

Linux adalah satu-satunya platform yang benar-benar dijalankan. CI matrix sudah
disiapkan, tetapi native Windows/macOS execution, permissions, signals, shell
quoting, dan Codex hook path masih memerlukan run aktual.

### 8. Parity ledger masih menunjukkan pekerjaan terbuka

Ledger RTK berisi 157 capability/source entries dan sengaja menandai banyak item
sebagai partial atau no parity. Ini adalah catatan transparansi, bukan indikasi
bahwa semua item sudah selesai.

## Pengembangan selanjutnya

### Prioritas P0 — keamanan dan Codex integration

1. **Status milestone: partial.** Jangan ubah menjadi completed sampai Bash rewrite,
   model-facing filtering, dan pengurangan token terbukti end-to-end.
2. **Masih diblokir eksternal:** dapatkan kontrak Codex yang mempertahankan
   command asli untuk approval classification sambil mengubah model-facing output.
3. **Selesai untuk versi terpasang:** matrix mencakup prefix denial, escalation,
   explicit shell, explicit workdir, unknown command, competing hooks, permission
   mode, dan PostToolUse exit status; verdict saat ini `incompatible`.
4. **Selesai untuk boundary yang dapat diamati:** `ExecutionEvidence` mencatat
   proses top-level mulai/selesai, exit/signal, durasi, serta stream; nested stage
   tidak disimpulkan dari graph.
5. **Tetap nonaktif:** automatic rewrite baru dapat dipertimbangkan setelah
   kontrak dan seluruh invariant lulus. Mutating, privileged, unknown,
   interactive, pipeline, dan dynamic script tetap passthrough.
6. **Acceptance test wajib:** jalankan command Bash melalui Codex, buktikan TTC
   mengeksekusi command asli tepat sekali, bandingkan model-facing bytes/token
   sebelum dan sesudah filtering, lalu simpan hasilnya sebagai artifact versioned.

### Prioritas P1 — semantic filter parity

1. Implementasikan parser terpisah untuk Git status/diff/log/show, ripgrep, Cargo,
   Go, pytest, Vitest/Jest/Playwright, TypeScript, ESLint/Biome, Ruff/Mypy,
   PHP, JVM, .NET, Ruby, dan Docker.
2. Prioritaskan structured output yang memang sudah diminta user/tool; jangan
   menyuntikkan flag JSON secara diam-diam.
3. Tambahkan golden fixture failure/warning/large/edge untuk setiap filter dan
   assertion bahwa failure diagnostic tetap dapat dipakai agent untuk memperbaiki
   masalah.
4. Tambahkan composite-output segmentation dengan explicit stage markers dan
   fallback raw untuk interleaving yang ambigu.

### Prioritas P1 — resolver dan workspace

1. Lengkapi semantics versi npm/pnpm/Yarn/Bun, termasuk lifecycle configuration,
   filter selectors, recursive flags, `--dir`/`--prefix`, dan argument forwarding.
2. Perluas workspace resolver ke glob negation, dependency selectors, package
   aliases, nested workspace roots, dan changed-file selectors.
3. Tambahkan static introspection untuk `turbo.json`, `nx.json`, Composer,
   Gradle/Maven, dan task systems tanpa mengeksekusi konfigurasi.
4. Tambahkan cache inspect/purge dan cache invalidation test untuk rename/delete
   workspace package.

### Prioritas P2 — reliability dan observability

1. Jalankan CI matrix nyata pada macOS dan Windows.
2. Validasi Windows ACL, `command_windows`, PowerShell, cmd quoting, console
   signals, dan installer rollback.
3. Tambahkan crash/fault injection untuk disk-full, permission denied, corrupted
   zstd, SQLite busy/locked, process death, broken pipe, dan child spawn failure.
4. Tambahkan bounded queue/backpressure tests dengan stdout/stderr interleaving
   besar.
5. Pertimbangkan tokenizer lokal yang lebih akurat, tetap melaporkan estimasi
   sebagai estimasi dan bukan billing aktual.

### Prioritas P2 — harness expansion

Setelah Codex path aman, buat adapter tipis untuk Claude Code, Gemini CLI, Cursor,
Copilot, OpenCode, dan harness lain. Core tidak boleh bergantung pada format JSON
atau approval semantics satu harness. Setiap adapter harus memiliki protocol
fixture, malformed input test, no-op behavior, dan security review sendiri.

## Cara menjalankan kondisi saat ini

```bash
cd /home/skirja/Work/Personal/ttc-ai
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked

~/.codex/bin/ttc doctor
~/.codex/bin/ttc explain 'pnpm check' --json
~/.codex/bin/ttc run -- cargo test --locked
~/.codex/bin/ttc gain --history
```

Hook Codex dapat ditinjau melalui `/hooks`. Pada versi saat ini, output hook yang
diharapkan untuk command aman maupun unknown adalah `{}`, karena automatic rewrite
memang belum diaktifkan. Hasil ini hanya membuktikan hook dapat dipanggil; hasil ini
tidak membuktikan filtering atau pengurangan token.

## Kesimpulan status

TTC-AI sudah memiliki fondasi produk yang dapat diuji dan digunakan secara
eksplisit: command asli tetap authoritative, filtering konservatif, recovery
tersedia, dan infrastruktur hook Codex sudah terpasang secara fail-open. Integrasi
Codex penghemat token belum berjalan dan milestone P0 tetap partial. TTC-AI belum
boleh disebut selesai karena automatic Bash rewrite, model-facing filtering,
pengukuran pengurangan token, full RTK parity, dan cross-platform validation masih
terbuka.
