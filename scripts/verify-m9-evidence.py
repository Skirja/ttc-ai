"""Require all passing prerequisite artifacts and verify their checksum manifests."""
import hashlib
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent
base = Path(sys.argv[1]).resolve()
required = {
    "m2-byte-comparison": "report.txt",
    "m3-streaming-evidence": "report.txt",
    "m4-javascript-evidence": "report.txt",
    "m5-core-ecosystem-evidence": "report.txt",
    "m6-monorepo-evidence": "report.txt",
    "m7-php-jvm-dotnet-evidence": "report.txt",
    "m8-remaining-ecosystem-evidence": "smoke-report.txt",
}
for name, report in required.items():
    directory = base / name
    if not (directory / report).is_file():
        raise SystemExit(f"Required evidence missing: {name}/{report}")
    sums = directory / "SHA256SUMS"
    if name in {"m6-monorepo-evidence", "m7-php-jvm-dotnet-evidence", "m8-remaining-ecosystem-evidence"} and not sums.is_file():
        raise SystemExit(f"Required checksum manifest missing: {sums}")
    if sums.is_file():
        entries = sums.read_text().splitlines()
        if not entries:
            raise SystemExit(f"Empty checksum manifest: {sums}")
        for line in entries:
            digest, relative = line.split(None, 1)
            relative = relative.removeprefix("*")
            path = Path(relative)
            if path.is_absolute() or ".." in path.parts:
                raise SystemExit("Unsafe evidence path")
            if relative.startswith("scripts/"):
                source = ROOT / path
            elif relative.startswith("target/"):
                source = directory / path.name
            else:
                source = directory / path
            if hashlib.sha256(source.read_bytes()).hexdigest() != digest:
                raise SystemExit(f"Evidence checksum mismatch: {source}")
    print(f"{name}: report present" + (", checksum verified" if sums.is_file() else ""))
