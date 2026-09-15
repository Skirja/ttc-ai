# Sesi 03 — Eksperimen Rewrite Codex CLI Resmi

**Tanggal:** 2026-09-14 WIB  
**Status:** partial

## Yang dikerjakan

- Mengubah adapter TTC agar menerima event Codex resmi tanpa metadata protocol
  tambahan, tetap menerima event v2 eksperimental, dan menghasilkan
  `permissionDecision: "allow"` bersama `updatedInput.command`.
- Menguji CLI terpisah `/home/linuxbrew/.linuxbrew/bin/codex` versi
  `codex-cli 0.154.0`.
- Memasang TTC release ke `/home/skirja/.codex/bin/ttc` dan mengaktifkan hook
  TTC/Codex untuk pengujian.
- Menjalankan E2E sementara dengan fake Responses server dan profile sementara.
- Mengembalikan binary terpasang ke profile production kosong setelah pengujian.

## Perubahan

- Ditambah/diubah: `src/integrations.rs` untuk kontrak resmi `allow` +
  `updatedInput`, fallback event resmi, dan test event Codex tanpa context.
- Ditambah: `docs/experiment-codex-ttc-automatic-filtering.md` sebagai dokumentasi
  eksperimen lengkap.
- Diubah konfigurasi pengguna: `/home/skirja/.codex/config.toml` memiliki
  `[hooks] enabled = true`; `/home/skirja/.config/ttc-ai/config.toml` memiliki
  `[hooks] enabled = true`.
- Dihapus setelah eksperimen: profile sementara, virtualenv/report probe, dan
  seluruh clone/build Codex upstream `/home/skirja/Work/Personal/codex`.
- Tidak diubah: binary Codex Desktop `/usr/lib/chatgpt/resources/codex`.

## Keputusan

- Kontrak production mengikuti Codex resmi `permissionDecision: "allow"` +
  `updatedInput`; `permissionDecision: "rewrite"` diperlakukan sebagai format
  eksperimental, bukan output production.
- Profile production TTC tetap kosong dan automatic rewrite tetap fail-closed.
- Hook boleh terdaftar/enabled, tetapi tidak boleh rewrite tanpa compatibility
  evidence lengkap.
- Manual `ttc run` tetap dianggap berhasil secara terpisah dari acceptance
  automatic Codex filtering.

## Verifikasi

- `cargo fmt --check` — lulus.
- `cargo test --locked` — lulus: 7 unit integrasi, 43 core, 14 execution, 1
  golden, 6 structured, dan doc-tests.
- `cargo build --release --locked` — lulus.
- `cargo clippy --locked --all-targets -- -D warnings` — lulus.
- `ttc doctor` dengan CLI `0.154.0` — hook terdaftar/enabled, profile production
  kosong, `automatic_rewrite_active: false`.
- Direct hook resmi dengan profile sementara — menghasilkan `allow + updatedInput`
  dan wrapper TTC.
- E2E CLI `0.154.0` dengan profile sementara — command sentinel dijalankan sekali,
  exit code `7` dipertahankan, dan prefix denial tidak dieksekusi.
- `ttc run` langsung — output test dikompaksi dan raw recall dibuat.
- Synthetic model-output token reduction — belum lulus; fixture fake tidak
  menghasilkan output yang terfilter lebih pendek.
- `git diff --check` — lulus.

## Review

Direview: self-check final diff dan gate lokal → **NEEDS CHANGES** karena
production compatibility profile dan model-facing token reduction belum terbukti.
Tidak ada formal code review atau commit. Setelah verifikasi awal, adapter
diubah lagi untuk mendukung kontrak resmi Codex; kode final adalah versi setelah
perubahan tersebut.

## Batasan saat ini

- Automatic rewrite production masih nonaktif.
- Profile exact untuk `codex-cli 0.154.0` belum boleh dipromosikan karena invariant
  shell/login/permission dan model-output belum lengkap.
- Codex Desktop belum diuji sebagai jalur rewrite; Desktop menggunakan binary
  bundled terpisah dan tidak disentuh.
- Output yang diterima model belum terbukti lebih pendek pada E2E final.
- Dukungan ecosystem/filter belum setara penuh dengan RTK.

## Next action

Tentukan apakah proyek dilanjutkan dengan profile/adapter Codex resmi yang lebih
sederhana, atau dipangkas menjadi CLI/wrapper Python standalone untuk filtering
manual.
