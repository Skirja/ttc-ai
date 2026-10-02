"""Distribution E2E: isolated HOME/XDG and a loopback release server."""
import hashlib
import http.server
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
BINARY = Path(sys.argv.pop(1)).resolve()
ASSET = "ttc-x86_64-unknown-linux-gnu"
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
IMAGE = BINARY.read_bytes()
DIGEST = hashlib.sha256(IMAGE).hexdigest()


class Release(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self):
        super().__init__(("127.0.0.1", 0), Handler)
        self.latest = "v" + VERSION
        self.requests = []
        self.bad_checksum = False
        self.duplicate = False
        self.broken = False
        self.switch = False


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_GET(self):
        server = self.server
        server.requests.append(self.path)
        if self.path == "/releases/latest":
            self.send_response(302)
            self.send_header("Location", "/releases/tag/" + server.latest)
            self.end_headers()
            return
        if self.path.startswith("/releases/tag/"):
            data = b"release page"
        elif self.path.endswith("/" + ASSET):
            data = IMAGE
            if server.switch:
                server.latest = "v99.99.99"
        elif self.path.endswith("/SHA256SUMS"):
            digest = "0" * 64 if server.bad_checksum else DIGEST
            data = f"{digest}  {ASSET}\n".encode()
            if server.duplicate:
                data += data
        else:
            self.send_error(404)
            return
        self.send_response(200)
        length = len(data) + 100 if server.broken and self.path.endswith(ASSET) else len(data)
        self.send_header("Content-Length", str(length))
        self.end_headers()
        try:
            self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError):
            pass


def encode(metadata):
    return "\n".join(f"{key} = {json.dumps(value, ensure_ascii=False)}" for key, value in metadata.items()) + "\n"


class InstallTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="ttc-m9-install-")
        self.root = Path(self.scratch.name)
        self.home = self.root / "home space ' 日本"
        self.home.mkdir()
        self.data = self.root / "data"
        self.target = self.home / ".local/bin/ttc"
        self.manifest = self.data / "ttc/install.toml"
        self.bashrc = self.home / ".bashrc"
        self.bashrc.write_text("export USER_CONFIG=1\n")
        self.server = Release()
        self.worker = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.worker.start()
        self.env = os.environ.copy()
        self.env.update({
            "HOME": str(self.home), "XDG_DATA_HOME": str(self.data),
            "XDG_CONFIG_HOME": str(self.root / "config"), "XDG_STATE_HOME": str(self.root / "state"),
            "TMPDIR": str(self.root), "TTC_INTERNAL_TEST_TMP_ROOT": str(self.root),
            "TTC_INTERNAL_TEST_INSTALL": "1",
            "TTC_INTERNAL_TEST_RELEASE_BASE_URL": f"http://127.0.0.1:{self.server.server_port}",
        })

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.worker.join()
        self.scratch.cleanup()

    def install(self, success=True, args=()):
        result = subprocess.run(["sh", str(ROOT / "install.sh"), *args], env=self.env, cwd=self.root, capture_output=True, timeout=30)
        if success:
            self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        else:
            self.assertNotEqual(result.returncode, 0)
        return result

    def run_ttc(self, *args, installed=True):
        return subprocess.run([str(self.target if installed else BINARY), *args], env=self.env, cwd=self.root, capture_output=True, timeout=30)

    def metadata(self):
        return tomllib.loads(self.manifest.read_text())

    def put_metadata(self, metadata):
        self.manifest.write_text(encode(metadata))

    def test_fresh_reinstall_backup_path_and_global_uninstall(self):
        before = self.bashrc.read_bytes()
        result = self.install()
        self.assertEqual(self.run_ttc("--version").stdout, f"ttc {VERSION}\n".encode())
        self.assertEqual(self.target.stat().st_mode & 0o777, 0o755)
        self.assertEqual(self.metadata()["sha256"], DIGEST)
        self.assertEqual(self.metadata()["active_harnesses"], [])
        self.assertNotIn(b"install codex", result.stdout)
        self.assertIn(b"source", result.stdout)
        backups = list(self.home.glob(".ttc-*-bashrc-backup"))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_bytes(), before)
        managed = self.bashrc.read_bytes()
        self.install()
        self.assertEqual(self.bashrc.read_bytes(), managed)
        self.assertEqual(len(list(self.home.glob(".ttc-*-bashrc-backup"))), 1)
        self.bashrc.write_bytes(managed + b"export USER_AFTER=2\n")
        raw = self.root / "state/ttc/capture"
        raw.parent.mkdir(parents=True)
        raw.write_bytes(b"raw retained")
        self.assertEqual(self.run_ttc("uninstall").returncode, 0)
        self.assertFalse(self.target.exists())
        self.assertFalse(self.manifest.exists())
        self.assertEqual(self.bashrc.read_bytes(), before + b"export USER_AFTER=2\n")
        self.assertEqual(raw.read_bytes(), b"raw retained")
        self.assertEqual(self.run_ttc("uninstall", installed=False).returncode, 0)

    def test_latest_is_resolved_once_for_both_downloads(self):
        self.server.switch = True
        self.install()
        downloads = [path for path in self.server.requests if "/download/" in path]
        self.assertEqual(downloads, [f"/releases/download/v{VERSION}/{ASSET}", f"/releases/download/v{VERSION}/SHA256SUMS"])

    def test_owned_upgrade_is_atomic_for_running_version_observer(self):
        self.target.parent.mkdir(parents=True)
        # Current ELF with a downgraded version string, without historical code.
        # Shebang interpreters reopen the pathname and are not ELF baselines.
        self.assertTrue(IMAGE.startswith(b"\x7fELF"))
        old = IMAGE.replace(VERSION.encode(), b"0.0.9")
        self.target.write_bytes(old)
        self.target.chmod(0o755)
        self.manifest.parent.mkdir(parents=True)
        self.put_metadata({"schema_version": 1, "owner": "ttc", "version": "0.0.9", "sha256": hashlib.sha256(old).hexdigest(),
                           "installed_path": str(self.target), "active_harnesses": [], "path_owned": False, "path_file": "", "path_block": ""})
        self.assertEqual(self.run_ttc("--version").stdout, b"ttc 0.0.9\n")
        stop = threading.Event()
        failures = []
        observations = []

        def observe():
            while not stop.is_set():
                try:
                    result = self.run_ttc("--version")
                    observations.append(result.stdout)
                    if result.returncode or result.stdout not in (b"ttc 0.0.9\n", f"ttc {VERSION}\n".encode()):
                        failures.append((result.returncode, result.stdout, result.stderr))
                except OSError as error:
                    failures.append(str(error))

        thread = threading.Thread(target=observe)
        thread.start()
        try:
            self.install()
        finally:
            stop.set()
            thread.join()
        self.assertTrue(observations)
        self.assertFalse(failures, failures)
        self.assertEqual(self.metadata()["version"], VERSION)
        self.assertEqual(self.target.read_bytes(), IMAGE)

    def test_mismatch_duplicate_and_interrupted_download_preserve_install(self):
        self.install()
        before = self.target.read_bytes(), self.manifest.read_bytes(), self.bashrc.read_bytes()
        for flag in ["bad_checksum", "duplicate", "broken"]:
            with self.subTest(flag=flag):
                setattr(self.server, flag, True)
                self.install(success=False)
                setattr(self.server, flag, False)
                self.assertEqual((self.target.read_bytes(), self.manifest.read_bytes(), self.bashrc.read_bytes()), before)

    def test_tag_mismatch_and_prerelease_fail_before_install(self):
        for tag in ["v99.0.0", "v0.1.0-rc.1", "v00.1.0"]:
            with self.subTest(tag=tag):
                self.server.latest = tag
                self.install(success=False)
                self.assertFalse(self.target.exists())

    def test_platform_and_version_selector_rejected_before_download(self):
        self.install(success=False, args=("v0.1.0",))
        self.assertFalse(self.server.requests)
        shim = self.root / "shim"
        shim.mkdir()
        uname = shim / "uname"
        uname.write_text("#!/bin/sh\nprintf 'Darwin\\n'\n")
        uname.chmod(0o755)
        self.env["PATH"] = str(shim) + ":" + self.env["PATH"]
        self.install(success=False)
        self.assertFalse(self.server.requests)
        self.assertFalse(self.target.exists())

    def test_endpoint_override_requires_explicit_opt_in_and_loopback(self):
        self.env.pop("TTC_INTERNAL_TEST_INSTALL")
        self.install(success=False)
        self.assertFalse(self.server.requests)
        self.env["TTC_INTERNAL_TEST_INSTALL"] = "1"
        for url in ["http://example.invalid", "http://127.0.0.1:80@example.invalid", "http://127.0.0.1:80/path"]:
            self.env["TTC_INTERNAL_TEST_RELEASE_BASE_URL"] = url
            self.install(success=False)
        self.assertFalse(self.server.requests)

    def test_manual_unowned_binary_is_not_adopted(self):
        self.target.parent.mkdir(parents=True)
        self.target.write_bytes(IMAGE)
        self.target.chmod(0o755)
        before = self.bashrc.read_bytes()
        self.install(success=False)
        self.assertEqual(self.target.read_bytes(), IMAGE)
        self.assertEqual(self.bashrc.read_bytes(), before)
        self.assertFalse(self.manifest.exists())
        self.assertNotEqual(self.run_ttc("uninstall", installed=False).returncode, 0)

    def test_symlink_target_and_metadata_preserve_unrelated_file(self):
        self.target.parent.mkdir(parents=True)
        foreign = self.root / "foreign"
        foreign.write_bytes(b"foreign")
        self.target.symlink_to(foreign)
        self.install(success=False)
        self.assertEqual(foreign.read_bytes(), b"foreign")
        self.target.unlink()
        self.manifest.parent.mkdir(parents=True, exist_ok=True)
        self.manifest.symlink_to(foreign)
        self.install(success=False)
        self.assertEqual(foreign.read_bytes(), b"foreign")

    def test_corrupt_metadata_and_modified_binary_are_preserved(self):
        self.install()
        valid = self.manifest.read_bytes()
        for invalid in [b"not toml", valid.replace(b"schema_version = 1", b"schema_version = 2"), valid + b"unknown = 1\n"]:
            self.manifest.write_bytes(invalid)
            self.install(success=False)
            self.assertEqual(self.manifest.read_bytes(), invalid)
            self.assertEqual(self.target.read_bytes(), IMAGE)
        self.manifest.write_bytes(valid)
        self.target.write_bytes(b"user modified binary")
        self.install(success=False)
        self.assertNotEqual(self.run_ttc("uninstall", installed=False).returncode, 0)
        self.assertEqual(self.target.read_bytes(), b"user modified binary")

    def test_active_harness_refuses_uninstall_and_survives_reinstall(self):
        self.install()
        meta = self.metadata()
        meta["active_harnesses"] = ["codex"]
        self.put_metadata(meta)
        self.install()
        self.assertEqual(self.metadata()["active_harnesses"], ["codex"])
        self.assertNotEqual(self.run_ttc("uninstall").returncode, 0)
        self.assertEqual(self.target.read_bytes(), IMAGE)

    def test_pending_transaction_blocks_mutations(self):
        self.install()
        pending = self.manifest.parent / "install.pending"
        pending.write_text('operation = "install"\n')
        before = self.target.read_bytes(), self.manifest.read_bytes()
        self.install(success=False)
        self.assertNotEqual(self.run_ttc("uninstall").returncode, 0)
        self.assertEqual((self.target.read_bytes(), self.manifest.read_bytes()), before)
        self.assertTrue(pending.exists())

    def test_path_existing_readonly_symlink_and_invalid_utf8_fallbacks(self):
        self.env["PATH"] = str(self.target.parent) + ":" + self.env["PATH"]
        before = self.bashrc.read_bytes()
        self.install()
        self.assertFalse(self.metadata()["path_owned"])
        self.assertEqual(self.bashrc.read_bytes(), before)
        self.env["PATH"] = os.environ["PATH"]
        self.bashrc.chmod(0o444)
        self.assertIn(b"PATH manual", self.install().stdout)
        self.assertEqual(self.target.read_bytes(), IMAGE)
        self.bashrc.chmod(0o644)
        self.bashrc.write_bytes(b"invalid\xff\n")
        self.assertIn(b"PATH manual", self.install().stdout)
        self.assertEqual(self.bashrc.read_bytes(), b"invalid\xff\n")
        foreign = self.root / "foreign-config"
        foreign.write_bytes(b"config unrelated")
        self.bashrc.unlink()
        self.bashrc.symlink_to(foreign)
        self.assertIn(b"PATH manual", self.install().stdout)
        self.assertEqual(foreign.read_bytes(), b"config unrelated")

    def test_other_program_and_user_changed_block_keep_path(self):
        self.install()
        other = self.target.parent / "other-tool"
        other.write_bytes(b"other program")
        before = self.bashrc.read_bytes()
        self.assertEqual(self.run_ttc("uninstall").returncode, 0)
        self.assertEqual(self.bashrc.read_bytes(), before)
        self.assertEqual(other.read_bytes(), b"other program")

    def test_default_data_home_and_missing_bashrc(self):
        self.env.pop("XDG_DATA_HOME")
        self.bashrc.unlink()
        self.install()
        manifest = self.home / ".local/share/ttc/install.toml"
        self.assertEqual(tomllib.loads(manifest.read_text())["sha256"], DIGEST)
        self.assertIn("# >>> TTC managed PATH >>>", self.bashrc.read_text())
        self.assertEqual(self.run_ttc("uninstall").returncode, 0)
        self.assertFalse(manifest.exists())
        self.assertEqual(self.bashrc.read_bytes(), b"")

    def test_install_and_uninstall_share_the_same_lock_inode(self):
        self.install()
        lock = self.manifest.parent / "install.lock"
        identity = lock.stat().st_ino
        commands = [[str(BINARY), "uninstall"], ["sh", str(ROOT / "install.sh")]] * 3
        processes = [subprocess.Popen(command, env=self.env, cwd=self.root, stdout=subprocess.PIPE, stderr=subprocess.PIPE) for command in commands]
        for process in processes:
            output = process.communicate(timeout=30)
            self.assertEqual(process.returncode, 0, output)
        self.assertEqual(lock.stat().st_ino, identity)
        self.assertEqual(self.target.exists(), self.manifest.exists())
        if self.target.exists():
            self.assertEqual(self.metadata()["sha256"], DIGEST)
            self.assertEqual(self.target.read_bytes(), IMAGE)
        self.assertLessEqual(self.bashrc.read_text().count("# >>> TTC managed PATH >>>"), 1)
        self.assertFalse((self.manifest.parent / "install.pending").exists())

    def test_concurrent_installers_serialize_commits(self):
        results = []

        def install():
            results.append(subprocess.run(["sh", str(ROOT / "install.sh")], env=self.env, cwd=self.root, capture_output=True, timeout=30))

        threads = [threading.Thread(target=install) for _ in range(4)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()
        self.assertEqual(len(results), 4)
        self.assertTrue(all(result.returncode == 0 for result in results), [result.stderr for result in results])
        self.assertEqual(self.target.read_bytes(), IMAGE)
        self.assertEqual(self.metadata()["sha256"], DIGEST)
        self.assertEqual(self.bashrc.read_text().count("# >>> TTC managed PATH >>>"), 1)
        self.assertFalse((self.manifest.parent / "install.pending").exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)
