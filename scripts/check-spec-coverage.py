"""Check reviewed SPEC mapping, references and gate coverage; emit auditable report.

This gate detects stale/unmapped contracts, not semantic correctness. Rust and
real-tool jobs provide behavioral evidence; master completion still needs review.
"""
import hashlib
import json
from pathlib import Path
import re
import sys

root = Path(__file__).resolve().parent.parent
spec = (root / "ai_docs/SPEC.md").read_text()
manifest = json.loads((root / "ai_docs/m9-coverage.json").read_text())
if hashlib.sha256(spec.encode()).hexdigest() != manifest["spec_sha256"]:
    raise SystemExit("SPEC berubah: audit mapping dan perbarui digest sebelum gate lulus")
sections = manifest["sections"]
workflow = (root / ".github/workflows/ci.yml").read_text()
jobs = set(re.findall(r"^  ([a-z][a-z0-9-]+):$", workflow.split("jobs:\n", 1)[1], re.M))
parts = re.split(r"(^#{2,3} .+$)", spec, flags=re.M)[1:]
seen = set()
rows = []
for index in range(0, len(parts), 2):
    heading = parts[index].lstrip("# ")
    key = heading.split(" —")[0] if heading.startswith("Phase") else heading.split()[0].rstrip(".")
    if key not in sections:
        raise SystemExit(f"SPEC requirement tanpa mapping: {heading}")
    seen.add(key)
    record = sections[key]
    if not record["perilaku"] or not record["implementasi"] or not record["test"] or not record["gate"]:
        raise SystemExit(f"Mapping kosong: {heading}")
    for filename in record["implementasi"] + record["test"]:
        if not (root / filename).is_file():
            raise SystemExit(f"Referensi mapping tidak tersedia: {filename}")
    if not set(record["gate"]) <= jobs:
        raise SystemExit(f"Gate mapping tidak tersedia: {heading}")
    clauses = [block for block in re.split(r"\n\s*\n", parts[index + 1].strip()) if block.strip()]
    rows.append((heading, record, clauses))
if seen != set(sections):
    raise SystemExit("Mapping memiliki section yang tidak ada pada SPEC")

print("# Audit coverage standalone M9\n")
print("Laporan dihasilkan oleh `python3 scripts/check-spec-coverage.py`; mapping yang\ndireview ada pada `ai_docs/m9-coverage.json`. Setiap section, command/form,\ndan kelompok klausul SPEC memiliki pemetaan source/test/gate. Digest mengunci\nSPEC yang diaudit; perubahan kontrak memerlukan audit ulang. Gate ini mengecek\nkelengkapan pemetaan, bukan membuktikan semantik implementasi dengan pencarian\nstring. Bukti perilaku berasal dari test dan pinned real-tool smoke pada run\nyang dicatat di TODO serta artifact kandidat yang sama.\n")
print(f"SPEC SHA-256: `{manifest['spec_sha256']}`.\n")
print("## Hasil audit dan batas penerimaan\n")
print("Core execution/filter M1–M8 dipertahankan. Gap distribusi ditutup oleh\nfinalizer lokal, installer checksum, transaksi ownership, PATH backup/fallback,\nuninstall, tag policy dan smoke ELF di luar source. Pertukaran atomik config\nmempertahankan file aktual yang tergeser; konflik sesudah validasi terakhir\nmenyimpan kedua versi dan marker recovery, termasuk pada rollback/uninstall.\nFeature fs dari nix terpin menyediakan renameat2 tanpa unsafe code di TTC. SHA-256 memakai dependency produksi\n`sha2 = =0.10.9` agar validasi ownership tidak mengeksekusi binary existing\natau bergantung utility dari PATH. Seluruh failure installer menggunakan\nHOME/XDG temporary; mock HTTP loopback memerlukan opt-in eksplisit.\n\nM10 hanya mencakup kontrak harness, auth E2E/sandbox/token dan publikasi;\ntidak ada command Codex yang diiklankan pada M9. Final acceptance M9 tetap\nmemerlukan clean successful master run dan artifact yang diunduh sesudah\nmerge pengguna. Laporan ini tidak menyatakan gate master telah lulus.\n")
for heading, record, clauses in rows:
    print(f"## {heading}\n")
    print(record["perilaku"] + "\n")
    for label, field in (("Implementasi", "implementasi"), ("Test/verifikasi", "test"), ("Evidence CI", "gate")):
        print(label + ": " + ", ".join(f"`{value}`" for value in record[field]) + ".\n")
    if record["cakupan_m10"]:
        print("Batas tahap: " + record["cakupan_m10"] + "\n")
    print(f"Klausul yang dipetakan ({len(clauses)} kelompok; seluruh bullet/command di dalamnya):\n")
    for number, clause in enumerate(clauses, 1):
        digest = hashlib.sha256(clause.encode()).hexdigest()[:12]
        # Full literal text keeps every command and failure condition inspectable.
        print(f"<details><summary>Klausul {number}, digest {digest}</summary>\n")
        print("```text\n" + clause + "\n```\n\n</details>\n")
