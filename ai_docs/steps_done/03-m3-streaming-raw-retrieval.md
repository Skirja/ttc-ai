# Sesi 03 — M3 streaming safety dan raw retrieval

**Tanggal:** 2026-09-24 WIB  
**Status:** completed

## Yang dikerjakan

M3 selesai pada branch `feat/m3-streaming-raw`. Pipeline memakai bounded channel,
framing per stream, config opsional, capture hingga 32 MiB, dan `ttc raw`.
[PR #2](https://github.com/Skirja/ttc-ai/pull/2) terbuka; tiga job pada
[CI run 35976973682](https://github.com/Skirja/ttc-ai/actions/runs/35976973682)
lulus untuk HEAD `70d6d21edc5f318470603e10a527ca60524f5fad`.

## Perubahan

- Ditambah: `src/core/config.rs`, `src/core/streaming.rs`, lima suite M3, dan
  `scripts/m3-evidence.sh`.
- Diubah: `src/core/raw_store.rs`, `src/core/execution.rs`, `src/cli.rs`,
  `.github/workflows/ci.yml`, tes M2 untuk isolasi HOME/state/temp, SPEC, TODO,
  dan README.
- Dihapus: tidak ada.

## Keputusan

- Satu binary crate dan versi `0.1.0` tetap berlaku; dependency `toml` dipakai
  untuk parsing config, sedangkan `nix` dan `signal-hook` tetap menangani sinyal.
- `max_raw_mb` maksimum 32 dan `retention_hours` maksimum 24. Config invalid
  membuat command berjalan raw. `--tail N` menghitung byte; selector raw menulis
  hasil ke stdout.
- Fallback capture memakai `/tmp/ttc-<uid>/runs` yang stabil antar-invocation.
  Tes dan skrip bukti mengarahkannya ke direktori sementara milik tes.
- Capture memakai jurnal undo dan dua header agar versi terakhir yang valid
  tetap dapat direplay setelah append gagal selama file dapat dibaca; file
  `.part` dapat direplay jika finalisasi gagal. Output berikutnya berjalan raw
  tanpa rerun.
- Jalur `exec` untuk `&` yang terlihat pada argv tetap berlaku. Branch fitur
  dipush sebagai PR ke `master`; pengguna yang melakukan merge. Repository
  `Skirja/ttc-ai` privat saat M2 dan visibilitasnya tidak diubah pada sesi ini.
- State lama tentang PR #1 yang menunggu merge tidak berlaku lagi: PR #1 sudah
  di-merge ke `master` sebagai `e2c97b5ec43ada7256ff9acd4c0af2c84c1f9bf2`.
  Temuan review M2 lama tidak dibawa sebagai blocker M3 karena perbaikan sudah
  masuk merge tersebut dan regresi M2 lulus kembali.

## Verifikasi

- `cargo test --test streaming`, `cargo test --test raw_store`,
  `cargo test --test raw_cli`, `cargo test --test storage_failure`, dan
  `cargo test --test config`: lulus.
- `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test --all-targets --all-features`, dan `cargo build --release`: lulus.
- `sh scripts/m3-evidence.sh target/m3-evidence/report.txt`: lulus; peak RSS
  2.900 KiB untuk 64 MiB dan 2.960 KiB untuk 256 MiB output. Skrip
  `sh scripts/m2-evidence.sh target/m2-evidence/report.txt` juga lulus.
- CI PR #2: run `35976764504` lulus untuk fix `d5c00ca58f4b6e4f3644ffbc782c4372d6fed035`;
  run `35976973682` lulus untuk HEAD dokumentasi terakhir.
- Dites manual: tidak; verifikasi memakai tes otomatis, skrip bukti, dan CI.

## Review

Direview: delta `master...7527541` → NEEDS CHANGES (tiga temuan: capture
setelah kegagalan storage, lookup fallback saat XDG `EACCES`, dan lokasi
fallback yang berubah mengikuti `TMPDIR`). Setelah review: commit `d5c00ca`
memperbaiki ketiganya dan menambah tes kegagalan; kode final belum direview
ulang secara formal.

## Batasan saat ini

- Recognizer produksi baru dikerjakan pada M4. M3 tetap raw; capture positif
  diuji memakai filter sintetis internal.
- `target/release/ttc` adalah artifact lokal yang tidak di-commit.
- PR #2 belum di-merge; catatan sesi ini dan `CURRENT_STATE.md` masih perubahan
  lokal yang belum di-commit.

## Next action

Commit dan push kedua catatan state lokal ini ke branch `feat/m3-streaming-raw`.
