"""Stable tag/version/master containment policy, isolated Git history."""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
sys.dont_write_bytecode = True
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("policy", Path(__file__).with_name("check-release-version.py"))
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


class ReleasePolicyTests(unittest.TestCase):
    def test_existing_tag_has_a_master_only_full_gate_recovery_path(self):
        workflow = (Path(__file__).parent.parent / ".github/workflows/ci.yml").read_text()
        self.assertIn("workflow_dispatch:\n    inputs:\n      release_tag:", workflow)
        self.assertIn("required: true\n        type: string", workflow)
        self.assertIn("ref: ${{ inputs.release_tag || github.sha }}", workflow)
        self.assertIn("(github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/master' && inputs.release_tag != '')", workflow)
        self.assertIn("!(github.event_name == 'workflow_dispatch' && inputs.release_tag != '')", workflow)
        self.assertTrue(policy.STABLE.fullmatch("0.1.0"))

    def test_artifact_checksum_precedes_restoring_executable_mode(self):
        workflow = (Path(__file__).parent.parent / ".github/workflows/ci.yml").read_text()
        verify_step = workflow.split("- name: Verify tag, source containment, manual evidence, and checksums", 1)[1]
        verify_step = verify_step.split("- name: Refuse to overwrite an existing GitHub Release", 1)[0]
        checksum = verify_step.index("sha256sum --check --status SHA256SUMS")
        chmod = verify_step.index("chmod 755 ./ttc-x86_64-unknown-linux-gnu")
        execute = verify_step.index("./ttc-x86_64-unknown-linux-gnu --version")
        self.assertLess(checksum, chmod)
        self.assertLess(chmod, execute)

    def test_stable_tag_matches_version_and_master_history(self):
        with tempfile.TemporaryDirectory(prefix="ttc-tag-policy-") as temporary:
            root = Path(temporary)
            isolation = patch.dict(os.environ, {
                "HOME": temporary, "XDG_CONFIG_HOME": str(root / "config"),
                "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
            })
            isolation.start()
            self.addCleanup(isolation.stop)
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
