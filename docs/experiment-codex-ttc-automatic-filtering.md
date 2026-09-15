# Eksperimen Automatic Bash Filtering Codex dengan TTC

**Tanggal:** 2026-09-14 WIB  
**Status:** partial / automatic rewrite belum berhasil  
**Tujuan:** mengevaluasi apakah TTC dapat mengubah command Bash Codex menjadi wrapper TTC, sehingga output test yang panjang dikompaksi sebelum masuk ke konteks model.

## Kesimpulan singkat

Filtering TTC sendiri sudah berjalan dan binary release berhasil dibuat. Filtering manual melalui `ttc run` dapat mengompaksi output test dan tetap mempertahankan diagnostic serta exit status.

Automatic rewrite melalui hook Codex Desktop belum berjalan. Namun CLI brew
`codex-cli 0.154.0` kemudian terbukti menerima kontrak resmi Codex
`permissionDecision: "allow"` + `updatedInput` dan menjalankan wrapper TTC pada
uji sementara. Profile production tetap kosong setelah uji, sehingga rewrite
otomatis belum dibiarkan aktif secara permanen.

Dengan demikian, command yang dijalankan langsung oleh Codex masih:

    npm run test

bukan:

    ttc run ... --command "npm run test"

Akibatnya output otomatis dari Codex belum terfilter.

## Ekspektasi awal

Alur yang ingin dicapai:

    Codex meminta npm run test
            ↓
    PreToolUse hook mengubah command menjadi wrapper TTC
            ↓
    TTC menjalankan npm run test tepat satu kali
            ↓
    TTC menangkap stdout/stderr dan menyimpan raw recall
            ↓
    Output passing/progress yang repetitif dikompaksi
            ↓
    Diagnostic, error, warning, dan output tidak dikenal diteruskan
            ↓
    Codex menerima output yang lebih pendek

Untuk test yang sukses, TTC tidak membuat ringkasan sukses palsu. Record passing yang dikenali dapat dikompaksi dan TTC dapat menampilkan jumlah record/progress yang dikompaksi. Untuk test gagal, error dan exit code asli tetap diteruskan.

## Yang diubah di repo TTC

Branch kerja: `codex/ttc-safe-hook-rewrite`  
Repo: `/home/skirja/Work/Personal/ttc-ai`

Perubahan utama:

- `Invocation::Shell` mendapat field `login`.
- `ttc run --login` ditambahkan; mode POSIX memakai `-lc`, mode biasa tetap memakai `-c`.
- Adapter Codex menerima event resmi Codex tanpa protocol metadata tambahan, dan
  tetap menerima event protocol v2 dengan `execution_context` untuk eksperimen
  lintas-repo.
- Parsing event dibuat typed dan fail-closed untuk field hilang, protocol tidak cocok, context invalid, input terlalu besar, atau shell unsupported.
- Ditambahkan `CodexCompatibilityProfile` dengan exact Codex version, platform, protocol version, serta invariant approval, shell, cwd, login, sandbox, competing hooks, model output, dan exit status.
- Profile dikompilasi ke binary TTC. Profile produksi saat ini kosong:

    {"schema_version":1,"profiles":[]}

- Rewrite hanya boleh terjadi jika konfigurasi TTC, recovery, hook, profile, shell POSIX, sandbox non-privileged, klasifikasi command, dan risk memenuhi syarat.
- Command TTC tidak boleh direwrite kembali untuk mencegah recursive wrapper.
- TTC mengevaluasi command asli dan command hasil rewrite; policy paling ketat tetap berlaku.
- Raw output disimpan untuk recall; command hanya dieksekusi sekali.
- Output rewrite mengikuti kontrak resmi Codex: `permissionDecision: "allow"`
  bersama `updatedInput.command`; nilai custom `permissionDecision: "rewrite"`
  bukan format production Codex.
- `doctor` membedakan `registered`, `compatible`, `enabled`, dan `automatic_rewrite_active`.
- Ditambahkan script staging/promosi profile dan probe compatibility.

File utama yang disentuh:

- `src/command.rs`
- `src/execution.rs`
- `src/integrations.rs`
- `src/main.rs`
- `tests/execution.rs`
- `build.rs`
- `profiles/codex.json`
- `scripts/codex_probe.py`
- `scripts/stage_codex_profile.py`
- `scripts/promote_codex_profile.py`
- `scripts/requirements-codex-probe.txt`

## Perubahan Codex upstream yang sempat dicoba

Karena rencana awal mencakup perubahan lintas repo, source Codex upstream sempat di-clone ke:

    /home/skirja/Work/Personal/codex

Perubahan percobaan di repo tersebut meliputi:

- keputusan hook baru `permissionDecision: "rewrite"`;
- `updatedInput.command`;
- `execution_context` pada event hook;
- preserving rewrite yang menyimpan command asli untuk evaluasi policy;
- lattice keputusan `deny > approval/escalation > allow`;
- preservasi shell, login, cwd, sandbox, argv, environment, output, exit code, dan signal;
- generated hook schema dan unit test terkait;
- approval test untuk command asli versus command hasil rewrite.

Codex CLI development berhasil dibangun dan memiliki versi:

    codex-cli 0.0.0

Binary development tersebut memang sempat dijalankan oleh probe lokal dengan fake Responses server dan command sentinel. Probe tidak menggunakan Codex Desktop pengguna, tidak mengubah project pengguna, dan tidak menjalankan command destruktif.

Setelah pengguna meminta cleanup, seluruh clone/source/build Codex tersebut dihapus. Tidak ada source atau executable dari clone itu yang tersisa.

## Hasil compatibility probe

Probe kandidat berjalan terhadap binary Codex development dan menghasilkan verdict `incompatible`.

Invariant yang berhasil:

- protocol v2 dan execution context tersedia pada binary kandidat;
- command aman dieksekusi tepat satu kali;
- prefix denial tetap tidak dieksekusi;
- cwd tetap sama;
- sandbox invariant dasar tetap sama;
- unknown command tetap passthrough;
- deny dari competing hook tetap menang;
- exit code `7` tetap diteruskan.

Invariant yang gagal:

- explicit shell fidelity;
- login shell fidelity;
- permission mode behavior pada probe;
- model-facing output fidelity.

Pada kasus model-output sintetis, baseline dan rewrite sama-sama tercatat sekitar 4212 bytes / 1177 token `o200k_base`. Jadi acceptance token reduction belum terbukti.

Production-hook E2E dengan profile terpromosi tidak dijalankan karena kandidat sendiri sudah incompatible dan profile `0.0.0` tidak aman untuk dipublikasikan.

### Uji tambahan dengan Codex CLI resmi 0.154.0

Setelah CLI terpisah dipasang dari Homebrew:

    /home/linuxbrew/.linuxbrew/bin/codex
    codex-cli 0.154.0

Adapter TTC diubah ke kontrak resmi `allow + updatedInput`. Dengan profile
sementara yang hanya dipakai saat eksperimen, direct hook menghasilkan wrapper:

    {"permissionDecision":"allow","updatedInput":{"command":"... ttc run ..."}}

Production probe terhadap fake Responses server menunjukkan:

- hook dipanggil oleh CLI resmi;
- command aman dieksekusi tepat satu kali;
- command dengan `exit 7` tetap menghasilkan exit code `7`;
- prefix denial tetap tidak menjalankan sentinel;
- wrapper diterima dan dijalankan oleh CLI `0.154.0`.

Uji filtering langsung dengan binary release TTC menghasilkan output ringkas:

    TTC: 1 passing records, 0 progress lines compacted
    raw: ttc recall 8928896ffbcac0b20f21e6949660711d

Synthetic model-output fixture pada probe belum menunjukkan pengurangan token,
karena fixture tersebut tidak menghasilkan format output yang dikenali resolver
pada jalur fake project. Jadi model-facing token reduction production tetap
belum dianggap lulus.

## Build dan verifikasi TTC

Perintah yang benar-benar dijalankan:

    cargo fmt --check
    cargo test --locked
    cargo build --release --locked
    cargo clippy --locked --all-targets -- -D warnings

Hasil:

- format: lulus;
- test: lulus, 7 unit integrasi, 43 core, 14 execution, 1 golden, 6 structured, dan doc-tests;
- release build: lulus;
- strict Clippy: lulus.

Binary release:

    /home/skirja/Work/Personal/ttc-ai/target/release/ttc

Versi:

    ttc 0.1.0

Hash binary release dan binary terpasang sama:

    8735ab684ff81abb76f139162bbf0570bacd4a7919bd09eb0adbf28000e889d9

Smoke test `ttc run` berhasil menjalankan command dan meneruskan output. Filtering test record juga berhasil diuji secara langsung melalui `ttc run`.

## Setup hook pada Codex Desktop

Codex Desktop yang terpasang berada di:

    /usr/lib/chatgpt/resources/codex

Path tersebut diverifikasi sebagai proses Codex Desktop aktif dan tidak diubah, dihapus, atau diganti.

Codex Desktop terdeteksi sebagai:

    codex-cli 0.154.0-alpha.6.2

TTC release dipasang ke:

    /home/skirja/.codex/bin/ttc

Hook Codex sudah terdaftar dengan command:

    '/home/skirja/.codex/bin/ttc' hook codex

Opt-in diaktifkan pada dua sisi:

- `/home/skirja/.codex/config.toml` mendapat `[hooks] enabled = true`;
- `/home/skirja/.config/ttc-ai/config.toml` dibuat dengan `[hooks] enabled = true`.

Status `ttc doctor` setelah setup:

    ttc_hook_enabled: true
    hook_registered: true
    hook_executable_works: true
    compatible: false
    automatic_rewrite_active: false

Smoke test hook untuk event legacy dan event v2 sama-sama menghasilkan:

    {}

Itu adalah perilaku fail-closed yang diharapkan ketika profile Codex exact belum terverifikasi. Konsekuensinya, hook belum mengubah command dan output Codex masih raw.

## Cleanup yang sudah dilakukan

Atas permintaan pengguna, semua artifact dari build Codex development dihapus:

- `/home/skirja/Work/Personal/codex` — clone, source, target, dan executable;
- `/tmp/ttc-codex-probe-venv` — virtualenv probe;
- `/tmp/ttc-codex-contract-report.json` — report probe sementara;
- `scripts/__pycache__` — cache Python sementara.

Tidak ada proses build Codex yang tersisa. Cache dependency umum Cargo tidak dihapus karena bukan source/executable Codex dan dapat dipakai oleh project lain.

## Mengapa implementasi terasa overengineered

Filtering output saja memang dapat dibuat jauh lebih sederhana, misalnya dengan Python wrapper yang menjalankan command, membaca stdout/stderr, lalu menyaring baris tertentu. Pendekatan itu cukup untuk eksperimen pengurangan token manual.

Kompleksitas bertambah karena target awal bukan hanya filtering, tetapi automatic rewrite yang aman di dalam Codex. Agar wrapper tidak mengubah arti command, implementasi mencoba mempertahankan sekaligus membuktikan:

- approval/prefix rule command asli;
- sandbox dan escalation;
- shell executable dan login mode;
- cwd efektif;
- environment dan argv;
- exit code dan signal;
- command hanya dieksekusi sekali;
- deny dari hook lain tetap menang;
- raw recall tetap tersedia;
- command unknown atau berisiko tetap passthrough.

Kontrak hook Codex yang tersedia saat ini belum menyediakan seluruh metadata dan semantik `rewrite` tersebut. Karena itu adapter yang aman tidak boleh sekadar mengganti string command lalu mengklaim selesai.

## Status akhir untuk keputusan berikutnya

Yang sudah terbukti:

- TTC dapat dibangun sebagai Rust binary release;
- TTC dapat menjalankan command dan memfilter output secara manual;
- test suite TTC dan strict Clippy lulus;
- hook dapat dipasang dan dijalankan;
- Codex Desktop tidak disentuh;
- command/output asli tetap aman ketika compatibility tidak terverifikasi.

Yang belum terbukti:

- Codex Desktop otomatis mengubah `npm run test` menjadi wrapper TTC;
- Codex Desktop menggunakan wrapper yang sama seperti CLI brew;
- output yang diterima model benar-benar output hasil filter TTC;
- token model-facing berkurang pada jalur production hook;
- exact-version production profile untuk Codex Desktop.

Pilihan realistis jika eksperimen dilanjutkan:

1. tetap memakai TTC manual (`ttc run`) dan menyederhanakan automatic hook;
2. membuat wrapper Python sederhana khusus untuk kebutuhan filter output;
3. melanjutkan perubahan upstream Codex dan compatibility E2E sampai protocol v2 resmi tersedia;
4. tidak melanjutkan automatic integration dan mempertahankan TTC sebagai CLI standalone.

Untuk kondisi sekarang, automatic rewrite belum boleh disebut selesai atau aktif.
