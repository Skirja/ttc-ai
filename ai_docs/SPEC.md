# Greenfield Specification — TTC Automatic Bash Output Filter

**Status:** Proposed<br>
**Target awal:** Linux, Bash/POSIX, Codex CLI<br>
**Implementasi:** binary Rust baru tanpa membawa kode atau arsitektur TTC lama<br>
**Versi rilis pertama:** 0.1.0 (SemVer)<br>
**Lisensi:** MIT<br>
**Urutan wajib:** selesaikan binary TTC terlebih dahulu, lalu buat hook Codex

## 1. Tujuan

TTC adalah wrapper command lokal untuk mengurangi output Bash yang masuk ke konteks Codex.

Hook Codex selalu membungkus setiap Bash command dengan TTC. TTC kemudian memilih salah satu perilaku:

1. output dikenali sebagai log test, build, lint, typecheck, atau tooling yang aman diringkas — filter;
2. output tidak dikenali atau tidak aman diringkas — teruskan output asli tanpa perubahan.

Contoh filter:

    npm run test
        ↓ hook
    ttc 'npm run test'
        ↓ TTC
    output test yang repetitif dikompaksi

Contoh passthrough:

    cat README.md
        ↓ hook
    ttc 'cat README.md'
        ↓ TTC
    seluruh isi README.md diteruskan apa adanya

Command asli selalu dijalankan tepat satu kali. TTC tidak mengganti package manager, test runner, atau monorepo runner dengan program lain.

## 2. Keputusan produk

- Semua Bash command dari Codex dibungkus TTC.
- Hook tidak mengklasifikasikan command.
- Binary TTC menentukan apakah output difilter atau diteruskan penuh.
- Default setiap output adalah retain.
- Full-access adalah mode penggunaan utama.
- Hook selalu memakai kontrak resmi Codex:
  - permissionDecision bernilai allow;
  - updatedInput.command berisi wrapper TTC.
- Tidak ada compatibility profile per versi Codex.
- Tidak ada flag automatic_rewrite.
- Tidak ada patch atau build source Codex.
- Tidak ada PostToolUse.
- Tidak ada database, daemon, service, atau telemetry pada MVP.
- Tidak ada bypass manual untuk menjalankan command tanpa filter.
- Tidak ada instruksi AGENTS.md, CLAUDE.md, atau prompt tambahan untuk meminta
  model memakai TTC.
- Linux dan shell POSIX adalah target rilis pertama.
- Artifact rilis pertama adalah binary `x86_64-unknown-linux-gnu`.
- Codex CLI minimum yang didukung adalah 0.154.0.
- Project lama hanya referensi kebutuhan; kode dan arsitekturnya tidak digunakan.

## 3. Urutan implementasi

### Phase 1 — Binary TTC standalone

Binary harus selesai dan stabil tanpa bergantung pada Codex.

Deliverable:

- executable ttc;
- direct argv execution;
- single-string native-shell execution;
- streaming stdout dan stderr;
- filter engine;
- exact passthrough;
- exit code dan signal propagation;
- seluruh ecosystem yang tercantum dalam specification;
- monorepo support;
- unit, integration, fixture, dan real-tool smoke tests;
- release-mode binary dan artifact CI Linux yang lulus acceptance Phase 1.

Hook Codex tidak dikerjakan sebelum acceptance Phase 1 lulus.

### Phase 2 — Hook Codex

Setelah Phase 1 lulus:

- tambahkan ttc hook codex;
- tambahkan ttc install codex;
- tambahkan ttc uninstall codex;
- hook selalu membungkus semua Bash command;
- test memakai Codex CLI yang benar-benar terpasang;
- buktikan output model-facing lebih pendek untuk command filterable;
- buktikan output command passthrough identik dengan baseline;
- publikasikan GitHub Release hanya setelah acceptance Phase 2 lulus.

## 4. CLI contract

### 4.1 Direct argv

Bentuk argv yang didukung binary dan digunakan oleh test/integrasi:

    ttc npm run test
    ttc pnpm test
    ttc cargo test
    ttc go test ./...
    ttc python -m pytest
    ttc dotnet test

Semua argument setelah ttc diteruskan sebagai argv asli tanpa shell tambahan.

Package script tetap menangkap seluruh child process. Contoh:

    ttc npm run check

Jika script check berisi `npx tsc --noEmit && npm run lint`, npm menjalankan
compound script tersebut sebagai child. Seluruh stdout/stderr tetap mengalir
melalui TTC dan dapat memakai filter TypeScript serta lint sekaligus.

### 4.2 Single-string shell command

Digunakan oleh hook:

    ttc 'npm run test'

Jika TTC menerima tepat satu argument yang berisi command, TTC menjalankan
argument tersebut melalui shell native:

    ttc 'npx tsc --noEmit && npm run lint'
    ttc 'FOO=bar npm test'
    ttc 'cd packages/api && npm test'
    ttc 'pnpm test && go test ./...'

Build Linux/macOS memakai shell POSIX. Build Windows nantinya memakai shell
native yang dipilih untuk platform tersebut. Hook bertanggung jawab melakukan
quoting sesuai platform sehingga seluruh command tetap satu argument.

Commands minimum:

    ttc <program> [args...]
    ttc '<complete shell command>'
    ttc raw <id>
    ttc install codex
    ttc uninstall codex
    ttc uninstall

`ttc hook codex` adalah entrypoint internal yang dipasang ke config Codex dan
tidak ditampilkan sebagai command utama pada help. Namespace internal hook
boleh ditambah untuk harness lain tanpa mengubah execution atau filter engine.

`ttc --version` membaca versi SemVer yang sama dengan package Rust. `ttc --help`
menampilkan command publik, tetapi tidak ada `explain`, `doctor`, atau
self-update pada MVP.

### 4.3 Exit behavior

- Exit code child menjadi exit code TTC.
- Jika child mati karena signal, TTC meneruskan signal yang sama.
- TTC tidak mengubah failure menjadi success.
- TTC tidak menjalankan ulang command saat filtering gagal.
- Error internal setelah child dimulai membuat output berikutnya raw, bukan rerun.

## 5. Hook Codex

### 5.1 Input minimum

Hook membaca satu object JSON PreToolUse dari stdin:

    {
      "hook_event_name": "PreToolUse",
      "tool_name": "Bash",
      "cwd": "/absolute/project/path",
      "tool_input": {
        "command": "npm run test"
      }
    }

Field tambahan diabaikan.

### 5.2 Output

Untuk setiap Bash command selain recursive TTC wrapper:

    {
      "hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "allow",
        "updatedInput": {
          "command": "'/absolute/path/to/ttc' 'npm run test'"
        }
      }
    }

Hook selalu menghasilkan wrapper. Hook tidak memeriksa apakah command filterable.

### 5.3 Quoting

POSIX quoting wajib menangani:

- spasi;
- single quote;
- double quote;
- dollar sign;
- newline;
- Unicode;
- shell operators;
- command substitution;
- environment assignment.

Original command harus menjadi satu argument setelah executable TTC. Hook tidak
menambahkan option lain di antara TTC dan command asli.

### 5.4 Recursive wrapper prevention

Jika command sudah memanggil binary TTC, hook mengembalikan object kosong.

Contoh yang tidak dibungkus ulang:

    ttc npm test
    /home/user/.local/bin/ttc 'cargo test'

### 5.5 Install dan uninstall

ttc install codex:

- tidak menyalin atau memperbarui binary;
- memverifikasi binary terpasang di `~/.local/bin/ttc`;
- memverifikasi Codex CLI minimum 0.154.0;
- menambahkan satu PreToolUse hook dengan matcher ^Bash$;
- memakai absolute path `~/.local/bin/ttc` pada command hook;
- memverifikasi fitur hook tersedia tanpa compatibility profile;
- tidak mengubah model atau setting unrelated;
- idempotent;
- membuat backup config;
- mencetak instruksi trust jika diperlukan;
- tidak menulis AGENTS.md, CLAUDE.md, atau instruction file lain.

`ttc uninstall codex` hanya menghapus hook Codex yang dimiliki TTC. Binary,
integrasi harness lain, setting unrelated, dan perubahan config setelah install
tetap dipertahankan.

`ttc uninstall` menghapus instalasi global TTC hanya jika tidak ada integrasi
harness aktif. Command ini menghapus binary dan metadata milik TTC. Managed PATH
entry dipertahankan jika `~/.local/bin` berisi program lain agar uninstall TTC
tidak merusak tool lain.

Tidak ada kondisi hook terlihat aktif tetapi diam-diam tidak wrapping. Jika hook terpasang dan trusted, setiap Bash command masuk melalui TTC.

## 6. Pipeline binary TTC

    Invocation
      → spawn original command once
      → read stdout dan stderr concurrently
      → classify command dan output signatures
      → known safe record: compact
      → unknown record: emit exactly
      → emit compact summary jika diperlukan
      → return original exit code atau signal

Klasifikasi hanya menentukan filtering, bukan permission atau eksekusi.

## 7. Filter policy

### 7.1 Default retain

Setiap baris dipertahankan kecuali recognizer spesifik membuktikan bahwa baris tersebut hanya passing record atau progress noise.

Baris selalu dipertahankan bila mengandung:

- error;
- warning atau warn;
- failed atau failure;
- panic;
- fatal;
- assert;
- expected;
- actual;
- deprecated;
- vulnerability;
- security;
- stack trace;
- file path dengan line atau column;
- failed test result.

### 7.2 Unknown output

Output unknown diteruskan penuh, termasuk:

    cat
    sed
    awk
    head
    tail
    custom scripts
    application-specific CLIs
    binary output
    format baru yang gagal diparse

### 7.3 Machine-readable output

JSON, JSONL, XML, YAML, SARIF, TAP, dan output exact lain diteruskan penuh kecuali ada parser lossless khusus.

Flags passthrough antara lain:

    --json
    --jsonl
    --xml
    --yaml
    --sarif
    --output
    -o
    --format

go test -json boleh difilter hanya menggunakan parser JSON khusus.

### 7.4 Watch, dev, dan interactive

Command berikut tetap dibungkus TTC tetapi memakai raw streaming:

    npm run dev
    pnpm dev
    yarn dev
    bun dev
    vite dev
    next dev
    nuxt dev
    cargo watch
    watchexec
    nodemon
    tail -f
    docker compose up
    kubectl logs -f

Tidak ada buffering sampai proses selesai. stdin, signal, dan TTY behavior diteruskan.

### 7.5 Summary

Jika ada output yang dikompaksi:

    TTC: 842 passing records and 136 progress records compacted

Jika tidak ada output yang dikompaksi, TTC tidak menambahkan summary.

## 8. Command yang harus dikenali

Hook membungkus semua Bash command. Daftar berikut adalah command yang outputnya harus dikenali dan difilter.

### 8.1 JavaScript dan TypeScript package managers

| Command atau pola | Perilaku |
|---|---|
| npm test | Resolve script test dan filter output |
| npm run test | Filter test output |
| npm run lint | Filter progress dan duplicate diagnostic |
| npm run typecheck | Filter progress; retain type errors |
| npm run build | Filter build progress |
| npm run check | Gabungkan filter dari script body |
| npm ci, npm install | Filter download/progress; retain audit dan warning |
| npm --workspace PKG test | Filter selected workspace |
| npm run test --workspaces | Filter multi-workspace output |
| pnpm test, pnpm run test | Filter test output |
| pnpm lint, pnpm run lint | Filter lint output |
| pnpm typecheck, pnpm run typecheck | Filter typecheck output |
| pnpm build, pnpm run build | Filter build output |
| pnpm -r test, pnpm --recursive test | Multi-package filtering |
| pnpm --filter SELECTOR test | Selected-package filtering |
| pnpm -C DIR test, pnpm --dir DIR test | Honor project directory |
| pnpm exec TOOL | Detect nested tool |
| pnpm dlx TOOL | Detect nested tool |
| pnpx TOOL | Detect nested tool |
| yarn test, yarn run test | Filter test output |
| yarn lint, yarn typecheck, yarn build | Filter corresponding family |
| yarn workspace PKG test | Selected workspace |
| yarn workspaces foreach | Multi-workspace filtering |
| bun test | Filter Bun test records |
| bun run test, lint, typecheck, build | Resolve package script |
| bunx TOOL | Detect nested tool |
| npx TOOL | Detect nested tool |

### 8.2 JavaScript dan TypeScript tools

Test runners:

    vitest
    vitest run
    jest
    playwright test
    cypress run
    mocha
    ava
    tap

Lint dan typecheck:

    eslint
    biome check
    biome lint
    oxlint
    stylelint
    tsc
    tsc --noEmit
    vue-tsc
    svelte-check
    flow

Build:

    vite build
    next build
    nuxt build
    webpack
    rollup
    esbuild
    tsup
    swc

Format check:

    prettier --check
    biome format
    dprint check

Filter mempertahankan failed tests, assertion diff, snapshot diff, stack trace, warning, dan final summary.

### 8.3 JavaScript monorepo runners

Supported:

    turbo run test
    turbo run lint
    turbo run typecheck
    turbo run build
    nx test
    nx run PROJECT:test
    nx run-many -t test
    nx affected -t test
    lerna run test
    lage test
    moon run :test
    npm, pnpm, dan yarn workspace commands

Recognized files:

    package.json
    pnpm-workspace.yaml
    turbo.json
    nx.json
    lerna.json
    yarn.lock
    pnpm-lock.yaml

Original root command tetap dijalankan. Manifest hanya memilih filter families.

### 8.4 Rust

Supported:

    cargo test
    cargo test --workspace
    cargo test -p PACKAGE
    cargo nextest run
    cargo build
    cargo build --workspace
    cargo check
    cargo clippy
    cargo fmt --check
    cargo doc

Filter:

- compact passing tests;
- compact Compiling dan Checking progress;
- retain compiler diagnostic lengkap;
- retain failed-test output;
- retain warning dan final summary.

### 8.5 Python

Wrapper:

    python
    python3
    uv run
    poetry run
    pipenv run

Filtered tools:

    python -m pytest
    python3 -m pytest
    pytest
    python -m unittest
    tox
    nox
    ruff check
    mypy
    pyright
    pylint
    black --check
    coverage run
    coverage report
    pip install
    uv pip install
    poetry install

Generic python app.py tetap dibungkus TTC. Jika output tidak dikenali, output diteruskan penuh.

### 8.6 Go

Supported:

    go test
    go test ./...
    go test -v ./...
    go test -json ./...
    go build ./...
    go vet ./...
    go generate ./...
    golangci-lint run
    staticcheck ./...

Recognized files:

    go.mod
    go.sum
    go.work

Filter mempertahankan failed test output, panic, race detector, vet diagnostic, build error, dan package summary.

### 8.7 PHP

Supported:

    phpunit
    vendor/bin/phpunit
    pest
    vendor/bin/pest
    php artisan test
    phpstan analyse
    psalm
    phpcs
    php-cs-fixer fix --dry-run
    composer test
    composer run test
    composer install
    composer update

Generic php script.php tetap raw bila output tidak dikenali.

### 8.8 Java dan JVM

Supported:

    mvn test
    mvn verify
    mvn package
    mvn install
    ./mvnw test
    gradle test
    gradle build
    gradle check
    ./gradlew test
    ./gradlew build
    javac
    JUnit console runner

Filter Maven download progress, Gradle task progress, dan passing JUnit records. Compilation error, failed tests, stack trace, warning, dan build summary dipertahankan.

### 8.9 .NET

Supported:

    dotnet test
    dotnet test SOLUTION
    dotnet build
    dotnet restore
    dotnet publish
    dotnet format --verify-no-changes

Filter passing test dan restore/build progress. Compiler diagnostic code, failed test output, stack trace, warning, dan summary dipertahankan.

### 8.10 C, C++, Swift, Ruby, dan build tools

Supported:

    cmake --build
    ctest
    ninja
    make
    make test
    make check
    swift build
    swift test
    bundle exec rspec
    rspec
    rubocop
    rake test

Filter hanya progress dan passing records yang dikenal. Dynamic targets raw jika family tidak dapat ditentukan.

### 8.11 Container dan infrastructure

Filtering terbatas:

    docker build
    docker compose build
    podman build
    terraform validate
    helm lint

Plan, diff, resource changes, warning, error, dan security output selalu dipertahankan.

Interactive docker compose up dan kubectl logs -f raw streaming.

### 8.12 Commands yang selalu raw

Walaupun dibungkus TTC, output berikut raw pada MVP:

    cat
    bat
    sed
    awk
    head
    tail
    less
    more
    file
    strings
    base64
    xxd
    hexdump
    env
    printenv
    echo
    printf
    curl
    wget
    ssh
    scp
    database clients
    application-specific CLIs
    git diff
    git show

## 9. Monorepo requirements

### 9.1 Original command authoritative

TTC tidak menjalankan package satu per satu. TTC menjalankan root command asli:

    pnpm test
    turbo run test
    nx run-many -t test
    cargo test --workspace
    go test ./...

Dependency graph, cache, concurrency, dan environment runner tetap berlaku.

### 9.2 Recursive script discovery

TTC membaca manifest secara statis:

- package.json scripts;
- npm/pnpm/yarn/bun lifecycle scripts;
- nested script aliases;
- turbo.json;
- nx.json;
- pnpm-workspace.yaml;
- Cargo workspace;
- go.work;
- Composer scripts;
- Gradle/Maven project files.

Batas:

- recursion depth maksimum 16;
- cycle detection;
- parsing gagal berarti raw;
- discovery tidak menjalankan script.

### 9.3 Mixed-language monorepo

Contoh root package.json:

    {
      "scripts": {
        "test": "turbo run test && go test ./..."
      }
    }

Ketika Codex menjalankan pnpm test, TTC menjalankan pnpm test sekali dan mengaktifkan filter:

- JavaScript test;
- Turbo workspace;
- Go test.

Output JavaScript dan Go dapat dikompaksi dalam satu invocation. Error dari ecosystem mana pun tetap terlihat.

### 9.4 Output-signature fallback

TTC juga mendeteksi signature output saat streaming karena runner tidak selalu mengekspos child command:

- Rust compiler/test;
- Vitest/Jest;
- Go test;
- pytest;
- Maven/Gradle;
- dotnet;
- PHP test tools.

Signature detection baru aktif setelah beberapa record konsisten. Record sebelum confidence tercapai tetap diteruskan.

## 10. Streaming architecture

MVP:

- spawn satu child;
- satu reader untuk stdout;
- satu reader untuk stderr;
- bounded channel;
- line framing per stream;
- pending line maksimum 1 MiB;
- non-UTF-8 langsung raw;
- tidak menyimpan seluruh output di memory;
- tidak memakai SQLite;
- tidak memakai background service.

Urutan internal masing-masing stream harus tetap sama.

Binary di `~/.local/bin/ttc` harus readable dan executable ketika command
dijalankan dalam sandbox Codex read-only, workspace-write, maupun
danger-full-access. TTC tidak menambah writable root atau memperlebar permission
command asli.

## 11. Raw output retrieval

Raw output menggunakan file biasa, bukan database. Command `ttc raw` hanya
membaca hasil capture; command ini bukan bypass filtering dan tidak menjalankan
ulang command asli.

- lokasi default: XDG_STATE_HOME/ttc/runs;
- fallback jika lokasi default tidak writable: direktori temporary per-user
  dengan permission 0700;
- satu ID random per invocation;
- permission user-only;
- maksimum 32 MiB total per invocation;
- jika batas terlampaui, pertahankan bagian terbaru dan catat jumlah byte yang
  dibuang;
- simpan hanya jika ada byte yang benar-benar dikompaksi;
- failure tanpa kompaksi tidak disimpan dan tetap byte-exact;
- untuk invocation yang dikompaksi, capture berisi original stdout/stderr sebelum
  filtering agar output asli dapat direplay, subject to batas 32 MiB;
- simpan event ber-tag stream agar stdout dan stderr dapat direplay dalam urutan
  yang diamati TTC;
- hapus file lebih lama dari 24 jam saat invocation berikutnya;
- jika storage default dan fallback sama-sama gagal, TTC beralih ke raw output
  tanpa menjalankan ulang command.

Output:

    raw: ttc raw ID

Summary kompaksi dan hint raw ditulis ke stderr. Jika tidak ada kompaksi, TTC
tidak menambahkan metadata apa pun.

Commands:

    ttc raw ID
    ttc raw ID --stdout
    ttc raw ID --stderr
    ttc raw ID --tail 100

Tanpa selector, `ttc raw ID` mereplay stdout dan stderr dalam urutan event yang
diamati. Selector `--stdout` dan `--stderr` mengekstrak stream masing-masing.
Command mencari ID pada lokasi XDG maupun fallback temporary.

## 12. Minimal configuration

File:

    ~/.config/ttc/config.toml

Default:

    max_raw_mb = 32
    retention_hours = 24

Tidak ada:

- compatibility profile;
- automatic rewrite flag;
- metrics;
- telemetry;
- remote config;
- project policy lattice.

Tidak ada environment variable atau subcommand untuk melewati filtering.
Untuk menonaktifkan integrasi, pengguna menjalankan `ttc uninstall codex`.

## 13. Distribution dan versioning

Source distribusi publik adalah GitHub repository `skirja/ttc-ai`.

Rilis pertama:

- versi package dan binary: `0.1.0`;
- tag: `v0.1.0`;
- target: `x86_64-unknown-linux-gnu`;
- asset executable langsung bernama `ttc-x86_64-unknown-linux-gnu`;
- checksum `SHA256SUMS`;
- `install.sh` sebagai jalur install utama.

Tag `vX.Y.Z` hanya boleh dibuat setelah seluruh acceptance binary dan Codex
lulus. Tag menjalankan ulang CI, memverifikasi versi tag sama dengan versi
package, membangun dan smoke-test release binary, membuat checksum, lalu
memublikasikan GitHub Release otomatis.

Installer utama:

    curl --proto '=https' --tlsv1.2 -LsSf \
      https://github.com/skirja/ttc-ai/releases/latest/download/install.sh | sh

`install.sh`:

- selalu memasang stable release terbaru;
- mendukung Linux x86_64 pada MVP dan menolak platform lain dengan jelas;
- mengunduh executable dan checksum dari GitHub Release;
- memverifikasi SHA-256 sebelum install;
- membuat `~/.local/bin` bila belum ada;
- memasang binary secara atomik sebagai `~/.local/bin/ttc` dengan permission
  executable;
- mencatat version, checksum, installed path, PATH ownership, dan integrasi
  harness aktif pada metadata user di `XDG_DATA_HOME/ttc/install.toml` dengan
  default `~/.local/share/ttc/install.toml`;
- menolak menimpa `~/.local/bin/ttc` yang tidak terbukti dimiliki TTC;
- idempotent untuk versi yang sama dan mengganti versi lama dengan versi latest;
- tidak menjalankan `ttc install codex` secara otomatis;
- jika `~/.local/bin` belum ada di PATH Bash, membuat backup lalu menambahkan
  managed block idempotent ke `~/.bashrc`;
- mencetak instruksi `source` atau membuka shell baru karena installer tidak
  dapat mengubah environment parent shell;
- mencetak `~/.local/bin/ttc install codex` sebagai next action yang langsung
  dapat dipakai sebelum PATH direload;
- jika update shell config gagal, binary tetap terpasang dan installer mencetak
  instruksi PATH manual.

Pengguna manual dapat mengunduh executable dan checksum yang sama, menjalankan
verifikasi, memberi permission dengan `chmod +x`, lalu memindahkan binary ke
path pilihannya. Jalur manual di luar `~/.local/bin/ttc` tidak didaftarkan oleh
`ttc install codex` pada MVP.

Update dilakukan dengan menjalankan ulang installer latest. Tidak ada
`ttc update`, version selector, atau network access dari binary TTC pada MVP.

CI utama berjalan pada branch `master`. Artifact release-mode Phase 1 harus
lulus sebelum implementasi hook Codex dimulai. Public release hanya dibuat
setelah Phase 2 lulus.

## 14. Test plan

### 14.1 Unit tests

- POSIX quoting round-trip;
- recursive wrapper prevention;
- command classifier;
- output-signature classifier;
- every filter retains warning/error/failure;
- unknown output byte-exact;
- non-UTF-8 passthrough;
- long-line passthrough;
- mixed stdout/stderr;
- parser failure becomes raw.
- raw event replay dan stream selector;
- 32 MiB tail retention dan dropped-byte metadata;
- install manifest ownership dan SemVer consistency.

### 14.2 Execution tests

- original command runs exactly once;
- success exit code preserved;
- exit 7 preserved;
- SIGINT dan SIGTERM propagated;
- stdin inherited;
- cwd inherited;
- environment inherited;
- shell operators execute correctly;
- watch/dev raw streaming;
- XDG denial memakai temporary user-only storage;
- seluruh storage failure switches to raw without rerun;
- read-only, workspace-write, dan danger-full-access tidak mengubah permission
  command asli.

### 14.3 Ecosystem fixtures

Success, failure, warning, dan large fixture minimum untuk:

- npm/pnpm/yarn/bun dan Vitest/Jest;
- Turbo/Nx;
- Cargo;
- Python/pytest;
- Go;
- PHP;
- Maven/Gradle;
- dotnet.

Tambahkan fixture untuk C/C++ build tools, Swift, Ruby, container, dan
infrastructure. Large fixture minimum 1.000 passing atau progress records dan
harus mengurangi byte output minimum 80% tanpa kehilangan warning, error,
failure, diagnostic, atau final summary.

Setiap ecosystem family memiliki minimal satu smoke E2E memakai tool asli.
Versi tool dipin pada CI agar hasil reproducible.

### 14.4 Monorepo E2E

Required fixtures:

1. pnpm workspace JavaScript;
2. Turbo monorepo test/lint/typecheck;
3. Nx monorepo;
4. Cargo workspace;
5. Go workspace;
6. mixed JavaScript dan Go root script;
7. nested package script memanggil Python atau Rust.

Assertions:

- root command dijalankan sekali;
- seluruh package yang seharusnya berjalan tetap berjalan;
- passing/progress records berkurang;
- error package mana pun terlihat;
- exit code sama dengan baseline.

### 14.5 Distribution E2E

- tag dan package version mismatch gagal;
- artifact Linux dapat dijalankan tanpa source tree;
- checksum valid dan checksum mismatch ditolak;
- fresh install, reinstall same version, dan update berjalan atomik;
- PATH block Bash idempotent dan fallback instruction benar;
- `ttc uninstall` menolak saat integrasi harness aktif;
- global uninstall tidak menghapus PATH bila `~/.local/bin` dipakai program lain.

### 14.6 Codex hook E2E

Gunakan Codex CLI terpasang, bukan serializer tiruan.
Minimum version test adalah 0.154.0. Test lokal memakai login yang sudah ada,
release-mode TTC binary, temporary fixture workspace, dan `codex exec
--ephemeral`. Config dan artifact test harus dipulihkan atau dihapus setelah
setiap run.

Cases:

1. npm run test success — output model-facing lebih pendek;
2. npm run test failure — error lengkap dan non-zero;
3. pnpm test monorepo — output package dikompaksi;
4. mixed JS dan Go — kedua family terfilter;
5. cargo test — passing records dikompaksi;
6. cat README.md — output identik baseline;
7. npm run dev — raw streaming;
8. quote dan shell operator — command sekali;
9. command TTC — tidak recursive.
10. executable TTC dapat dipanggil pada read-only, workspace-write, dan
    danger-full-access tanpa memperlebar permission command asli.

Ukur bytes dan token pada request model berikutnya, bukan hanya terminal output.
Large filterable output wajib berkurang minimum 80% secara byte dan token harus
lebih rendah daripada baseline.

## 15. Definition of Done

Phase 1 selesai jika:

- binary release berhasil dibangun;
- seluruh test dan lint lulus;
- command asli selalu dijalankan sekali;
- exit, signal, cwd, dan environment benar;
- seluruh ecosystem, monorepo fixtures, dan pinned real-tool smoke tests lulus;
- unknown command raw byte-exact;
- setiap large fixture mengurangi byte minimum 80%;
- release-mode `x86_64-unknown-linux-gnu` artifact dan installer lulus CI pada
  branch `master`.

Phase 2 selesai jika:

- install hook idempotent;
- setiap Bash command dibungkus TTC;
- tidak ada recursive wrapper;
- Codex CLI menjalankan wrapper;
- npm run test, pnpm test, cargo test, dan mixed monorepo menghasilkan output model-facing lebih pendek;
- cat dan unknown command identik baseline;
- failure dan exit status identik baseline;
- seluruh sandbox mode lulus;
- test lokal ephemeral tidak menyisakan session, config, atau fixture;
- install langsung aktif tanpa compatibility profile atau hidden flag;
- tag `v0.1.0` menghasilkan public GitHub Release dengan executable, checksum,
  dan installer.

## 16. Non-goals

MVP tidak mencoba:

- memodifikasi source Codex;
- membuat protocol Codex baru;
- compatibility matrix per versi;
- permission profiles;
- PowerShell atau Cmd;
- arbitrary shell analysis sempurna;
- meringkas file/source content;
- dashboard metrics;
- LLM filtering;
- mengganti execution logic Turbo/Nx/package manager;
- full RTK feature parity pada rilis pertama.

## 17. Repository layout

Gunakan satu Rust binary crate dengan modul berdasarkan tanggung jawab: CLI,
invocation/execution, classification, filter families, raw storage, harness
adapter, dan installer integration. Test integration dan fixture dipisahkan dari
source production. Jangan membuat workspace multi-crate, daemon, database, atau
abstraksi harness generik di luar interface minimum yang dibutuhkan Codex.

Struktur boleh berkembang saat implementasi selama dependency tetap satu arah,
filter family dapat diuji terpisah, dan execution engine tidak bergantung pada
harness tertentu.

## 18. Final behavior

Aturan TTC:

> Hook selalu membungkus Bash command. TTC hanya memfilter output yang benar-benar dikenali. Semua output lain diteruskan penuh.

Jika hook terpasang, Bash command masuk melalui TTC. Jika TTC tidak dapat mengoptimalkan output, pengguna tetap menerima output asli.
