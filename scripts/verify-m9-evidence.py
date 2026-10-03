"""Require the combined smoke artifacts and verify every checksum manifest."""
from __future__ import annotations

import hashlib
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent
base = Path(sys.argv[1]).resolve()
required = {
    "rust-safety-evidence": (
        "rust-suite.txt",
        "passthrough/report.txt",
        "streaming/report.txt",
    ),
    "javascript-monorepo-evidence": (
        "javascript/report.txt",
        "monorepo/report.txt",
    ),
    "rust-python-go-evidence": ("report.txt",),
    "php-jvm-dotnet-evidence": ("report.txt", "php-build.log"),
    "build-ruby-swift-infrastructure-evidence": ("smoke-report.txt",),
}


def verify_manifest(directory: Path, manifest: Path) -> set[str]:
    entries = manifest.read_text(encoding="utf-8").splitlines()
    if not entries:
        raise SystemExit(f"Empty checksum manifest: {manifest}")
    verified: set[str] = set()
    for line in entries:
        try:
            digest, relative = line.split(None, 1)
        except ValueError as error:
            raise SystemExit(f"Malformed checksum entry in {manifest}") from error
        relative = relative.removeprefix("*")
        path = Path(relative)
        if path.is_absolute() or ".." in path.parts:
            raise SystemExit(f"Unsafe evidence path in {manifest}")
        source = directory / path
        if not source.is_file():
            raise SystemExit(f"Evidence file missing from checksum manifest: {source}")
        if hashlib.sha256(source.read_bytes()).hexdigest() != digest:
            raise SystemExit(f"Evidence checksum mismatch: {source}")
        verified.add(path.as_posix())
    return verified


rust_suite = base / "rust-safety-evidence" / "rust-suite.txt"
if not rust_suite.is_file():
    raise SystemExit(f"Required full Rust suite log missing: {rust_suite}")
rust_suite_text = rust_suite.read_text(encoding="utf-8")
missing_targets = [
    f"Running tests/{test_file.name}"
    for test_file in sorted((ROOT / "tests").glob("*.rs"))
    if f"Running tests/{test_file.name}" not in rust_suite_text
]
if missing_targets:
    raise SystemExit("Full Rust suite log omits test targets: " + ", ".join(missing_targets))

for name, reports in required.items():
    directory = base / name
    if not directory.is_dir():
        raise SystemExit(f"Required passing evidence artifact missing: {name}")
    for report in reports:
        if not (directory / report).is_file():
            raise SystemExit(f"Required evidence missing: {name}/{report}")
    manifest = directory / "EVIDENCE-SHA256SUMS"
    if not manifest.is_file():
        raise SystemExit(f"Required checksum manifest missing: {manifest}")
    verified = verify_manifest(directory, manifest)
    for report in reports:
        if report not in verified:
            raise SystemExit(f"Evidence is not covered by its checksum manifest: {name}/{report}")
    for nested_manifest in directory.rglob("SHA256SUMS"):
        verify_manifest(nested_manifest.parent, nested_manifest)
    print(f"{name}: {len(reports)} reports and checksum manifests verified")
