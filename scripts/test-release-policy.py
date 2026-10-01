"""Stable tag/version/master containment policy, isolated Git history."""
import importlib.util
from pathlib import Path
import subprocess
import sys
sys.dont_write_bytecode = True
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("policy", Path(__file__).with_name("check-release-version.py"))
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


class ReleasePolicyTests(unittest.TestCase):
    def test_stable_tag_matches_version_and_master_history(self):
        with tempfile.TemporaryDirectory(prefix="ttc-tag-policy-") as temporary:
            root = Path(temporary)
            def git(*args):
                return subprocess.run(["git", *args], cwd=root, check=True, capture_output=True)
            git("init", "-b", "master")
            git("config", "user.name", "TTC isolated test")
            git("config", "user.email", "test@example.invalid")
            manifest = root / "Cargo.toml"
            manifest.write_text('[package]\nversion = "0.1.0"\n')
            git("add", "Cargo.toml")
            git("commit", "-m", "master baseline")
            self.assertEqual(policy.check(manifest, "v0.1.0", master="master"), "0.1.0")
            for tag in ("v0.2.0", "0.1.0", "v0.1.0-rc.1", "v00.1.0", "v0.1.0\n"):
                with self.assertRaises(ValueError):
                    policy.check(manifest, tag, master="master")
            git("switch", "-c", "feature")
            (root / "feature").write_text("not in master")
            git("add", "feature")
            git("commit", "-m", "feature only")
            with self.assertRaises(subprocess.CalledProcessError):
                policy.check(manifest, "v0.1.0", master="master")
            manifest.write_text('[package]\nversion = "0.1.0-rc.1"\n')
            with self.assertRaises(ValueError):
                policy.check(manifest)


if __name__ == "__main__":
    unittest.main()
