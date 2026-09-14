# Current State

**Diperbarui:** 2026-09-14 WIB

## Sedang dikerjakan

- P0 keamanan dan integrasi Codex — partial; baseline keamanan selesai, tetapi
  automatic Bash rewrite dan model-facing output filtering belum berjalan.

## Terakhir selesai

- Definition of Done automatic Codex filtering diselaraskan ke README, status/roadmap,
  Codex guide, implementation report, architecture, support, configuration, dan
  contributing docs; hasil produk automatic filtering tetap belum selesai.
- Kode tersentuh: tidak ada; perubahan hanya dokumentasi dan `ai_docs/`.

## Keputusan yang dikunci

- Automatic Codex rewrite harus fail-closed; tidak ada opt-in tersembunyi atau bypass konfigurasi.
- Definisi selesai integrasi Codex: command Bash eligible harus benar-benar di-rewrite
  otomatis melalui TTC dan model-facing output harus terbukti terfilter sehingga
  token berkurang; hook registered/trusted yang hanya passthrough tidak dihitung
  sebagai selesai atau berjalan.
- Penggunaan manual `ttc run` bukan pengganti acceptance integrasi Codex otomatis.
- Command/argv/shell asli tetap authoritative dan hanya dijalankan sekali.
- `ExecutionEvidence` hanya menyatakan fakta proses top-level yang benar-benar di-spawn; nested workload stage tetap `unavailable`.
- Unknown, dynamic, interactive, pipeline, privileged, dan machine-output paths tetap passthrough.
- Recovery tetap lokal/private; raw output dan diagnostic tidak boleh dihilangkan demi ringkasan.

## Temuan / blocker terbuka

- Codex `0.154.0-alpha.6.2` incompatible: candidate rewrite melewati prefix denial baseline.
- Explicit shell fidelity gagal pada candidate rewrite; permission mode probe tidak sesuai mode yang diminta.
- PostToolUse hanya memberi raw string tanpa exit status terstruktur.
- Belum ada kontrak/version profile yang mengattest approval equivalence untuk command asli.
- Hook produksi saat ini hanya mengembalikan `{}` pada `PreToolUse`; tidak ada
  automatic rewrite atau output filtering di Codex.

## Batasan yang diketahui

- Linux tervalidasi; native macOS/Windows ACL, quoting, signals, dan installer belum dijalankan.
- RTK semantic parity, structured ecosystem parsers, dan resolver semantics kompleks masih parsial.
- Hook tidak dapat memulihkan output yang sudah dipotong Codex sebelum PostToolUse.

## Next action

- Implementasikan dan validasi jalur end-to-end versioned yang me-rewrite command Bash
  eligible melalui TTC sambil mempertahankan approval, shell, cwd, dan exit status;
  buktikan model-facing output terfilter dan token berkurang.

## Arsip terakhir

- `ai_docs/steps_done/02-clarify-automatic-filtering-acceptance.md`
