# Sesi 01 — P0 Codex Security dan Runtime Evidence

**Tanggal:** 2026-09-14 10:36 WIB
**Status:** completed

## Yang dikerjakan

Implementasi P0 untuk integrasi Codex fail-closed dan bukti eksekusi runtime selesai. Automatic rewrite tetap nonaktif karena profile kompatibilitas Codex belum terverifikasi.

## Perubahan

- Ditambah: `ExecutionEvidence`, `StageEvidence`, stream counters, dan atribusi nested `unavailable`.
- Diubah: adapter Codex menjadi parser event bertipe, compatibility report schema v1, doctor schema v2, dan hook fail-open.
- Dihapus: dormant `/bin/sh` rewrite serializer yang dapat menyamarkan approval semantics.
- Ditambah: matrix probe Codex terisolasi dan artefak `docs/research/codex-compatibility.json`.
- Diubah: metadata recovery, test execution/core, README, architecture, security, Codex guide, roadmap, support matrix, dan validation artifacts.
- Dipasang ulang: `/home/skirja/.codex/bin/ttc`; konfigurasi Codex tidak berubah.

## Keputusan

- Automatic Codex rewrite fail-closed; tidak ada opt-in tersembunyi atau config switch yang melewati gate.
- Command asli tetap authoritative; TTC tidak menginstrumentasi atau memecah shell untuk membuat evidence nested.
- Evidence hanya mencatat proses top-level yang benar-benar di-spawn; workload graph tidak dianggap bukti stage berjalan.
- PostToolUse tidak dijadikan jalur filtering karena output hook tidak memiliki exit status terstruktur.
- Prefix denial, shell fidelity, permission mode, dan approval semantics tetap dianggap blocker sampai ada kontrak Codex version-specific yang terattest.

## Verifikasi

- `cargo fmt --check` → lulus.
- `cargo clippy --locked --all-targets -- -D warnings` → lulus.
- `cargo test --locked` → 66 test lulus.
- `cargo build --release --locked` → lulus; binary 8,061,016 bytes.
- Python compile check untuk `scripts/codex_probe.py` → lulus.
- `python3 scripts/codex_probe.py` → exit 0, matrix lengkap, verdict `incompatible`; baseline prefix denial diblokir tetapi candidate rewrite menjalankan command yang sama, explicit shell tidak fidelity, PostToolUse exit status tidak terstruktur.
- `target/release/ttc install codex` → idempotent, `config_changed: false`.
- `~/.codex/bin/ttc doctor` → schema 2, hook terdaftar dan executable, compatibility `incompatible`, rewrite `false`.
- Installed `/home/skirja/.codex/bin/ttc run -- cargo test --locked` → seluruh suite lulus, recall raw stdout 4036 bytes dan stderr 549 bytes.
- SQLite check capture final → `execution_evidence.schema_version=1`, nested attribution `unavailable`, stage finished.
- JSON validation untuk seluruh artefak dan `git diff --check` → lulus.
- Dites manual: sebagian — temporary Codex homes/mock Responses server dan installed hook smoke test; native macOS/Windows tidak dites.

## Review

Direview: self-review terhadap diff dan artefak terakhir → APPROVE WITH SUGGESTIONS (tidak ada reviewer eksternal). Setelah review: ditambahkan baseline differential prefix-denial, shell sentinel yang membedakan shell, blocker wording yang lebih akurat, dan final rebuild/reinstall. Kode final berbeda dari pemeriksaan awal tersebut dan telah divalidasi ulang.

## Batasan saat ini

Codex `0.154.0-alpha.6.2` belum memiliki profile aman: candidate rewrite dapat melewati prefix denial, explicit shell tidak terjaga, permission mode probe melaporkan `bypassPermissions`, dan PostToolUse tidak memberi exit status terstruktur. RTK semantic parity serta native macOS/Windows validation masih parsial.

## Next action

Dapatkan atau validasi kontrak Codex yang mempertahankan approval classification, shell, cwd, dan exit status command asli sebelum menambahkan profile kompatibilitas apa pun.
