# Audit coverage standalone M9

Laporan dihasilkan oleh `python3 scripts/check-spec-coverage.py`; mapping yang
direview ada pada `ai_docs/m9-coverage.json`. Setiap section, command/form,
dan kelompok klausul SPEC memiliki pemetaan source/test/gate. Digest mengunci
SPEC yang diaudit; perubahan kontrak memerlukan audit ulang. Gate ini mengecek
kelengkapan pemetaan, bukan membuktikan semantik implementasi dengan pencarian
string. Bukti perilaku berasal dari test dan pinned real-tool smoke pada run
yang dicatat di TODO serta artifact kandidat yang sama.

SPEC SHA-256: `4dd1b53b557b5c36962fdb7ec3bae8ed3547522bd01e77cbb316cb5633fb9534`.

## Hasil audit dan batas penerimaan

Core execution/filter M1–M8 dipertahankan. Gap distribusi ditutup oleh
finalizer lokal, installer checksum, transaksi ownership, PATH backup/fallback,
uninstall, tag policy dan smoke ELF di luar source. Pertukaran atomik config
mempertahankan file aktual yang tergeser; konflik sesudah validasi terakhir
menyimpan kedua versi dan marker recovery, termasuk pada rollback/uninstall.
Feature fs dari nix terpin menyediakan renameat2 tanpa unsafe code di TTC. SHA-256 memakai dependency produksi
`sha2 = =0.10.9` agar validasi ownership tidak mengeksekusi binary existing
atau bergantung utility dari PATH. Seluruh failure installer menggunakan
HOME/XDG temporary; mock HTTP loopback memerlukan opt-in eksplisit.

M10 hanya mencakup kontrak harness, auth E2E/sandbox/token dan publikasi;
tidak ada command Codex yang diiklankan pada M9. Final acceptance M9 tetap
memerlukan clean successful master run dan artifact yang diunduh sesudah
merge pengguna. Laporan ini tidak menyatakan gate master telah lulus.

## 1. Tujuan

Standalone sekali; default retain, tanpa daemon/network/harness dependency. Artifact GNU mendahului M10.

Implementasi: `src/main.rs`, `src/core/execution.rs`, `src/core/streaming.rs`, `Cargo.toml`.

Test/verifikasi: `tests/execution.rs`, `tests/passthrough.rs`, `tests/filter_safety.rs`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m9-distribution`.

Batas tahap: Hook semua Bash, kontrak permission resmi, minimum Codex dan penggunaan full-access dibuktikan pada M10.

Klausul yang dipetakan (8 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 596c09eafeb6</summary>

```text
TTC adalah wrapper command lokal untuk mengurangi output Bash yang masuk ke konteks Codex.
```

</details>

<details><summary>Klausul 2, digest cd804ee830c9</summary>

```text
Hook Codex selalu membungkus setiap Bash command dengan TTC. TTC kemudian memilih salah satu perilaku:
```

</details>

<details><summary>Klausul 3, digest b17165337a39</summary>

```text
1. output dikenali sebagai log test, build, lint, typecheck, atau tooling yang aman diringkas — filter;
2. output tidak dikenali atau tidak aman diringkas — teruskan output asli tanpa perubahan.
```

</details>

<details><summary>Klausul 4, digest 988738e63726</summary>

```text
Contoh filter:
```

</details>

<details><summary>Klausul 5, digest f75600dde7de</summary>

```text
    npm run test
        ↓ hook
    ttc 'npm run test'
        ↓ TTC
    output test yang repetitif dikompaksi
```

</details>

<details><summary>Klausul 6, digest 7529293055a5</summary>

```text
Contoh passthrough:
```

</details>

<details><summary>Klausul 7, digest 286238aa4176</summary>

```text
    cat README.md
        ↓ hook
    ttc 'cat README.md'
        ↓ TTC
    seluruh isi README.md diteruskan apa adanya
```

</details>

<details><summary>Klausul 8, digest fe8e007378cd</summary>

```text
Command asli selalu dijalankan tepat satu kali. TTC tidak mengganti package manager, test runner, atau monorepo runner dengan program lain.
```

</details>

## 2. Keputusan produk

Standalone sekali; default retain, tanpa daemon/network/harness dependency. Artifact GNU mendahului M10.

Implementasi: `src/main.rs`, `src/core/execution.rs`, `src/core/streaming.rs`, `Cargo.toml`.

Test/verifikasi: `tests/execution.rs`, `tests/passthrough.rs`, `tests/filter_safety.rs`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m9-distribution`.

Batas tahap: Hook semua Bash, kontrak permission resmi, minimum Codex dan penggunaan full-access dibuktikan pada M10.

Klausul yang dipetakan (1 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 79a96965944f</summary>

```text
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
```

</details>

## 3. Urutan implementasi

Standalone sekali; default retain, tanpa daemon/network/harness dependency. Artifact GNU mendahului M10.

Implementasi: `src/main.rs`, `src/core/execution.rs`, `src/core/streaming.rs`, `Cargo.toml`.

Test/verifikasi: `tests/execution.rs`, `tests/passthrough.rs`, `tests/filter_safety.rs`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m9-distribution`.

Batas tahap: Hook semua Bash, kontrak permission resmi, minimum Codex dan penggunaan full-access dibuktikan pada M10.

Klausul yang dipetakan (0 kelompok; seluruh bullet/command di dalamnya):

## Phase 1 — Binary TTC standalone

Standalone sekali; default retain, tanpa daemon/network/harness dependency. Artifact GNU mendahului M10.

Implementasi: `src/main.rs`, `src/core/execution.rs`, `src/core/streaming.rs`, `Cargo.toml`.

Test/verifikasi: `tests/execution.rs`, `tests/passthrough.rs`, `tests/filter_safety.rs`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m9-distribution`.

Batas tahap: Hook semua Bash, kontrak permission resmi, minimum Codex dan penggunaan full-access dibuktikan pada M10.

Klausul yang dipetakan (4 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 6a051dcdbf2a</summary>

```text
Binary harus selesai dan stabil tanpa bergantung pada Codex.
```

</details>

<details><summary>Klausul 2, digest bb819682c9e4</summary>

```text
Deliverable:
```

</details>

<details><summary>Klausul 3, digest ccaff2c24af6</summary>

```text
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
```

</details>

<details><summary>Klausul 4, digest 7b391695069d</summary>

```text
Hook Codex tidak dikerjakan sebelum acceptance Phase 1 lulus.
```

</details>

## Phase 2 — Hook Codex

Kontrak harness terdokumentasi; entrypoint hook/install/uninstall Codex tidak tersedia pada M9.

Implementasi: `src/cli.rs`.

Test/verifikasi: `tests/cli.rs`.

Evidence CI: `rust`.

Batas tahap: Seluruh kontrak hook, serializer/quoting hook, recursive wrapper, config/trust, auth E2E ephemeral, token reduction, sandbox Codex dan publikasi menunggu M10. Global uninstall sudah dipetakan pada §13.

Klausul yang dipetakan (2 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 9e77a02aa165</summary>

```text
Setelah Phase 1 lulus:
```

</details>

<details><summary>Klausul 2, digest d30324c24006</summary>

```text
- tambahkan ttc hook codex;
- tambahkan ttc install codex;
- tambahkan ttc uninstall codex;
- hook selalu membungkus semua Bash command;
- test memakai Codex CLI yang benar-benar terpasang;
- buktikan output model-facing lebih pendek untuk command filterable;
- buktikan output command passthrough identik dengan baseline;
- publikasikan GitHub Release hanya setelah acceptance Phase 2 lulus.
```

</details>

## 4. CLI contract

Argv asli atau tepat satu shell string; stdin/cwd/env/TTY diwariskan, per-stream bytes/order, exit/signal dipertahankan, tanpa rerun.

Implementasi: `src/cli.rs`, `src/core/execution.rs`, `src/core/streaming.rs`.

Test/verifikasi: `tests/cli.rs`, `tests/execution.rs`, `tests/passthrough.rs`, `tests/signals.rs`, `tests/shell_contract.rs`, `tests/storage_failure.rs`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m9-distribution`.

Batas tahap: Command install/uninstall/hook Codex dalam daftar CLI baru diimplementasikan M10; help M9 tidak mengiklankannya.

Klausul yang dipetakan (0 kelompok; seluruh bullet/command di dalamnya):

## 4.1 Direct argv

Argv asli atau tepat satu shell string; stdin/cwd/env/TTY diwariskan, per-stream bytes/order, exit/signal dipertahankan, tanpa rerun.

Implementasi: `src/cli.rs`, `src/core/execution.rs`, `src/core/streaming.rs`.

Test/verifikasi: `tests/cli.rs`, `tests/execution.rs`, `tests/passthrough.rs`, `tests/signals.rs`, `tests/shell_contract.rs`, `tests/storage_failure.rs`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m9-distribution`.

Batas tahap: Command install/uninstall/hook Codex dalam daftar CLI baru diimplementasikan M10; help M9 tidak mengiklankannya.

Klausul yang dipetakan (6 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest b79647cfaa19</summary>

```text
Bentuk argv yang didukung binary dan digunakan oleh test/integrasi:
```

</details>

<details><summary>Klausul 2, digest 1493eac4f1b6</summary>

```text
    ttc npm run test
    ttc pnpm test
    ttc cargo test
    ttc go test ./...
    ttc python -m pytest
    ttc dotnet test
```

</details>

<details><summary>Klausul 3, digest e68b82ccde00</summary>

```text
Semua argument setelah ttc diteruskan sebagai argv asli tanpa shell tambahan.
```

</details>

<details><summary>Klausul 4, digest a8e902dbed5a</summary>

```text
Package script tetap menangkap seluruh child process. Contoh:
```

</details>

<details><summary>Klausul 5, digest 6c979c40ac79</summary>

```text
    ttc npm run check
```

</details>

<details><summary>Klausul 6, digest f3974c5fe369</summary>

```text
Jika script check berisi `npx tsc --noEmit && npm run lint`, npm menjalankan
compound script tersebut sebagai child. Seluruh stdout/stderr tetap mengalir
melalui TTC dan dapat memakai filter TypeScript serta lint sekaligus.
```

</details>

## 4.2 Single-string shell command

Argv asli atau tepat satu shell string; stdin/cwd/env/TTY diwariskan, per-stream bytes/order, exit/signal dipertahankan, tanpa rerun.

Implementasi: `src/cli.rs`, `src/core/execution.rs`, `src/core/streaming.rs`.

Test/verifikasi: `tests/cli.rs`, `tests/execution.rs`, `tests/passthrough.rs`, `tests/signals.rs`, `tests/shell_contract.rs`, `tests/storage_failure.rs`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m9-distribution`.

Batas tahap: Command install/uninstall/hook Codex dalam daftar CLI baru diimplementasikan M10; help M9 tidak mengiklankannya.

Klausul yang dipetakan (9 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest d87226e9beae</summary>

```text
Digunakan oleh hook:
```

</details>

<details><summary>Klausul 2, digest 3f7d2ead598e</summary>

```text
    ttc 'npm run test'
```

</details>

<details><summary>Klausul 3, digest f69334c9bda0</summary>

```text
Jika TTC menerima tepat satu argument yang berisi command, TTC menjalankan
argument tersebut melalui shell native:
```

</details>

<details><summary>Klausul 4, digest 4a845c3a7a68</summary>

```text
    ttc 'npx tsc --noEmit && npm run lint'
    ttc 'FOO=bar npm test'
    ttc 'cd packages/api && npm test'
    ttc 'pnpm test && go test ./...'
```

</details>

<details><summary>Klausul 5, digest 6a8f4e82a496</summary>

```text
Build Linux/macOS memakai shell POSIX. Build Windows nantinya memakai shell
native yang dipilih untuk platform tersebut. Hook bertanggung jawab melakukan
quoting sesuai platform sehingga seluruh command tetap satu argument.
```

</details>

<details><summary>Klausul 6, digest 0866c93280ea</summary>

```text
Commands minimum:
```

</details>

<details><summary>Klausul 7, digest a93690e93325</summary>

```text
    ttc <program> [args...]
    ttc '<complete shell command>'
    ttc raw <id>
    ttc install codex
    ttc uninstall codex
    ttc uninstall
```

</details>

<details><summary>Klausul 8, digest b5c28ff38c83</summary>

```text
`ttc hook codex` adalah entrypoint internal yang dipasang ke config Codex dan
tidak ditampilkan sebagai command utama pada help. Namespace internal hook
boleh ditambah untuk harness lain tanpa mengubah execution atau filter engine.
```

</details>

<details><summary>Klausul 9, digest 4a31b1918bd3</summary>

```text
`ttc --version` membaca versi SemVer yang sama dengan package Rust. `ttc --help`
menampilkan command publik, tetapi tidak ada `explain`, `doctor`, atau
self-update pada MVP.
```

</details>

## 4.3 Exit behavior

Argv asli atau tepat satu shell string; stdin/cwd/env/TTY diwariskan, per-stream bytes/order, exit/signal dipertahankan, tanpa rerun.

Implementasi: `src/cli.rs`, `src/core/execution.rs`, `src/core/streaming.rs`.

Test/verifikasi: `tests/cli.rs`, `tests/execution.rs`, `tests/passthrough.rs`, `tests/signals.rs`, `tests/shell_contract.rs`, `tests/storage_failure.rs`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m9-distribution`.

Batas tahap: Command install/uninstall/hook Codex dalam daftar CLI baru diimplementasikan M10; help M9 tidak mengiklankannya.

Klausul yang dipetakan (1 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 53742f95a0c9</summary>

```text
- Exit code child menjadi exit code TTC.
- Jika child mati karena signal, TTC meneruskan signal yang sama.
- TTC tidak mengubah failure menjadi success.
- TTC tidak menjalankan ulang command saat filtering gagal.
- Error internal setelah child dimulai membuat output berikutnya raw, bukan rerun.
```

</details>

## 5. Hook Codex

Kontrak harness terdokumentasi; entrypoint hook/install/uninstall Codex tidak tersedia pada M9.

Implementasi: `src/cli.rs`.

Test/verifikasi: `tests/cli.rs`.

Evidence CI: `rust`.

Batas tahap: Seluruh kontrak hook, serializer/quoting hook, recursive wrapper, config/trust, auth E2E ephemeral, token reduction, sandbox Codex dan publikasi menunggu M10. Global uninstall sudah dipetakan pada §13.

Klausul yang dipetakan (0 kelompok; seluruh bullet/command di dalamnya):

## 5.1 Input minimum

Kontrak harness terdokumentasi; entrypoint hook/install/uninstall Codex tidak tersedia pada M9.

Implementasi: `src/cli.rs`.

Test/verifikasi: `tests/cli.rs`.

Evidence CI: `rust`.

Batas tahap: Seluruh kontrak hook, serializer/quoting hook, recursive wrapper, config/trust, auth E2E ephemeral, token reduction, sandbox Codex dan publikasi menunggu M10. Global uninstall sudah dipetakan pada §13.

Klausul yang dipetakan (3 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 1a84fc75ad3a</summary>

```text
Hook membaca satu object JSON PreToolUse dari stdin:
```

</details>

<details><summary>Klausul 2, digest 385cd30a7e3f</summary>

```text
    {
      "hook_event_name": "PreToolUse",
      "tool_name": "Bash",
      "cwd": "/absolute/project/path",
      "tool_input": {
        "command": "npm run test"
      }
    }
```

</details>

<details><summary>Klausul 3, digest 44b404ae5d03</summary>

```text
Field tambahan diabaikan.
```

</details>

## 5.2 Output

Kontrak harness terdokumentasi; entrypoint hook/install/uninstall Codex tidak tersedia pada M9.

Implementasi: `src/cli.rs`.

Test/verifikasi: `tests/cli.rs`.

Evidence CI: `rust`.

Batas tahap: Seluruh kontrak hook, serializer/quoting hook, recursive wrapper, config/trust, auth E2E ephemeral, token reduction, sandbox Codex dan publikasi menunggu M10. Global uninstall sudah dipetakan pada §13.

Klausul yang dipetakan (3 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 46eb564cc9f5</summary>

```text
Untuk setiap Bash command selain recursive TTC wrapper:
```

</details>

<details><summary>Klausul 2, digest a7027133f124</summary>

```text
    {
      "hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "allow",
        "updatedInput": {
          "command": "'/absolute/path/to/ttc' 'npm run test'"
        }
      }
    }
```

</details>

<details><summary>Klausul 3, digest 5a16d53b1829</summary>

```text
Hook selalu menghasilkan wrapper. Hook tidak memeriksa apakah command filterable.
```

</details>

## 5.3 Quoting

Kontrak harness terdokumentasi; entrypoint hook/install/uninstall Codex tidak tersedia pada M9.

Implementasi: `src/cli.rs`.

Test/verifikasi: `tests/cli.rs`.

Evidence CI: `rust`.

Batas tahap: Seluruh kontrak hook, serializer/quoting hook, recursive wrapper, config/trust, auth E2E ephemeral, token reduction, sandbox Codex dan publikasi menunggu M10. Global uninstall sudah dipetakan pada §13.

Klausul yang dipetakan (3 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 4704f449ffba</summary>

```text
POSIX quoting wajib menangani:
```

</details>

<details><summary>Klausul 2, digest 949bc5294de8</summary>

```text
- spasi;
- single quote;
- double quote;
- dollar sign;
- newline;
- Unicode;
- shell operators;
- command substitution;
- environment assignment.
```

</details>

<details><summary>Klausul 3, digest 69f7e8950ef5</summary>

```text
Original command harus menjadi satu argument setelah executable TTC. Hook tidak
menambahkan option lain di antara TTC dan command asli.
```

</details>

## 5.4 Recursive wrapper prevention

Kontrak harness terdokumentasi; entrypoint hook/install/uninstall Codex tidak tersedia pada M9.

Implementasi: `src/cli.rs`.

Test/verifikasi: `tests/cli.rs`.

Evidence CI: `rust`.

Batas tahap: Seluruh kontrak hook, serializer/quoting hook, recursive wrapper, config/trust, auth E2E ephemeral, token reduction, sandbox Codex dan publikasi menunggu M10. Global uninstall sudah dipetakan pada §13.

Klausul yang dipetakan (3 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 280aedb9edc0</summary>

```text
Jika command sudah memanggil binary TTC, hook mengembalikan object kosong.
```

</details>

<details><summary>Klausul 2, digest c9df2ff1eded</summary>

```text
Contoh yang tidak dibungkus ulang:
```

</details>

<details><summary>Klausul 3, digest 762f401a864b</summary>

```text
    ttc npm test
    /home/user/.local/bin/ttc 'cargo test'
```

</details>

## 5.5 Install dan uninstall

Kontrak harness terdokumentasi; entrypoint hook/install/uninstall Codex tidak tersedia pada M9.

Implementasi: `src/cli.rs`.

Test/verifikasi: `tests/cli.rs`.

Evidence CI: `rust`.

Batas tahap: Seluruh kontrak hook, serializer/quoting hook, recursive wrapper, config/trust, auth E2E ephemeral, token reduction, sandbox Codex dan publikasi menunggu M10. Global uninstall sudah dipetakan pada §13.

Klausul yang dipetakan (5 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 0c84443daa3d</summary>

```text
ttc install codex:
```

</details>

<details><summary>Klausul 2, digest acf9c3dec977</summary>

```text
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
```

</details>

<details><summary>Klausul 3, digest 8bc6f9d4e197</summary>

```text
`ttc uninstall codex` hanya menghapus hook Codex yang dimiliki TTC. Binary,
integrasi harness lain, setting unrelated, dan perubahan config setelah install
tetap dipertahankan.
```

</details>

<details><summary>Klausul 4, digest 456d21e28cae</summary>

```text
`ttc uninstall` menghapus instalasi global TTC hanya jika tidak ada integrasi
harness aktif. Command ini menghapus binary dan metadata milik TTC. Managed PATH
entry dipertahankan jika `~/.local/bin` berisi program lain agar uninstall TTC
tidak merusak tool lain.
```

</details>

<details><summary>Klausul 5, digest 1a580965d031</summary>

```text
Tidak ada kondisi hook terlihat aktif tetapi diam-diam tidak wrapping. Jika hook terpasang dan trusted, setiap Bash command masuk melalui TTC.
```

</details>

## 6. Pipeline binary TTC

Argv asli atau tepat satu shell string; stdin/cwd/env/TTY diwariskan, per-stream bytes/order, exit/signal dipertahankan, tanpa rerun.

Implementasi: `src/cli.rs`, `src/core/execution.rs`, `src/core/streaming.rs`.

Test/verifikasi: `tests/cli.rs`, `tests/execution.rs`, `tests/passthrough.rs`, `tests/signals.rs`, `tests/shell_contract.rs`, `tests/storage_failure.rs`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m9-distribution`.

Batas tahap: Command install/uninstall/hook Codex dalam daftar CLI baru diimplementasikan M10; help M9 tidak mengiklankannya.

Klausul yang dipetakan (2 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest d284fecc996e</summary>

```text
Invocation
      → spawn original command once
      → read stdout dan stderr concurrently
      → classify command dan output signatures
      → known safe record: compact
      → unknown record: emit exactly
      → emit compact summary jika diperlukan
      → return original exit code atau signal
```

</details>

<details><summary>Klausul 2, digest fe867beff8c4</summary>

```text
Klasifikasi hanya menentukan filtering, bukan permission atau eksekusi.
```

</details>

## 7. Filter policy

Recognizer grammar spesifik + confidence; diagnostic/unknown/structured/binary/long-line raw. Watch/dev/TTY inherited. Summary/hint hanya bila compact. Go JSON exception memakai input event bytes.

Implementasi: `src/core/classification.rs`, `src/core/streaming.rs`, `src/core/filters/mod.rs`, `src/core/filters/ecosystems.rs`.

Test/verifikasi: `tests/filter_safety.rs`, `tests/passthrough.rs`, `tests/streaming.rs`, `tests/raw_command_matrix.rs`, `tests/go_fixtures.rs`, `tests/reduction.rs`, `tests/shell_contract.rs`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m4-javascript`, `m5-core-ecosystems`, `m8-remaining-ecosystems`.

Klausul yang dipetakan (0 kelompok; seluruh bullet/command di dalamnya):

## 7.1 Default retain

Recognizer grammar spesifik + confidence; diagnostic/unknown/structured/binary/long-line raw. Watch/dev/TTY inherited. Summary/hint hanya bila compact. Go JSON exception memakai input event bytes.

Implementasi: `src/core/classification.rs`, `src/core/streaming.rs`, `src/core/filters/mod.rs`, `src/core/filters/ecosystems.rs`.

Test/verifikasi: `tests/filter_safety.rs`, `tests/passthrough.rs`, `tests/streaming.rs`, `tests/raw_command_matrix.rs`, `tests/go_fixtures.rs`, `tests/reduction.rs`, `tests/shell_contract.rs`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m4-javascript`, `m5-core-ecosystems`, `m8-remaining-ecosystems`.

Klausul yang dipetakan (3 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest eec1d8b7ad27</summary>

```text
Setiap baris dipertahankan kecuali recognizer spesifik membuktikan bahwa baris tersebut hanya passing record atau progress noise.
```

</details>

<details><summary>Klausul 2, digest 93fe09e07c70</summary>

```text
Baris selalu dipertahankan bila mengandung:
```

</details>

<details><summary>Klausul 3, digest 0f86b4911428</summary>

```text
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
```

</details>

## 7.2 Unknown output

Recognizer grammar spesifik + confidence; diagnostic/unknown/structured/binary/long-line raw. Watch/dev/TTY inherited. Summary/hint hanya bila compact. Go JSON exception memakai input event bytes.

Implementasi: `src/core/classification.rs`, `src/core/streaming.rs`, `src/core/filters/mod.rs`, `src/core/filters/ecosystems.rs`.

Test/verifikasi: `tests/filter_safety.rs`, `tests/passthrough.rs`, `tests/streaming.rs`, `tests/raw_command_matrix.rs`, `tests/go_fixtures.rs`, `tests/reduction.rs`, `tests/shell_contract.rs`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m4-javascript`, `m5-core-ecosystems`, `m8-remaining-ecosystems`.

Klausul yang dipetakan (2 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest dc378e2edb4d</summary>

```text
Output unknown diteruskan penuh, termasuk:
```

</details>

<details><summary>Klausul 2, digest ed2c2b45b005</summary>

```text
    cat
    sed
    awk
    head
    tail
    custom scripts
    application-specific CLIs
    binary output
    format baru yang gagal diparse
```

</details>

## 7.3 Machine-readable output

Recognizer grammar spesifik + confidence; diagnostic/unknown/structured/binary/long-line raw. Watch/dev/TTY inherited. Summary/hint hanya bila compact. Go JSON exception memakai input event bytes.

Implementasi: `src/core/classification.rs`, `src/core/streaming.rs`, `src/core/filters/mod.rs`, `src/core/filters/ecosystems.rs`.

Test/verifikasi: `tests/filter_safety.rs`, `tests/passthrough.rs`, `tests/streaming.rs`, `tests/raw_command_matrix.rs`, `tests/go_fixtures.rs`, `tests/reduction.rs`, `tests/shell_contract.rs`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m4-javascript`, `m5-core-ecosystems`, `m8-remaining-ecosystems`.

Klausul yang dipetakan (5 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 8299844c302c</summary>

```text
JSON, JSONL, XML, YAML, SARIF, TAP, dan output exact lain diteruskan penuh kecuali ada parser lossless khusus.
```

</details>

<details><summary>Klausul 2, digest 3a5913ec169c</summary>

```text
`go test -json` adalah satu pengecualian dengan parser khusus pada §8.6.
Lifecycle event dan seluruh event selain frame passing yang diidentifikasi parser
tetap diteruskan byte-exact. JSON output lain tetap raw.
```

</details>

<details><summary>Klausul 3, digest 0711aa5d1114</summary>

```text
Flags passthrough antara lain:
```

</details>

<details><summary>Klausul 4, digest ea72b45d856b</summary>

```text
    --json
    --jsonl
    --xml
    --yaml
    --sarif
    --output
    --message-format=json
    -o
    --format
```

</details>

<details><summary>Klausul 5, digest 476432509443</summary>

```text
go test -json boleh difilter hanya menggunakan parser JSON khusus.
```

</details>

## 7.4 Watch, dev, dan interactive

Recognizer grammar spesifik + confidence; diagnostic/unknown/structured/binary/long-line raw. Watch/dev/TTY inherited. Summary/hint hanya bila compact. Go JSON exception memakai input event bytes.

Implementasi: `src/core/classification.rs`, `src/core/streaming.rs`, `src/core/filters/mod.rs`, `src/core/filters/ecosystems.rs`.

Test/verifikasi: `tests/filter_safety.rs`, `tests/passthrough.rs`, `tests/streaming.rs`, `tests/raw_command_matrix.rs`, `tests/go_fixtures.rs`, `tests/reduction.rs`, `tests/shell_contract.rs`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m4-javascript`, `m5-core-ecosystems`, `m8-remaining-ecosystems`.

Klausul yang dipetakan (3 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 037938576745</summary>

```text
Command berikut tetap dibungkus TTC tetapi memakai raw streaming:
```

</details>

<details><summary>Klausul 2, digest 0f84f55a6f8e</summary>

```text
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
```

</details>

<details><summary>Klausul 3, digest e9162a6aefa0</summary>

```text
Tidak ada buffering sampai proses selesai. stdin, signal, dan TTY behavior diteruskan.
```

</details>

## 7.5 Summary

Recognizer grammar spesifik + confidence; diagnostic/unknown/structured/binary/long-line raw. Watch/dev/TTY inherited. Summary/hint hanya bila compact. Go JSON exception memakai input event bytes.

Implementasi: `src/core/classification.rs`, `src/core/streaming.rs`, `src/core/filters/mod.rs`, `src/core/filters/ecosystems.rs`.

Test/verifikasi: `tests/filter_safety.rs`, `tests/passthrough.rs`, `tests/streaming.rs`, `tests/raw_command_matrix.rs`, `tests/go_fixtures.rs`, `tests/reduction.rs`, `tests/shell_contract.rs`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m4-javascript`, `m5-core-ecosystems`, `m8-remaining-ecosystems`.

Klausul yang dipetakan (3 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest fcb1bfad664f</summary>

```text
Jika ada output yang dikompaksi:
```

</details>

<details><summary>Klausul 2, digest cf5ef3eb63cb</summary>

```text
    TTC: 842 passing records and 136 progress records compacted
```

</details>

<details><summary>Klausul 3, digest 876f1c3c5ec1</summary>

```text
Jika tidak ada output yang dikompaksi, TTC tidak menambahkan summary.
```

</details>

## 8. Command yang harus dikenali

Package/wrapper memilih family statis. Test/pass/progress grammar compact; error/diff/stack/warning/summary retain. Format/check diagnostic-only tidak wajib menghapus byte; TAP/alternative reporter raw.

Implementasi: `src/core/classification.rs`, `src/core/filters/mod.rs`, `src/core/manifests.rs`.

Test/verifikasi: `tests/classifier.rs`, `tests/javascript_fixtures.rs`, `tests/javascript_e2e.rs`, `tests/package_scripts.rs`, `tests/filter_safety.rs`, `tests/reduction.rs`, `scripts/m4-smoke.sh`, `scripts/m6-smoke.sh`.

Evidence CI: `rust`, `m4-javascript`, `m6-package-monorepo`.

Klausul yang dipetakan (1 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest f9df6e368b6f</summary>

```text
Hook membungkus semua Bash command. Daftar berikut adalah command yang outputnya harus dikenali dan difilter.
```

</details>

## 8.1 JavaScript dan TypeScript package managers

Package/wrapper memilih family statis. Test/pass/progress grammar compact; error/diff/stack/warning/summary retain. Format/check diagnostic-only tidak wajib menghapus byte; TAP/alternative reporter raw.

Implementasi: `src/core/classification.rs`, `src/core/filters/mod.rs`, `src/core/manifests.rs`.

Test/verifikasi: `tests/classifier.rs`, `tests/javascript_fixtures.rs`, `tests/javascript_e2e.rs`, `tests/package_scripts.rs`, `tests/filter_safety.rs`, `tests/reduction.rs`, `scripts/m4-smoke.sh`, `scripts/m6-smoke.sh`.

Evidence CI: `rust`, `m4-javascript`, `m6-package-monorepo`.

Klausul yang dipetakan (1 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest fe8bacd12363</summary>

```text
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
```

</details>

## 8.2 JavaScript dan TypeScript tools

Package/wrapper memilih family statis. Test/pass/progress grammar compact; error/diff/stack/warning/summary retain. Format/check diagnostic-only tidak wajib menghapus byte; TAP/alternative reporter raw.

Implementasi: `src/core/classification.rs`, `src/core/filters/mod.rs`, `src/core/manifests.rs`.

Test/verifikasi: `tests/classifier.rs`, `tests/javascript_fixtures.rs`, `tests/javascript_e2e.rs`, `tests/package_scripts.rs`, `tests/filter_safety.rs`, `tests/reduction.rs`, `scripts/m4-smoke.sh`, `scripts/m6-smoke.sh`.

Evidence CI: `rust`, `m4-javascript`, `m6-package-monorepo`.

Klausul yang dipetakan (9 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest a5b88c042d2d</summary>

```text
Test runners:
```

</details>

<details><summary>Klausul 2, digest e9aed105f4c4</summary>

```text
    vitest
    vitest run
    jest
    playwright test
    cypress run
    mocha
    ava
    tap
```

</details>

<details><summary>Klausul 3, digest af9645a44ba5</summary>

```text
Lint dan typecheck:
```

</details>

<details><summary>Klausul 4, digest 33de15f39db0</summary>

```text
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
```

</details>

<details><summary>Klausul 5, digest f9fd8a28cf11</summary>

```text
Build:
```

</details>

<details><summary>Klausul 6, digest 48fd8d2bd20f</summary>

```text
    vite build
    next build
    nuxt build
    webpack
    rollup
    esbuild
    tsup
    swc
```

</details>

<details><summary>Klausul 7, digest d51b3638569c</summary>

```text
Format check:
```

</details>

<details><summary>Klausul 8, digest 498238dddb47</summary>

```text
    prettier --check
    biome format
    dprint check
```

</details>

<details><summary>Klausul 9, digest f113c24f6729</summary>

```text
Filter mempertahankan failed tests, assertion diff, snapshot diff, stack trace, warning, dan final summary.
```

</details>

## 8.3 JavaScript monorepo runners

Manifest lazy statis bounded; cycle/overflow/dynamic/ambiguous raw. Root runner authoritative; selector candidate dan prefix terdaftar, confidence stream/project/task/recognizer, unknown diagnostic mematikan stream. Mixed family tanpa child tambahan.

Implementasi: `src/core/manifests.rs`, `src/core/classification.rs`, `src/core/filters/mod.rs`, `src/core/filters/mod.rs`.

Test/verifikasi: `tests/manifests.rs`, `tests/package_scripts.rs`, `tests/monorepo.rs`, `tests/mixed_monorepo.rs`, `tests/classifier.rs`, `scripts/m6-smoke.sh`.

Evidence CI: `rust`, `m6-package-monorepo`.

Klausul yang dipetakan (5 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest ef44792f9e44</summary>

```text
Supported:
```

</details>

<details><summary>Klausul 2, digest 84f123e9d811</summary>

```text
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
```

</details>

<details><summary>Klausul 3, digest e83fe26910d7</summary>

```text
Recognized files:
```

</details>

<details><summary>Klausul 4, digest 38f415e46828</summary>

```text
    package.json
    pnpm-workspace.yaml
    turbo.json
    nx.json
    lerna.json
    yarn.lock
    pnpm-lock.yaml
```

</details>

<details><summary>Klausul 5, digest 90dfda04cceb</summary>

```text
Original root command tetap dijalankan. Manifest hanya memilih filter families.
```

</details>

## 8.4 Rust

Libtest/nextest PASS dan compiler progress dikenal compact setelah tiga record aman; workspace/package tetap command asli; compiler/failure/summary retain.

Implementasi: `src/core/classification.rs`, `src/core/filters/ecosystems.rs`.

Test/verifikasi: `tests/classifier.rs`, `tests/rust_fixtures.rs`, `tests/core_ecosystem_e2e.rs`, `tests/reduction.rs`, `scripts/m5-smoke.sh`.

Evidence CI: `rust`, `m5-core-ecosystems`.

Klausul yang dipetakan (4 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest ef44792f9e44</summary>

```text
Supported:
```

</details>

<details><summary>Klausul 2, digest 0d739427f06c</summary>

```text
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
```

</details>

<details><summary>Klausul 3, digest e1146f006ab3</summary>

```text
Filter:
```

</details>

<details><summary>Klausul 4, digest f8687e462707</summary>

```text
- compact passing tests;
- dukung `--workspace` dan `-p PACKAGE` pada invocation Cargo asli;
- compact hanya record libtest `test NAME ... ok` dan record PASS nextest yang
  cocok grammar;
- compact Compiling dan Checking progress;
- `cargo doc` juga boleh compact record Documenting yang cocok grammar;
- pertahankan tiga record aman pertama untuk membangun confidence;
- retain compiler diagnostic lengkap;
- retain failed-test output;
- retain warning dan final summary.
```

</details>

## 8.5 Python

Wrapper Python statis; pytest/unittest passing dan pip download grammar compact; diagnostic-only/tabel/summary/aplikasi generic raw.

Implementasi: `src/core/classification.rs`, `src/core/filters/ecosystems.rs`.

Test/verifikasi: `tests/classifier.rs`, `tests/python_fixtures.rs`, `tests/core_ecosystem_e2e.rs`, `tests/reduction.rs`, `scripts/m5-smoke.sh`.

Evidence CI: `rust`, `m5-core-ecosystems`.

Klausul yang dipetakan (6 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 1fd7adf48c04</summary>

```text
Wrapper:
```

</details>

<details><summary>Klausul 2, digest be38da50ea2b</summary>

```text
    python
    python3
    uv run
    poetry run
    pipenv run
```

</details>

<details><summary>Klausul 3, digest fc94b5b34435</summary>

```text
Filtered tools:
```

</details>

<details><summary>Klausul 4, digest fff5aa677fb3</summary>

```text
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
```

</details>

<details><summary>Klausul 5, digest ae97c1f906eb</summary>

```text
`python -m coverage run -m TOOL` meneruskan klasifikasi ke tool yang dijalankan.
Filter compact record pytest verbose `PATH::TEST PASSED` (dengan atau tanpa
persentase progress), unittest `test_NAME (...) ... ok`, dan progress download
pip yang cocok grammar. Ruff, mypy, pyright, pylint, Black, tox, nox, coverage
report, serta output install selain progress yang dikenal tetap mempertahankan
diagnostic, tabel, dan summary. Tidak ada output yang dihapus jika tool tersebut
hanya menghasilkan diagnostic atau summary.
```

</details>

<details><summary>Klausul 6, digest a716f57d92f9</summary>

```text
Generic `python app.py` tetap dibungkus TTC. Jika output tidak dikenali, output
diteruskan penuh.
```

</details>

## 8.6 Go

Go PASS frame compact; lifecycle JSON event byte-exact. Duplicate/new/malformed event mematikan parser, compound JSON raw. Generate/vet/build diagnostic retain.

Implementasi: `src/core/classification.rs`, `src/core/filters/ecosystems.rs`.

Test/verifikasi: `tests/classifier.rs`, `tests/go_fixtures.rs`, `tests/core_ecosystem_e2e.rs`, `tests/reduction.rs`, `scripts/m5-smoke.sh`.

Evidence CI: `rust`, `m5-core-ecosystems`.

Klausul yang dipetakan (6 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest ef44792f9e44</summary>

```text
Supported:
```

</details>

<details><summary>Klausul 2, digest 04be1825fa0f</summary>

```text
    go test
    go test ./...
    go test -v ./...
    go test -json ./...
    go build ./...
    go vet ./...
    go generate ./...
    golangci-lint run
    staticcheck ./...
```

</details>

<details><summary>Klausul 3, digest e83fe26910d7</summary>

```text
Recognized files:
```

</details>

<details><summary>Klausul 4, digest 7ad644ea6c8f</summary>

```text
    go.mod
    go.sum
    go.work
```

</details>

<details><summary>Klausul 5, digest e8b8c74fad0c</summary>

```text
Untuk output teks `go test -v`, TTC hanya compact frame `--- PASS: TEST (N.NNs)`
yang cocok grammar. Package summary, `=== RUN`, test failure, panic, race
detector, vet/build diagnostic, serta output `go generate` dipertahankan.
```

</details>

<details><summary>Klausul 6, digest b43c4162da3f</summary>

```text
`go test -json` memakai parser newline-delimited event khusus dari struktur
`TestEvent` resmi ([Go test2json](https://go.dev/cmd/test2json/)). Parser
mempertahankan semua lifecycle action (`start`, `run`, `pause`, `cont`, `pass`,
`bench`, `fail`, `skip`) dan seluruh event build/failure. Parser hanya boleh
compact satu baris `Action: "output"` bila `OutputType` bernilai `frame`, `Test`
sesuai nama pada satu-satunya baris `Output` `--- PASS: TEST (N.NNs)`, dan event
memiliki field dengan tipe yang dikenal. Semua event lifecycle tetap byte-exact.
Event dengan key duplikat, field/action/type baru, JSON malformed, atau payload
ambigu membuat invocation selanjutnya raw tanpa mengubah byte event yang sudah
diteruskan. Event yang dipertahankan ditulis dari byte input asli, bukan hasil
serialisasi ulang. Parser hanya aktif untuk invocation `go test -json` tunggal;
compound command yang memuat JSON tetap raw.
```

</details>

## 8.7 PHP

Composer static hints; PHPUnit/Pest/progress PHPCS/PHPStan/Composer grammar compact. PHP callback/dynamic/custom reporter/mutating fixer raw; audit/warning/failure retain.

Implementasi: `src/core/classification.rs`, `src/core/manifests.rs`, `src/core/filters/php.rs`.

Test/verifikasi: `tests/classifier.rs`, `tests/manifests.rs`, `tests/php_fixtures.rs`, `tests/additional_ecosystem_e2e.rs`, `tests/reduction.rs`, `scripts/m7-smoke.sh`.

Evidence CI: `rust`, `m7-php-jvm-dotnet`.

Klausul yang dipetakan (4 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest ef44792f9e44</summary>

```text
Supported:
```

</details>

<details><summary>Klausul 2, digest a1a14e3b7e6d</summary>

```text
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
```

</details>

<details><summary>Klausul 3, digest d08f49d73784</summary>

```text
TTC mengenali PHPUnit/Pest termasuk executable `vendor/bin`, `php artisan test`,
PHPStan, Psalm, PHPCS, php-cs-fixer dry-run, serta `composer test`, `composer run
test`, `composer install`, dan `composer update`. Composer script hanya memakai
hints statis dari manifest. PHP executable, callback, script aplikasi, atau
reporter yang tidak dapat dibuktikan dari command tetap raw.
```

</details>

<details><summary>Klausul 4, digest 4653188a4be1</summary>

```text
Recognizer hanya compact progress terminal yang memiliki grammar khusus:
record titik PHPUnit, baris passing Pest/PHPUnit, progress PHPCS/PHPStan, dan
download/install package Composer. Tool diagnostic-only boleh dikenali sebagai
family tanpa menghapus output. Tabel hasil, summary, diagnostic, assertion diff,
warning, security/audit output, struktur JSON/XML/TAP, dan reporter alternatif
dipertahankan. Output dinamis, flag reporter yang tidak didukung, php-cs-fixer
tanpa `--dry-run`, dan php-cs-fixer yang mengubah file berjalan raw.
```

</details>

## 8.8 Java dan JVM

Maven download/Gradle task/JUnit tree PASS compact; javac diagnostic-only raw. Wrapper/goal/run count/unknown reporter dan structural output tetap aman.

Implementasi: `src/core/classification.rs`, `src/core/filters/jvm.rs`, `src/core/manifests.rs`.

Test/verifikasi: `tests/classifier.rs`, `tests/manifests.rs`, `tests/jvm_fixtures.rs`, `tests/additional_ecosystem_e2e.rs`, `tests/reduction.rs`, `scripts/m7-smoke.sh`.

Evidence CI: `rust`, `m7-php-jvm-dotnet`.

Klausul yang dipetakan (4 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest ef44792f9e44</summary>

```text
Supported:
```

</details>

<details><summary>Klausul 2, digest 209b990c223c</summary>

```text
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
```

</details>

<details><summary>Klausul 3, digest 2b1879cb3c23</summary>

```text
TTC mengenali Maven dan `mvnw` untuk `test`, `verify`, `package`, dan
`install`; Gradle dan `gradlew` untuk `test`, `build`, dan `check`; serta
`javac` dan JUnit Console. Filter hanya baris download Maven yang cocok dengan
grammar, task Gradle `> Task :...` tanpa suffix atau dengan status yang dikenal
(`UP-TO-DATE`, `FROM-CACHE`, `SKIPPED`, `NO-SOURCE`), dan passing JUnit Console
dengan reporter tree yang didukung (`--details=tree`).
```

</details>

<details><summary>Klausul 4, digest dc316ecaa7b8</summary>

```text
Compilation error, compiler diagnostic, task gagal, test failure, stack trace,
warning, build summary, output machine-readable, dan reporter JUnit lain
dipertahankan. `javac` dikenali untuk perlindungan diagnostik, tetapi outputnya
tetap raw.
```

</details>

## 8.9 .NET

VSTest PASS/restore/build grammar compact; diagnostic code/failure/stack/summary retain. Mutating format/unknown logger raw.

Implementasi: `src/core/classification.rs`, `src/core/filters/dotnet.rs`.

Test/verifikasi: `tests/classifier.rs`, `tests/dotnet_fixtures.rs`, `tests/additional_ecosystem_e2e.rs`, `tests/reduction.rs`, `scripts/m7-smoke.sh`.

Evidence CI: `rust`, `m7-php-jvm-dotnet`.

Klausul yang dipetakan (3 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest ef44792f9e44</summary>

```text
Supported:
```

</details>

<details><summary>Klausul 2, digest c3166b4c02ff</summary>

```text
    dotnet test
    dotnet test SOLUTION
    dotnet build
    dotnet restore
    dotnet publish
    dotnet format --verify-no-changes
```

</details>

<details><summary>Klausul 3, digest e606ffef68bd</summary>

```text
TTC mengenali `dotnet test`, `build`, `restore`, `publish`, serta
`dotnet format --verify-no-changes`. Filter passing VSTest record dan baris
restore/build yang cocok dengan grammar. Diagnostic code, compiler output,
failed test, assertion diff, stack trace, warning, summary, JSON/XML/TAP, dan
logger/reporter yang tidak didukung dipertahankan. Format tanpa
`--verify-no-changes` berjalan raw.
```

</details>

## 8.10 C, C++, Swift, Ruby, dan build tools

Known CMake/Ninja progress/CTest passing, Swift compile/pass, Ruby meter compact. Dynamic targets/custom make/raw flags/correction/reporter raw. Semua diagnostic dan target/test summary retain.

Implementasi: `src/core/classification.rs`, `src/core/filters/m8.rs`, `src/core/filters/m8.rs`.

Test/verifikasi: `tests/classifier.rs`, `tests/build_tool_fixtures.rs`, `tests/ruby_swift_fixtures.rs`, `tests/remaining_ecosystem_e2e.rs`, `tests/reduction.rs`, `scripts/m8-smoke.sh`.

Evidence CI: `rust`, `m8-remaining-ecosystems`.

Klausul yang dipetakan (5 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest ef44792f9e44</summary>

```text
Supported:
```

</details>

<details><summary>Klausul 2, digest 3c17cffd3a73</summary>

```text
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
```

</details>

<details><summary>Klausul 3, digest 9e1d462dd0e2</summary>

```text
Filter hanya progress dan passing records yang dikenal. Grammar yang boleh
dikompaksi:
```

</details>

<details><summary>Klausul 4, digest 1c60db6ade94</summary>

```text
- CMake: record persentase `Building ...` atau `Linking ...` yang cocok grammar;
  summary target tetap ada.
- Ninja dan backend CMake Ninja: record `[N/M] Building ...` atau `Linking ...`;
  nama target eksplisit dan mode tool berjalan raw.
- CTest serta Make target `test`/`check`: hanya record test `Passed` dengan
  jumlah dan durasi valid. Failure, output test, dan summary tetap ada.
- Swift: record compile/module/link yang dikenal dan passing XCTest/Swift
  Testing dengan durasi valid. `swift build --show-bin-path` dan output package
  terstruktur raw.
- RSpec dan Rake test: hanya baris yang seluruhnya berisi titik passing.
  RuboCop: hanya baris meter titik default. Formatter alternatif, reporter,
  correction, dan offense tetap raw/retained.
```

</details>

<details><summary>Klausul 5, digest efb83d7585d1</summary>

```text
Dynamic target CMake/CTest/Ninja/Make yang tidak dapat diidentifikasi, custom
Makefile, target selain `test`/`check`, dan grammar build/test yang tidak dikenal
berjalan raw. TTC tidak membaca/evaluasi Makefile untuk memilih filter.
```

</details>

## 8.11 Container dan infrastructure

BuildKit internal metadata dan Helm banner saja compact; RUN/COPY/app/security/resource/diff/summary retain. Terraform diagnostic-only; unknown Podman full retain; follow/Compose up raw streaming.

Implementasi: `src/core/classification.rs`, `src/core/filters/m8.rs`.

Test/verifikasi: `tests/classifier.rs`, `tests/infrastructure_fixtures.rs`, `tests/raw_command_matrix.rs`, `tests/remaining_ecosystem_e2e.rs`, `tests/m8_smoke_assertions.rs`, `scripts/m8-smoke.sh`.

Evidence CI: `rust`, `m8-remaining-ecosystems`.

Klausul yang dipetakan (5 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 12e2d8d2f05e</summary>

```text
Filtering terbatas:
```

</details>

<details><summary>Klausul 2, digest 9d4fb49c7c59</summary>

```text
    docker build
    docker compose build
    podman build
    terraform validate
    helm lint
```

</details>

<details><summary>Klausul 3, digest 990e6a73e296</summary>

```text
Plan, diff, resource changes, warning, error, dan security output selalu dipertahankan.
```

</details>

<details><summary>Klausul 4, digest 167d4ed2f7f9</summary>

```text
Docker dan Docker Compose hanya boleh compact record metadata BuildKit internal
untuk memuat definisi build, `.dockerignore`, build context, atau metadata image.
Langkah `RUN`/`COPY`, cache/build summary, output aplikasi, warning, error, dan
security message tetap dipertahankan. JSON/rawjson dan mode quiet berjalan raw.
Output Podman yang tidak cocok grammar progress khusus dipertahankan penuh.
`terraform validate` adalah diagnostic-only: output validasi maupun summary
tidak pernah dikompaksi; JSON berjalan raw. Helm lint hanya boleh compact banner
`==> Linting CHART`; lint result, rekomendasi, diagnostic, dan summary tetap ada.
```

</details>

<details><summary>Klausul 5, digest d850b76e0512</summary>

```text
Interactive docker compose up (termasuk opsi global Compose) dan `kubectl logs`
dengan `-f`/`--follow` raw streaming.
```

</details>

## 8.12 Commands yang selalu raw

Semua reader/network/database/aplikasi unknown/git diff/show raw byte-exact; bukan bypass manual.

Implementasi: `src/core/classification.rs`, `src/core/execution.rs`.

Test/verifikasi: `tests/raw_command_matrix.rs`, `tests/passthrough.rs`, `tests/classifier.rs`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m8-remaining-ecosystems`, `m9-distribution`.

Klausul yang dipetakan (2 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 741653e152d5</summary>

```text
Walaupun dibungkus TTC, output berikut raw pada MVP:
```

</details>

<details><summary>Klausul 2, digest 3c5251b0d2b5</summary>

```text
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
```

</details>

## 9. Monorepo requirements

Manifest lazy statis bounded; cycle/overflow/dynamic/ambiguous raw. Root runner authoritative; selector candidate dan prefix terdaftar, confidence stream/project/task/recognizer, unknown diagnostic mematikan stream. Mixed family tanpa child tambahan.

Implementasi: `src/core/manifests.rs`, `src/core/classification.rs`, `src/core/filters/mod.rs`, `src/core/filters/mod.rs`.

Test/verifikasi: `tests/manifests.rs`, `tests/package_scripts.rs`, `tests/monorepo.rs`, `tests/mixed_monorepo.rs`, `tests/classifier.rs`, `scripts/m6-smoke.sh`.

Evidence CI: `rust`, `m6-package-monorepo`.

Klausul yang dipetakan (0 kelompok; seluruh bullet/command di dalamnya):

## 9.1 Original command authoritative

Manifest lazy statis bounded; cycle/overflow/dynamic/ambiguous raw. Root runner authoritative; selector candidate dan prefix terdaftar, confidence stream/project/task/recognizer, unknown diagnostic mematikan stream. Mixed family tanpa child tambahan.

Implementasi: `src/core/manifests.rs`, `src/core/classification.rs`, `src/core/filters/mod.rs`, `src/core/filters/mod.rs`.

Test/verifikasi: `tests/manifests.rs`, `tests/package_scripts.rs`, `tests/monorepo.rs`, `tests/mixed_monorepo.rs`, `tests/classifier.rs`, `scripts/m6-smoke.sh`.

Evidence CI: `rust`, `m6-package-monorepo`.

Klausul yang dipetakan (3 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 3288dc04ff85</summary>

```text
TTC tidak menjalankan package satu per satu. TTC menjalankan root command asli:
```

</details>

<details><summary>Klausul 2, digest c2d715624aa4</summary>

```text
    pnpm test
    turbo run test
    nx run-many -t test
    cargo test --workspace
    go test ./...
```

</details>

<details><summary>Klausul 3, digest c60d830987a2</summary>

```text
Dependency graph, cache, concurrency, dan environment runner tetap berlaku.
```

</details>

## 9.2 Recursive script discovery

Manifest lazy statis bounded; cycle/overflow/dynamic/ambiguous raw. Root runner authoritative; selector candidate dan prefix terdaftar, confidence stream/project/task/recognizer, unknown diagnostic mematikan stream. Mixed family tanpa child tambahan.

Implementasi: `src/core/manifests.rs`, `src/core/classification.rs`, `src/core/filters/mod.rs`, `src/core/filters/mod.rs`.

Test/verifikasi: `tests/manifests.rs`, `tests/package_scripts.rs`, `tests/monorepo.rs`, `tests/mixed_monorepo.rs`, `tests/classifier.rs`, `scripts/m6-smoke.sh`.

Evidence CI: `rust`, `m6-package-monorepo`.

Klausul yang dipetakan (9 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 8fe1d2c372f6</summary>

```text
TTC membaca manifest secara statis:
```

</details>

<details><summary>Klausul 2, digest 2e3491857fa9</summary>

```text
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
```

</details>

<details><summary>Klausul 3, digest f8a60b36e549</summary>

```text
Composer script string dan array dibaca statis. Alias script rekursif,
`@php`/`@composer`, working directory, dan callback lifecycle install/update
yang relevan diselesaikan memakai aturan Composer; callback PHP, ekspansi
dinamis, alias cycle, atau command yang tidak dapat dibuktikan membuat
invocation raw. Maven dibaca sebagai XML struktural dengan parser pull; TTC
hanya memakai identitas proyek dan module path literal, tidak mengambil
external entity, parent, atau dependency dari jaringan. Gradle Groovy/Kotlin
hanya memberi hints dari include/path literal tanpa mengevaluasi DSL, plugin,
atau menjalankan Gradle. Module/target dinamis dan referensi wajib yang ambigu
membuat invocation raw.
```

</details>

<details><summary>Klausul 4, digest e9354646407b</summary>

```text
Batas:
```

</details>

<details><summary>Klausul 5, digest 3ea26383564e</summary>

```text
- recursion depth maksimum 16;
- cycle detection;
- parsing gagal berarti raw;
- discovery tidak menjalankan script.
```

</details>

<details><summary>Klausul 6, digest 177cd0159647</summary>

```text
Discovery bersifat lazy dan statis. Direktori pencarian berhenti pada root
workspace terdekat yang ditemukan dari cwd. Batas per invocation adalah 1 MiB
per manifest, 16 MiB total manifest, 4.096 project, 16.384 entri direktori,
64 tingkat traversal, dan 16 tingkat resolusi alias script/target. Kedalaman
pertama bernilai 1; tingkat ke-16 masih diterima. Overflow, path ambigu, cycle,
manifest wajib yang tidak dapat dibaca, atau parse gagal membuat invocation
raw sebelum output dikompaksi. Cache discovery hanya hidup selama invocation.
```

</details>

<details><summary>Klausul 7, digest 9cc97bea1c22</summary>

```text
Traversal tidak memasuki `.git`, `node_modules`, atau `target` kecuali path
referensi eksplisit menunjuk langsung ke sana. Symlink dikanonisasi untuk
mencegah loop dan penghitungan project ganda; traversal otomatis tidak membaca
target symlink yang keluar dari root workspace.
```

</details>

<details><summary>Klausul 8, digest c5135f4be22a</summary>

```text
Analisis shell package script hanya mendukung command literal, assignment
environment, `cd` literal, `&&`, `;`, dan newline. Quoting dan escaping
mempertahankan argument operator literal. Pipeline, background job, redirection,
command substitution, ekspansi, atau control flow yang tidak dapat dibuktikan
statis berjalan raw. Lexer hanya memengaruhi klasifikasi; TTC tetap menjalankan
argv atau string command asli sekali.
```

</details>

<details><summary>Klausul 9, digest 045bb0da255f</summary>

```text
Resolver membedakan alias menurut manager, project canonical, dan nama script.
Cycle aktif membuat seluruh invocation raw, sedangkan pemanggilan alias yang
selesai sebelumnya bukan cycle. Ketika manager menjalankan lifecycle `pre*`/
`post*`, discovery mengikuti perilaku manager dari metadata proyek: npm dan Bun
mengurai lifecycle yang diketahui; pnpm hanya jika `enablePrePostScripts` aktif
di manifest workspace; Yarn Classic hanya jika versi 1 diketahui. Yarn modern
tidak menjalankan arbitrary pre/post script. Jika perilaku Yarn tidak diketahui
dan lifecycle kandidat ada, output raw.
```

</details>

## 9.2.1 Workspace dan runner hints

Manifest lazy statis bounded; cycle/overflow/dynamic/ambiguous raw. Root runner authoritative; selector candidate dan prefix terdaftar, confidence stream/project/task/recognizer, unknown diagnostic mematikan stream. Mixed family tanpa child tambahan.

Implementasi: `src/core/manifests.rs`, `src/core/classification.rs`, `src/core/filters/mod.rs`, `src/core/filters/mod.rs`.

Test/verifikasi: `tests/manifests.rs`, `tests/package_scripts.rs`, `tests/monorepo.rs`, `tests/mixed_monorepo.rs`, `tests/classifier.rs`, `scripts/m6-smoke.sh`.

Evidence CI: `rust`, `m6-package-monorepo`.

Klausul yang dipetakan (5 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest fefeffbad54f</summary>

```text
Workspace hints dibaca dari manifest npm/pnpm/Yarn/Bun, konfigurasi Turbo/Nx/
Lerna/Lage/Moon, Cargo workspace, dan `go.work`. Adapter hanya menentukan
candidate filter family. Runner asli tetap menentukan package terpilih,
dependency graph, urutan, concurrency, cache, cwd, dan environment.
```

</details>

<details><summary>Klausul 2, digest fbd4d2477d94</summary>

```text
Selector statis yang didukung meliputi workspace bernama/path npm, pnpm
recursive/filter/path dan relasi dependency/dependent lokal, Yarn workspace/
foreach, Bun script/filter, Turbo task, Nx target/run-many/affected, Lerna run,
Lage task, Moon target, Cargo workspace/package, serta Go workspace. Selector
Git/query/plugin atau konfigurasi executable tidak dijalankan oleh TTC. Hints
statis menjadi kandidat; filtering fallback hanya berlaku untuk recognizer
family yang sudah diketahui dan prefix project yang terdaftar. Prefix dibuang
hanya untuk pengenalan; record yang dipertahankan tetap byte asli. Prefix
ambigu, task banner, cache summary, unknown record, dan diagnostic tetap raw.
```

</details>

<details><summary>Klausul 3, digest c03a8bf6c872</summary>

```text
Prefix runner hanya boleh di-unwrapping jika grammar dan identitas project
terdaftar cocok. Bentuk yang didukung meliputi `packages/path task: ` dari
pnpm, `project#task: ` dari Turbo, `project:  ` dan `project:task: ` dari Nx/
Lerna, `project task : ` dari Lage, serta `project:task | ` dari Moon. ID
project yang dideklarasikan Moon menjadi alias sumber tambahan walau project
yang sama sudah ditemukan melalui workspace `package.json`. Alias yang sama
menunjuk lebih dari satu project menjadikan invocation raw. TTC selalu
mempertahankan prefix byte asli saat emission.
```

</details>

<details><summary>Klausul 4, digest a437b75033ee</summary>

```text
Jika manager menjalankan beberapa package tanpa prefix sumber per record,
record tak berprefix tetap raw. Ini mencakup bentuk standar npm `--workspaces`,
Yarn Classic `workspaces run`, dan Yarn modern `workspaces foreach`. Prefix
yang dikenal masih dapat difilter dalam invocation itu. Banner teks runner
seperti `Scope: 2 of 3 workspace projects` bukan bukti structured output dan
tidak mematikan recognizer untuk record package yang datang sesudahnya.
```

</details>

<details><summary>Klausul 5, digest 7ee039570a6c</summary>

```text
Sumber confidence dipisahkan menurut stream, project/task yang teridentifikasi,
dan recognizer. Batas state sumber adalah 4.096; overflow menonaktifkan filter
selebihnya. Fallback tanpa target script statis wajib mengenali prefix project
yang terdaftar. Diagnostic tanpa identitas sumber yang dapat dibuktikan membuat
sisa physical stream terkait raw, termasuk setelah baris kosong.
```

</details>

## 9.3 Mixed-language monorepo

Manifest lazy statis bounded; cycle/overflow/dynamic/ambiguous raw. Root runner authoritative; selector candidate dan prefix terdaftar, confidence stream/project/task/recognizer, unknown diagnostic mematikan stream. Mixed family tanpa child tambahan.

Implementasi: `src/core/manifests.rs`, `src/core/classification.rs`, `src/core/filters/mod.rs`, `src/core/filters/mod.rs`.

Test/verifikasi: `tests/manifests.rs`, `tests/package_scripts.rs`, `tests/monorepo.rs`, `tests/mixed_monorepo.rs`, `tests/classifier.rs`, `scripts/m6-smoke.sh`.

Evidence CI: `rust`, `m6-package-monorepo`.

Klausul yang dipetakan (5 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 8d5ba51213cb</summary>

```text
Contoh root package.json:
```

</details>

<details><summary>Klausul 2, digest 67d041e946bb</summary>

```text
    {
      "scripts": {
        "test": "turbo run test && go test ./..."
      }
    }
```

</details>

<details><summary>Klausul 3, digest 9bfd94f4f411</summary>

```text
Ketika Codex menjalankan pnpm test, TTC menjalankan pnpm test sekali dan mengaktifkan filter:
```

</details>

<details><summary>Klausul 4, digest 2d23f1b1f113</summary>

```text
- JavaScript test;
- Turbo workspace;
- Go test.
```

</details>

<details><summary>Klausul 5, digest 64f8f06f02d3</summary>

```text
Output JavaScript dan Go dapat dikompaksi dalam satu invocation. Error dari ecosystem mana pun tetap terlihat.
```

</details>

## 9.4 Output-signature fallback

Manifest lazy statis bounded; cycle/overflow/dynamic/ambiguous raw. Root runner authoritative; selector candidate dan prefix terdaftar, confidence stream/project/task/recognizer, unknown diagnostic mematikan stream. Mixed family tanpa child tambahan.

Implementasi: `src/core/manifests.rs`, `src/core/classification.rs`, `src/core/filters/mod.rs`, `src/core/filters/mod.rs`.

Test/verifikasi: `tests/manifests.rs`, `tests/package_scripts.rs`, `tests/monorepo.rs`, `tests/mixed_monorepo.rs`, `tests/classifier.rs`, `scripts/m6-smoke.sh`.

Evidence CI: `rust`, `m6-package-monorepo`.

Klausul yang dipetakan (3 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 3a0b02c59a6b</summary>

```text
TTC juga mendeteksi signature output saat streaming karena runner tidak selalu mengekspos child command:
```

</details>

<details><summary>Klausul 2, digest 954640f6092e</summary>

```text
- Rust compiler/test;
- Vitest/Jest;
- Go test;
- pytest;
- Maven/Gradle;
- dotnet;
- PHP test tools.
```

</details>

<details><summary>Klausul 3, digest 375c045cfbe0</summary>

```text
Signature detection baru aktif setelah beberapa record konsisten. Record sebelum confidence tercapai tetap diteruskan.
```

</details>

## 10. Streaming architecture

Concurrent stdout/stderr readers, bounded channel/framing/memory, pending line 1MiB, non-UTF8 raw, per-stream order. Binary readable/executable mode755; tidak mengatur writable root.

Implementasi: `src/core/execution.rs`, `src/core/streaming.rs`, `src/distribution/files.rs`.

Test/verifikasi: `tests/streaming.rs`, `tests/passthrough.rs`, `tests/shell_contract.rs`, `scripts/m3-evidence.sh`, `scripts/m9-install-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m9-distribution`.

Batas tahap: Eksekusi actual Codex read-only/workspace-write/danger-full-access dibuktikan manual M10.

Klausul yang dipetakan (4 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest e3f8dafa01b6</summary>

```text
MVP:
```

</details>

<details><summary>Klausul 2, digest 8d5b642d045b</summary>

```text
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
```

</details>

<details><summary>Klausul 3, digest ed137017069a</summary>

```text
Urutan internal masing-masing stream harus tetap sama.
```

</details>

<details><summary>Klausul 4, digest 41abc15da27f</summary>

```text
Binary di `~/.local/bin/ttc` harus readable dan executable ketika command
dijalankan dalam sandbox Codex read-only, workspace-write, maupun
danger-full-access. TTC tidak menambah writable root atau memperlebar permission
command asli.
```

</details>

## 11. Raw output retrieval

Tagged original streams, random ID, storage privat XDG/fallback stabil, tail32MiB/dropped count, cleanup24h, tanpa capture bila tidak compact; failure raw tanpa rerun, staged valid replay.

Implementasi: `src/core/raw_store.rs`, `src/cli.rs`, `src/core/streaming.rs`.

Test/verifikasi: `tests/raw_store.rs`, `tests/raw_cli.rs`, `tests/storage_failure.rs`, `tests/streaming.rs`, `scripts/m3-evidence.sh`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m3-streaming`, `m9-distribution`.

Klausul yang dipetakan (8 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest fbabd84e2351</summary>

```text
Raw output menggunakan file biasa, bukan database. Command `ttc raw` hanya
membaca hasil capture; command ini bukan bypass filtering dan tidak menjalankan
ulang command asli.
```

</details>

<details><summary>Klausul 2, digest c8e03f6f399a</summary>

```text
- lokasi default: XDG_STATE_HOME/ttc/runs;
- fallback jika lokasi default tidak writable: direktori temporary per-user
  `/tmp/ttc-<uid>/runs` yang stabil antar invocation dengan permission 0700;
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
```

</details>

<details><summary>Klausul 3, digest 457f4957af84</summary>

```text
Output:
```

</details>

<details><summary>Klausul 4, digest d0eab6018948</summary>

```text
    raw: ttc raw ID
```

</details>

<details><summary>Klausul 5, digest 4741fec4836b</summary>

```text
Summary kompaksi dan hint raw ditulis ke stderr. Jika tidak ada kompaksi, TTC
tidak menambahkan metadata apa pun.
```

</details>

<details><summary>Klausul 6, digest c12b95153db8</summary>

```text
Commands:
```

</details>

<details><summary>Klausul 7, digest 747524f5e1fc</summary>

```text
    ttc raw ID
    ttc raw ID --stdout
    ttc raw ID --stderr
    ttc raw ID --tail 100
```

</details>

<details><summary>Klausul 8, digest 58ebfc808777</summary>

```text
Tanpa selector, `ttc raw ID` mereplay stdout dan stderr dalam urutan event yang
diamati. Selector `--stdout` dan `--stderr` mengekstrak stream masing-masing.
Hasil selector ditulis ke stdout agar dapat dipipe. `--tail N` mengambil N byte
terakhir dari replay setelah selector diterapkan; N boleh nol dan dapat memotong
event di tengah. Capture yang terpotong menyatakan jumlah byte asli yang dibuang
melalui stderr saat direplay.
Command mencari ID pada lokasi XDG maupun fallback temporary.
Penolakan akses pada lokasi XDG tidak menghentikan pencarian fallback. Bila
penulisan capture gagal setelah kompaksi dimulai, bagian capture terakhir yang
valid tetap dapat direplay lewat ID selama filenya dapat dibaca, termasuk dari
file staging bila finalisasi gagal. Output berikutnya berjalan raw.
```

</details>

## 12. Minimal configuration

Config TOML optional batas32MiB/24h; invalid raw sekali. Tidak ada network/config remote/bypass/telemetry.

Implementasi: `src/core/config.rs`, `src/core/execution.rs`, `Cargo.toml`.

Test/verifikasi: `tests/config.rs`, `tests/storage_failure.rs`, `tests/cli.rs`.

Evidence CI: `rust`, `m3-streaming`.

Batas tahap: Uninstall integrasi Codex untuk disable hanya tersedia M10.

Klausul yang dipetakan (8 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 9395e29a927a</summary>

```text
File:
```

</details>

<details><summary>Klausul 2, digest c23236f25941</summary>

```text
    ~/.config/ttc/config.toml
```

</details>

<details><summary>Klausul 3, digest d1f6d9e71b8b</summary>

```text
Default:
```

</details>

<details><summary>Klausul 4, digest 6833c9fb43f4</summary>

```text
    max_raw_mb = 32
    retention_hours = 24
```

</details>

<details><summary>Klausul 5, digest 7f5e76e35b0a</summary>

```text
Kedua nilai adalah batas maksimum sekaligus default. Config hanya boleh memakai
bilangan bulat positif yang lebih kecil atau sama dengan 32 MiB dan 24 jam.
Config invalid membuat invocation berjalan raw tanpa menjalankan ulang command.
```

</details>

<details><summary>Klausul 6, digest 8b04e23d2368</summary>

```text
Tidak ada:
```

</details>

<details><summary>Klausul 7, digest c10483c5ff35</summary>

```text
- compatibility profile;
- automatic rewrite flag;
- metrics;
- telemetry;
- remote config;
- project policy lattice.
```

</details>

<details><summary>Klausul 8, digest 5d42a375b295</summary>

```text
Tidak ada environment variable atau subcommand untuk melewati filtering.
Untuk menonaktifkan integrasi, pengguna menjalankan `ttc uninstall codex`.
```

</details>

## 13. Distribution dan versioning

Linux GNU stable latest dipin satu tag HTTPS+SHA; local finalizer bounded streaminghash, strict schema/ownership/no symlink, lock/staging/atomic rename/rollback/pending marker. PATH exact-owned backup/manualfallback; uninstall preserve integration/userconfig/raw/otherprogram. Tag stable matches Cargo/master; semua gates clean ulang, candidate saja. Config existing memakai atomic exchange dan memeriksa file aktual yang tergeser; race sesudah validasi terakhir, rollback, dan uninstall mempertahankan recovery/marker.

Implementasi: `install.sh`, `src/distribution/mod.rs`, `src/distribution/files.rs`, `src/distribution/metadata.rs`, `src/distribution/path.rs`, `.github/workflows/ci.yml`, `scripts/check-release-version.py`.

Test/verifikasi: `src/distribution/mod.rs`, `src/distribution/path.rs`, `scripts/m9-install-tests.py`, `scripts/m9-artifact-tests.py`, `scripts/test-release-policy.py`, `tests/cli.rs`.

Evidence CI: `rust`, `release-policy`, `m9-distribution`.

Batas tahap: Public releases/latest dan job publish baru M10 sesudah user-authorized tag; belum diklaim tersedia.

Klausul yang dipetakan (15 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest ac47e87bfecf</summary>

```text
Source distribusi publik adalah GitHub repository `skirja/ttc-ai`.
```

</details>

<details><summary>Klausul 2, digest e99e522c55ea</summary>

```text
Rilis pertama:
```

</details>

<details><summary>Klausul 3, digest ea9db5168813</summary>

```text
- versi package dan binary: `0.1.0`;
- tag: `v0.1.0`;
- target: `x86_64-unknown-linux-gnu`;
- asset executable langsung bernama `ttc-x86_64-unknown-linux-gnu`;
- checksum `SHA256SUMS`;
- `install.sh` sebagai jalur install utama.
```

</details>

<details><summary>Klausul 4, digest 4a92a0a5c5e0</summary>

```text
Tag stable `vX.Y.Z` hanya boleh dibuat dengan otorisasi pengguna setelah seluruh
acceptance binary dan Codex lulus, pada commit yang terdapat dalam `master`.
Tag menjalankan ulang seluruh gate dari checkout bersih, memverifikasi versi
tag sama dengan versi package, membangun dan smoke-test release binary, serta
membuat checksum. Workflow M9 hanya menghasilkan artifact kandidat; job
publikasi GitHub Release ditambahkan pada M10 setelah gate rilisnya terpenuhi.
Push biasa ke `master` tidak memublikasikan release.
```

</details>

<details><summary>Klausul 5, digest 20877487a7df</summary>

```text
Installer utama:
```

</details>

<details><summary>Klausul 6, digest 40dd85b54c12</summary>

```text
    curl --proto '=https' --tlsv1.2 -LsSf \
      https://github.com/skirja/ttc-ai/releases/latest/download/install.sh | sh
```

</details>

<details><summary>Klausul 7, digest f10de53a3ee7</summary>

```text
`install.sh`:
```

</details>

<details><summary>Klausul 8, digest 69ee9d48f57c</summary>

```text
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
- pada M9, mencetak absolute path binary untuk command standalone sebelum
  PATH direload; setelah command integrasi tersedia pada M10, mencetak
  `~/.local/bin/ttc install codex` sebagai next action;
- jika update shell config gagal, binary tetap terpasang dan installer mencetak
  instruksi PATH manual.
```

</details>

<details><summary>Klausul 9, digest 730440d02b4e</summary>

```text
Installer mengunci versi dari redirect `releases/latest` ke satu tag stable
SemVer, lalu mengambil executable dan checksum dari tag yang sama. Versi
executable terverifikasi harus cocok dengan tag. Override endpoint hanya
tersedia pada flow test dengan opt-in eksplisit dan endpoint HTTP loopback;
interface production tidak menyediakan version selector.
```

</details>

<details><summary>Klausul 10, digest b083553da96c</summary>

```text
Finalizer tersembunyi `ttc __install --sha256 HASH` melakukan operasi lokal
tanpa jaringan dan tidak muncul pada help publik. Metadata schema versi 1
mencatat owner `ttc`, version, SHA-256, installed path, ownership blok Bash,
serta daftar `active_harnesses`. Operasi install/uninstall memakai lock file
`XDG_DATA_HOME/ttc/install.lock`; file koordinasi ini dipertahankan setelah
uninstall agar proses concurrent tidak memakai lock inode yang berbeda.
Path administrasi instalasi harus absolute dan UTF-8; `XDG_DATA_HOME` kosong
memakai default. Direktori HOME/data/bin harus milik user dan tidak writable
oleh user lain. Kontrak argv/output core tetap mendukung byte non-UTF-8.
```

</details>

<details><summary>Klausul 11, digest 3fd2f688a44d</summary>

```text
File yang diganti harus regular, dimiliki user yang menjalankan TTC, dan
bukan symlink. Metadata unknown/invalid, checksum binary existing berbeda,
atau transaksi terputus menyebabkan penolakan sebelum replacement berikutnya.
Marker `install.pending` dipertahankan bila interruption menyisakan state
ambigu; tidak ada recovery otomatis yang menimpa file. Kegagalan commit
metadata saat replacement mengembalikan binary sebelumnya. Config Bash
read-only, symlink, invalid UTF-8, atau terlalu besar untuk edit bounded tetap
dipertahankan dan menghasilkan instruksi PATH manual. Uninstall hanya
menghapus blok yang masih cocok byte-exact dengan ownership metadata, serta
mempertahankan config, raw capture, backup, dan penggunaan PATH lain.
```

</details>

<details><summary>Klausul 12, digest 033a7cbac9da</summary>

```text
Replacement dan rollback config Bash existing memakai pertukaran atomik yang
mempertahankan file aktual yang tergeser sebagai backup, lalu memvalidasi
identity dan byte snapshot pada file tersebut. Save pengguna setelah validasi
terakhir tidak boleh hilang. Konflik setelah pertukaran atau kegagalan sync
mempertahankan kedua versi, mencetak lokasi recovery, dan meninggalkan marker
transaksi agar operasi berikutnya ditolak. Rollback config yang sebelumnya
absent memindahkan file aktual ke lokasi recovery tanpa overwrite sebelum
validasi. Filesystem yang tidak mendukung operasi atomik tersebut menghasilkan
fallback PATH manual sebelum config disentuh.
```

</details>

<details><summary>Klausul 13, digest b5d4d3e5c743</summary>

```text
Pengguna manual dapat mengunduh executable dan checksum yang sama, menjalankan
verifikasi, memberi permission dengan `chmod +x`, lalu memindahkan binary ke
path pilihannya. Jalur manual di luar `~/.local/bin/ttc` tidak didaftarkan oleh
`ttc install codex` pada MVP.
```

</details>

<details><summary>Klausul 14, digest aa0fe28b4f38</summary>

```text
Update dilakukan dengan menjalankan ulang installer latest. Tidak ada
`ttc update`, version selector, atau network access dari binary TTC pada MVP.
```

</details>

<details><summary>Klausul 15, digest 89da29cce9d0</summary>

```text
CI utama berjalan pada branch `master`. Artifact release-mode Phase 1 harus
lulus sebelum implementasi hook Codex dimulai. Public release hanya dibuat
setelah Phase 2 lulus.
```

</details>

## 14. Test plan

Full test unit/integration/safety/fixture/monorepo, pinned real tools dan large >=80% bytes termasuk metadata. CI Linux artifact+installer master final acceptance; command exit/signals/env/once dan raw storage isolasi. Tidak mengklaim token/sandbox Codex dari fixture.

Implementasi: `tests/common/mod.rs`, `.github/workflows/ci.yml`.

Test/verifikasi: `tests/reduction.rs`, `tests/execution.rs`, `tests/raw_command_matrix.rs`, `tests/filter_safety.rs`, `tests/mixed_monorepo.rs`, `scripts/m3-evidence.sh`, `scripts/m4-smoke.sh`, `scripts/m5-smoke.sh`, `scripts/m6-smoke.sh`, `scripts/m7-smoke.sh`, `scripts/m8-smoke.sh`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m4-javascript`, `m5-core-ecosystems`, `m6-package-monorepo`, `m7-php-jvm-dotnet`, `m8-remaining-ecosystems`, `m9-distribution`.

Batas tahap: Quoting/recursive prevention hook pada §14.1 serta seluruh Phase2 DoD hanya M10. Final master run menunggu merge pengguna.

Klausul yang dipetakan (0 kelompok; seluruh bullet/command di dalamnya):

## 14.1 Unit tests

Full test unit/integration/safety/fixture/monorepo, pinned real tools dan large >=80% bytes termasuk metadata. CI Linux artifact+installer master final acceptance; command exit/signals/env/once dan raw storage isolasi. Tidak mengklaim token/sandbox Codex dari fixture.

Implementasi: `tests/common/mod.rs`, `.github/workflows/ci.yml`.

Test/verifikasi: `tests/reduction.rs`, `tests/execution.rs`, `tests/raw_command_matrix.rs`, `tests/filter_safety.rs`, `tests/mixed_monorepo.rs`, `scripts/m3-evidence.sh`, `scripts/m4-smoke.sh`, `scripts/m5-smoke.sh`, `scripts/m6-smoke.sh`, `scripts/m7-smoke.sh`, `scripts/m8-smoke.sh`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m4-javascript`, `m5-core-ecosystems`, `m6-package-monorepo`, `m7-php-jvm-dotnet`, `m8-remaining-ecosystems`, `m9-distribution`.

Batas tahap: Quoting/recursive prevention hook pada §14.1 serta seluruh Phase2 DoD hanya M10. Final master run menunggu merge pengguna.

Klausul yang dipetakan (1 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest e060ea0bc81b</summary>

```text
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
```

</details>

## 14.2 Execution tests

Full test unit/integration/safety/fixture/monorepo, pinned real tools dan large >=80% bytes termasuk metadata. CI Linux artifact+installer master final acceptance; command exit/signals/env/once dan raw storage isolasi. Tidak mengklaim token/sandbox Codex dari fixture.

Implementasi: `tests/common/mod.rs`, `.github/workflows/ci.yml`.

Test/verifikasi: `tests/reduction.rs`, `tests/execution.rs`, `tests/raw_command_matrix.rs`, `tests/filter_safety.rs`, `tests/mixed_monorepo.rs`, `scripts/m3-evidence.sh`, `scripts/m4-smoke.sh`, `scripts/m5-smoke.sh`, `scripts/m6-smoke.sh`, `scripts/m7-smoke.sh`, `scripts/m8-smoke.sh`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m4-javascript`, `m5-core-ecosystems`, `m6-package-monorepo`, `m7-php-jvm-dotnet`, `m8-remaining-ecosystems`, `m9-distribution`.

Batas tahap: Quoting/recursive prevention hook pada §14.1 serta seluruh Phase2 DoD hanya M10. Final master run menunggu merge pengguna.

Klausul yang dipetakan (1 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest f1dabd0ef2a5</summary>

```text
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
```

</details>

## 14.3 Ecosystem fixtures

Full test unit/integration/safety/fixture/monorepo, pinned real tools dan large >=80% bytes termasuk metadata. CI Linux artifact+installer master final acceptance; command exit/signals/env/once dan raw storage isolasi. Tidak mengklaim token/sandbox Codex dari fixture.

Implementasi: `tests/common/mod.rs`, `.github/workflows/ci.yml`.

Test/verifikasi: `tests/reduction.rs`, `tests/execution.rs`, `tests/raw_command_matrix.rs`, `tests/filter_safety.rs`, `tests/mixed_monorepo.rs`, `scripts/m3-evidence.sh`, `scripts/m4-smoke.sh`, `scripts/m5-smoke.sh`, `scripts/m6-smoke.sh`, `scripts/m7-smoke.sh`, `scripts/m8-smoke.sh`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m4-javascript`, `m5-core-ecosystems`, `m6-package-monorepo`, `m7-php-jvm-dotnet`, `m8-remaining-ecosystems`, `m9-distribution`.

Batas tahap: Quoting/recursive prevention hook pada §14.1 serta seluruh Phase2 DoD hanya M10. Final master run menunggu merge pengguna.

Klausul yang dipetakan (4 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 97f331be2926</summary>

```text
Success, failure, warning, dan large fixture minimum untuk:
```

</details>

<details><summary>Klausul 2, digest e82932443157</summary>

```text
- npm/pnpm/yarn/bun dan Vitest/Jest;
- Turbo/Nx;
- Cargo;
- Python/pytest;
- Go;
- PHP;
- Maven/Gradle;
- dotnet.
```

</details>

<details><summary>Klausul 3, digest ddc538d0adc9</summary>

```text
Tambahkan fixture untuk C/C++ build tools, Swift, Ruby, container, dan
infrastructure. Large fixture minimum 1.000 passing atau progress records dan
harus mengurangi byte output minimum 80% tanpa kehilangan warning, error,
failure, diagnostic, atau final summary.
```

</details>

<details><summary>Klausul 4, digest 133766ce33af</summary>

```text
Setiap ecosystem family memiliki minimal satu smoke E2E memakai tool asli.
Versi tool dipin pada CI agar hasil reproducible.
```

</details>

## 14.4 Monorepo E2E

Full test unit/integration/safety/fixture/monorepo, pinned real tools dan large >=80% bytes termasuk metadata. CI Linux artifact+installer master final acceptance; command exit/signals/env/once dan raw storage isolasi. Tidak mengklaim token/sandbox Codex dari fixture.

Implementasi: `tests/common/mod.rs`, `.github/workflows/ci.yml`.

Test/verifikasi: `tests/reduction.rs`, `tests/execution.rs`, `tests/raw_command_matrix.rs`, `tests/filter_safety.rs`, `tests/mixed_monorepo.rs`, `scripts/m3-evidence.sh`, `scripts/m4-smoke.sh`, `scripts/m5-smoke.sh`, `scripts/m6-smoke.sh`, `scripts/m7-smoke.sh`, `scripts/m8-smoke.sh`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m4-javascript`, `m5-core-ecosystems`, `m6-package-monorepo`, `m7-php-jvm-dotnet`, `m8-remaining-ecosystems`, `m9-distribution`.

Batas tahap: Quoting/recursive prevention hook pada §14.1 serta seluruh Phase2 DoD hanya M10. Final master run menunggu merge pengguna.

Klausul yang dipetakan (4 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 0e073c03f8a8</summary>

```text
Required fixtures:
```

</details>

<details><summary>Klausul 2, digest a338cfdbb526</summary>

```text
1. pnpm workspace JavaScript;
2. Turbo monorepo test/lint/typecheck;
3. Nx monorepo;
4. Cargo workspace;
5. Go workspace;
6. mixed JavaScript dan Go root script;
7. nested package script memanggil Python atau Rust.
```

</details>

<details><summary>Klausul 3, digest 27ed9f41aa38</summary>

```text
Assertions:
```

</details>

<details><summary>Klausul 4, digest 1c3813d3f282</summary>

```text
- root command dijalankan sekali;
- seluruh package yang seharusnya berjalan tetap berjalan;
- passing/progress records berkurang;
- error package mana pun terlihat;
- exit code sama dengan baseline.
```

</details>

## 14.5 Distribution E2E

Linux GNU stable latest dipin satu tag HTTPS+SHA; local finalizer bounded streaminghash, strict schema/ownership/no symlink, lock/staging/atomic rename/rollback/pending marker. PATH exact-owned backup/manualfallback; uninstall preserve integration/userconfig/raw/otherprogram. Tag stable matches Cargo/master; semua gates clean ulang, candidate saja. Config existing memakai atomic exchange dan memeriksa file aktual yang tergeser; race sesudah validasi terakhir, rollback, dan uninstall mempertahankan recovery/marker.

Implementasi: `install.sh`, `src/distribution/mod.rs`, `src/distribution/files.rs`, `src/distribution/metadata.rs`, `src/distribution/path.rs`, `.github/workflows/ci.yml`, `scripts/check-release-version.py`.

Test/verifikasi: `src/distribution/mod.rs`, `src/distribution/path.rs`, `scripts/m9-install-tests.py`, `scripts/m9-artifact-tests.py`, `scripts/test-release-policy.py`, `tests/cli.rs`.

Evidence CI: `rust`, `release-policy`, `m9-distribution`.

Batas tahap: Public releases/latest dan job publish baru M10 sesudah user-authorized tag; belum diklaim tersedia.

Klausul yang dipetakan (1 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest bc5406c5783f</summary>

```text
- tag dan package version mismatch gagal;
- artifact Linux dapat dijalankan tanpa source tree;
- checksum valid dan checksum mismatch ditolak;
- fresh install, reinstall same version, dan update berjalan atomik;
- PATH block Bash idempotent dan fallback instruction benar;
- `ttc uninstall` menolak saat integrasi harness aktif;
- global uninstall tidak menghapus PATH bila `~/.local/bin` dipakai program lain.
```

</details>

## 14.6 Codex hook E2E

Kontrak harness terdokumentasi; entrypoint hook/install/uninstall Codex tidak tersedia pada M9.

Implementasi: `src/cli.rs`.

Test/verifikasi: `tests/cli.rs`.

Evidence CI: `rust`.

Batas tahap: Seluruh kontrak hook, serializer/quoting hook, recursive wrapper, config/trust, auth E2E ephemeral, token reduction, sandbox Codex dan publikasi menunggu M10. Global uninstall sudah dipetakan pada §13.

Klausul yang dipetakan (4 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest a5be83908f0f</summary>

```text
Gunakan Codex CLI terpasang, bukan serializer tiruan.
Minimum version test adalah 0.154.0. Test lokal memakai login yang sudah ada,
release-mode TTC binary, temporary fixture workspace, dan `codex exec
--ephemeral`. Config dan artifact test harus dipulihkan atau dihapus setelah
setiap run.
```

</details>

<details><summary>Klausul 2, digest b9f101f565e3</summary>

```text
Cases:
```

</details>

<details><summary>Klausul 3, digest 39bbe76a1d05</summary>

```text
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
```

</details>

<details><summary>Klausul 4, digest 723ce917a08f</summary>

```text
Ukur bytes dan token pada request model berikutnya, bukan hanya terminal output.
Large filterable output wajib berkurang minimum 80% secara byte dan token harus
lebih rendah daripada baseline.
```

</details>

## 15. Definition of Done

Full test unit/integration/safety/fixture/monorepo, pinned real tools dan large >=80% bytes termasuk metadata. CI Linux artifact+installer master final acceptance; command exit/signals/env/once dan raw storage isolasi. Tidak mengklaim token/sandbox Codex dari fixture.

Implementasi: `tests/common/mod.rs`, `.github/workflows/ci.yml`.

Test/verifikasi: `tests/reduction.rs`, `tests/execution.rs`, `tests/raw_command_matrix.rs`, `tests/filter_safety.rs`, `tests/mixed_monorepo.rs`, `scripts/m3-evidence.sh`, `scripts/m4-smoke.sh`, `scripts/m5-smoke.sh`, `scripts/m6-smoke.sh`, `scripts/m7-smoke.sh`, `scripts/m8-smoke.sh`, `scripts/m9-artifact-tests.py`.

Evidence CI: `rust`, `m2-passthrough`, `m3-streaming`, `m4-javascript`, `m5-core-ecosystems`, `m6-package-monorepo`, `m7-php-jvm-dotnet`, `m8-remaining-ecosystems`, `m9-distribution`.

Batas tahap: Quoting/recursive prevention hook pada §14.1 serta seluruh Phase2 DoD hanya M10. Final master run menunggu merge pengguna.

Klausul yang dipetakan (4 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 3d7db7da37de</summary>

```text
Phase 1 selesai jika:
```

</details>

<details><summary>Klausul 2, digest ee7a09b089e4</summary>

```text
- binary release berhasil dibangun;
- seluruh test dan lint lulus;
- command asli selalu dijalankan sekali;
- exit, signal, cwd, dan environment benar;
- seluruh ecosystem, monorepo fixtures, dan pinned real-tool smoke tests lulus;
- unknown command raw byte-exact;
- setiap large fixture mengurangi byte minimum 80%;
- release-mode `x86_64-unknown-linux-gnu` artifact dan installer lulus CI pada
  branch `master`.
```

</details>

<details><summary>Klausul 3, digest ce3cf3e87363</summary>

```text
Phase 2 selesai jika:
```

</details>

<details><summary>Klausul 4, digest 62cc1a262095</summary>

```text
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
```

</details>

## 16. Non-goals

Satu binary crate, modul kecil core/distribution inward; tidak restore historical code, daemon/LLM/network/telemetry/harness premature.

Implementasi: `Cargo.toml`, `src/main.rs`, `src/core/mod.rs`, `src/distribution/mod.rs`.

Test/verifikasi: `tests/cli.rs`, `scripts/check-spec-coverage.py`.

Evidence CI: `rust`, `m9-distribution`.

Klausul yang dipetakan (2 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 95534264b111</summary>

```text
MVP tidak mencoba:
```

</details>

<details><summary>Klausul 2, digest 7bbfdd488665</summary>

```text
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
```

</details>

## 17. Repository layout

Satu binary crate, modul kecil core/distribution inward; tidak restore historical code, daemon/LLM/network/telemetry/harness premature.

Implementasi: `Cargo.toml`, `src/main.rs`, `src/core/mod.rs`, `src/distribution/mod.rs`.

Test/verifikasi: `tests/cli.rs`, `scripts/check-spec-coverage.py`.

Evidence CI: `rust`, `m9-distribution`.

Klausul yang dipetakan (2 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 3dfc190b9c61</summary>

```text
Gunakan satu Rust binary crate dengan modul berdasarkan tanggung jawab: CLI,
invocation/execution, classification, filter families, raw storage, harness
adapter, dan installer integration. Test integration dan fixture dipisahkan dari
source production. Jangan membuat workspace multi-crate, daemon, database, atau
abstraksi harness generik di luar interface minimum yang dibutuhkan Codex.
```

</details>

<details><summary>Klausul 2, digest 8a2eabedffbc</summary>

```text
Struktur boleh berkembang saat implementasi selama dependency tetap satu arah,
filter family dapat diuji terpisah, dan execution engine tidak bergantung pada
harness tertentu.
```

</details>

## 18. Final behavior

Kontrak harness terdokumentasi; entrypoint hook/install/uninstall Codex tidak tersedia pada M9.

Implementasi: `src/cli.rs`.

Test/verifikasi: `tests/cli.rs`.

Evidence CI: `rust`.

Batas tahap: Seluruh kontrak hook, serializer/quoting hook, recursive wrapper, config/trust, auth E2E ephemeral, token reduction, sandbox Codex dan publikasi menunggu M10. Global uninstall sudah dipetakan pada §13.

Klausul yang dipetakan (3 kelompok; seluruh bullet/command di dalamnya):

<details><summary>Klausul 1, digest 42e4102f36de</summary>

```text
Aturan TTC:
```

</details>

<details><summary>Klausul 2, digest 90725c2d6e14</summary>

```text
> Hook selalu membungkus Bash command. TTC hanya memfilter output yang benar-benar dikenali. Semua output lain diteruskan penuh.
```

</details>

<details><summary>Klausul 3, digest 6c5e4d4c66ec</summary>

```text
Jika hook terpasang, Bash command masuk melalui TTC. Jika TTC tidak dapat mengoptimalkan output, pengguna tetap menerima output asli.
```

</details>

