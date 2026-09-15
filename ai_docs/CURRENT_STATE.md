# Current State

**Diperbarui:** 2026-09-14 WIB

## Sedang dikerjakan

- P0 keamanan dan integrasi Codex — partial; TTC dan jalur rewrite CLI resmi
  sudah diuji, tetapi automatic production rewrite dan pengurangan token
  model-facing belum dinyatakan selesai.

## Terakhir selesai

- TTC release `0.1.0` berhasil dibangun dan dipasang ke
  `/home/skirja/.codex/bin/ttc`.
- Adapter menerima kontrak Codex resmi `allow + updatedInput` dan tetap
  fail-closed tanpa profile production.
- CLI terpisah `codex-cli 0.154.0` terbukti menerima wrapper TTC pada profile
  sementara; command sekali jalan dan exit code `7` dipertahankan.
- Dokumentasi eksperimen lengkap ditulis di
  `docs/experiment-codex-ttc-automatic-filtering.md`.
- Kode tersentuh: `src/integrations.rs`, `src/command.rs`,
  `src/execution.rs`, `src/main.rs`, `tests/execution.rs`, `build.rs`,
  `profiles/`, dan script probe/profile.

## Keputusan yang dikunci

- Automatic Codex rewrite harus fail-closed; tidak ada bypass tersembunyi.
- Kontrak production memakai `permissionDecision: "allow"` +
  `updatedInput.command`; `permissionDecision: "rewrite"` hanya eksperimen.
- Profile exact hanya boleh dipublikasikan setelah approval, shell, cwd, login,
  sandbox, competing hooks, exit status, dan model-output lulus.
- Command/argv/shell asli tetap authoritative dan hanya dijalankan sekali.
- `ExecutionEvidence` hanya menyatakan fakta proses top-level yang benar-benar
  di-spawn; nested workload stage tetap `unavailable`.
- Unknown, dynamic, interactive, pipeline, privileged, dan machine-output paths
  tetap passthrough.
- Recovery tetap lokal/private; raw output dan diagnostic tidak boleh dihilangkan
  demi ringkasan.
- Manual `ttc run` bukan pengganti acceptance automatic Codex filtering.
- Codex Desktop `/usr/lib/chatgpt/resources/codex` tidak disentuh.

## Temuan / blocker terbuka

- `profiles/codex.json` production masih kosong.
- `ttc doctor` dengan `codex-cli 0.154.0` menunjukkan
  `automatic_rewrite_active: false`.
- E2E sementara membuktikan wrapper dan exit status, tetapi synthetic
  model-output token reduction belum lulus.
- Shell/login/permission fidelity belum cukup untuk mempromosikan profile.
- Codex Desktop belum diuji sebagai jalur automatic rewrite.
- Belum ada formal code review atau commit; branch masih memiliki perubahan
  uncommitted.

## Batasan yang diketahui

- Linux tervalidasi; native macOS/Windows ACL, quoting, signals, dan installer
  belum dijalankan.
- RTK semantic parity, structured ecosystem parsers, dan resolver semantics
  kompleks masih parsial.
- Hook tidak dapat memulihkan output yang sudah dipotong Codex sebelum
  `PostToolUse`.
- Clone/source/build Codex upstream dan semua artifact probe sementara sudah
  dihapus; cache dependency umum Cargo tidak dihapus.

## Next action

- Tentukan apakah melanjutkan automatic Codex integration dengan profile resmi
  yang lebih sederhana, atau memangkas proyek menjadi TTC standalone/wrapper
  Python untuk filtering manual.

## Arsip terakhir

- `ai_docs/steps_done/03-codex-cli-rewrite-experiment.md`
