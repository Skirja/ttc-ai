"""Validate package and stable tag before running the release gates."""
import re
import subprocess
import sys
from pathlib import Path
import tomllib

STABLE = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\Z")


def check(manifest, tag=None, revision="HEAD", master="origin/master"):
    version = tomllib.loads(Path(manifest).read_text())["package"]["version"]
    if not isinstance(version, str) or not STABLE.fullmatch(version):
        raise ValueError("Cargo package version harus stable SemVer")
    if tag is not None:
        if not tag.startswith("v") or not STABLE.fullmatch(tag[1:]) or tag[1:] != version:
            raise ValueError("Tag stable SemVer harus sama dengan Cargo package version")
        subprocess.run(["git", "merge-base", "--is-ancestor", revision, master], cwd=Path(manifest).resolve().parent, check=True)
    return version


if __name__ == "__main__":
    try:
        print(check(sys.argv[1], *sys.argv[2:]))
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(f"ttc release: {error}", file=sys.stderr)
        raise SystemExit(1)
