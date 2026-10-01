"""Smoke a release binary outside the source tree with isolated state."""
import hashlib
import os
from pathlib import Path
import pty
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import tomllib

repository = Path(__file__).resolve().parent.parent
source = Path(sys.argv[1]).resolve()
version = tomllib.loads((repository / "Cargo.toml").read_text())["package"]["version"]
assert source.read_bytes().startswith(b"\x7fELF")

with tempfile.TemporaryDirectory(prefix="ttc-m9-artifact-") as scratch:
    root = Path(scratch)
    workspace = root / "workspace"
    workspace.mkdir()
    home = root / "home"
    home.mkdir()
    tools = root / "tools"
    tools.mkdir()
    binary = root / "ttc"
    shutil.copyfile(source, binary)
    binary.chmod(0o755)
    env = os.environ.copy()
    env.update({"HOME": str(home), "XDG_DATA_HOME": str(root / "data"),
                "XDG_STATE_HOME": str(root / "state"), "XDG_CONFIG_HOME": str(root / "config"),
                "TMPDIR": str(root), "TTC_INTERNAL_TEST_TMP_ROOT": str(root),
                "PATH": str(tools) + ":" + env["PATH"], "TTC_M9_MARKER": "fixture-env"})

    def run(args, count, stdin=b""):
        scoped = env.copy()
        scoped["COUNT_FILE"] = str(root / count)
        return subprocess.run(args, cwd=workspace, env=scoped, input=stdin, capture_output=True, timeout=20)

    assert run([str(binary), "--version"], "version.count").stdout == f"ttc {version}\n".encode()
    help_result = run([str(binary), "--help"], "help.count")
    assert help_result.returncode == 0 and b"ttc uninstall" in help_result.stdout
    assert b"__install" not in help_result.stdout and b"install codex" not in help_result.stdout
    probe = tools / "custom-probe"
    probe.write_text("#!/bin/sh\nprintf x >> \"$COUNT_FILE\"\nprintf 'cwd=%s env=%s\\n' \"$PWD\" \"$TTC_M9_MARKER\"\nprintf 'arg=<%s>\\n' \"$@\"\ncat\nprintf '\\377\\000tail\\n'\nprintf 'warning: stderr exact\\n' >&2\nexit 13\n")
    probe.chmod(0o755)
    arguments = ["space value", "quote'\"$value", "日本", "--json"]
    stdin = b"stdin\x00\xffexact\n"
    direct = run([str(probe), *arguments], "argv-direct.count", stdin)
    wrapped = run([str(binary), str(probe), *arguments], "argv-ttc.count", stdin)
    assert direct.returncode == wrapped.returncode == 13
    assert direct.stdout == wrapped.stdout and direct.stderr == wrapped.stderr
    assert b"fixture-env" in wrapped.stdout and str(workspace).encode() in wrapped.stdout
    assert (root / "argv-direct.count").read_bytes() == (root / "argv-ttc.count").read_bytes() == b"x"

    command = " ".join(shlex.quote(arg) for arg in [str(probe), *arguments]) + "; status=$?; printf 'shell continuation\\n' >&2; exit $status"
    direct = run(["/bin/sh", "-c", command], "shell-direct.count", stdin)
    wrapped = run([str(binary), command], "shell-ttc.count", stdin)
    assert direct.returncode == wrapped.returncode == 13
    assert direct.stdout == wrapped.stdout and direct.stderr == wrapped.stderr
    assert (root / "shell-direct.count").read_bytes() == (root / "shell-ttc.count").read_bytes() == b"x"

    for signal in [2, 15]:
        args = ["/bin/sh", "-c", f"kill -{signal} $$"]
        direct = run(args, f"signal-{signal}-direct.count")
        wrapped = run([str(binary), *args], f"signal-{signal}-ttc.count")
        assert direct.returncode == wrapped.returncode == -signal

    def terminal(args):
        master, slave = pty.openpty()
        try:
            process = subprocess.Popen(args, cwd=workspace, env=env, stdin=slave, stdout=slave, stderr=slave)
            code = process.wait(timeout=10)
            os.close(slave)
            slave = None
            output = os.read(master, 4096)
            return code, output
        finally:
            os.close(master)
            if slave is not None:
                os.close(slave)

    tty_command = ["/bin/sh", "-c", 'test -t 0 && test -t 1 && test -t 2 && printf "tty inherited\\n"']
    assert terminal(tty_command) == terminal([str(binary), *tty_command]) == (0, b"tty inherited\r\n")

    cargo = tools / "cargo"
    cargo.write_text("#!/bin/sh\nprintf x >> \"$COUNT_FILE\"\ni=0\nwhile [ \"$i\" -lt 1001 ]; do printf 'test fixture_%s ... ok\\n' \"$i\"; i=$((i + 1)); done\nprintf 'test result: ok. 1001 passed; 0 failed; 0 ignored\\n'\nprintf 'warning: keep original diagnostic\\n' >&2\nexit 7\n")
    cargo.chmod(0o755)
    direct = run(["cargo", "test"], "cargo-direct.count")
    wrapped = run([str(binary), "cargo", "test"], "cargo-ttc.count")
    assert direct.returncode == wrapped.returncode == 7
    assert (root / "cargo-direct.count").read_bytes() == (root / "cargo-ttc.count").read_bytes() == b"x"
    assert b"test result: ok. 1001 passed; 0 failed; 0 ignored\n" in wrapped.stdout
    assert wrapped.stderr.startswith(direct.stderr)
    assert 5 * (len(wrapped.stdout) + len(wrapped.stderr)) <= len(direct.stdout) + len(direct.stderr)
    raw = re.search(rb"^raw: ttc raw ([a-zA-Z0-9_-]+)$", wrapped.stderr, re.MULTILINE)
    assert raw is not None
    raw_id = raw.group(1).decode()
    replay_out = run([str(binary), "raw", raw_id, "--stdout"], "replay-out.count")
    replay_err = run([str(binary), "raw", raw_id, "--stderr"], "replay-err.count")
    assert replay_out.returncode == replay_err.returncode == 0
    assert replay_out.stdout == direct.stdout and replay_err.stdout == direct.stderr
    assert not replay_out.stderr and not replay_err.stderr
    print("standalone=pass version=" + version)
    print("argv/shell=byte-exact stdout/stderr cwd/env/stdin invocation-count=1")
    print("signal=SIGINT,SIGTERM tty=inherited diagnostics=retained raw-replay=byte-exact")
    print(f"large-rust bytes={len(direct.stdout)+len(direct.stderr)}/{len(wrapped.stdout)+len(wrapped.stderr)} reduction>=80%")
    print("binary-sha256=" + hashlib.sha256(binary.read_bytes()).hexdigest())
