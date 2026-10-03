#!/usr/bin/env python3
"""Exercise the docs-only gate allowlist and conservative diff handling."""
from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from ci_change_policy import (  # noqa: E402
    DiffError,
    changed_paths,
    full_gate_for_event,
    parse_name_status,
    requires_full_gate,
)


class ChangePolicyTests(unittest.TestCase):
    def test_allowlisted_documentation_uses_light_gate(self):
        for path in (
            "README.md",
            "LICENSE",
            "ai_docs/CURRENT_STATE.md",
            "ai_docs/TODO.md",
            "ai_docs/steps_done/01-session.md",
        ):
            with self.subTest(path=path):
                self.assertFalse(requires_full_gate([path]))

    def test_code_contract_or_unknown_path_uses_full_gate(self):
        for path in (
            "Cargo.toml",
            "src/main.rs",
            ".github/workflows/ci.yml",
            "AGENTS.md",
            "CLAUDE.md",
            "ai_docs/SPEC.md",
            "ai_docs/m9-coverage.json",
            "docs/guide.md",
        ):
            with self.subTest(path=path):
                self.assertTrue(requires_full_gate([path]))

    def test_mixed_change_uses_full_gate(self):
        self.assertTrue(
            requires_full_gate(["README.md", "src/main.rs", "ai_docs/TODO.md"])
        )

    def test_invalid_diff_records_fail_closed(self):
        for data in (b"M\0", b"R100\0old.md\0", b"Q\0file\0"):
            with self.subTest(data=data), self.assertRaises(DiffError):
                parse_name_status(data)
        self.assertTrue(requires_full_gate([]))

    def test_rename_and_delete_consider_every_path(self):
        rename = parse_name_status(b"R100\0README.md\0AGENTS.md\0")
        self.assertEqual(rename, ["README.md", "AGENTS.md"])
        self.assertTrue(requires_full_gate(rename))
        deleted = parse_name_status(b"D\0ai_docs/TODO.md\0")
        self.assertFalse(requires_full_gate(deleted))

    def test_tag_and_unknown_events_always_use_full_gate(self):
        self.assertTrue(full_gate_for_event("push", {"REF_TYPE": "tag"}))
        self.assertTrue(full_gate_for_event("workflow_dispatch", {}))

    def test_git_failure_and_missing_push_base_use_full_gate(self):
        self.assertTrue(full_gate_for_event("push", {"REF_TYPE": "branch"}))
        self.assertTrue(
            full_gate_for_event(
                "push",
                {
                    "REF_TYPE": "branch",
                    "BEFORE_SHA": "0" * 40,
                    "HEAD_SHA": "1" * 40,
                },
            )
        )
        with tempfile.TemporaryDirectory(prefix="ttc-ci-missing-range-") as directory:
            subprocess.run(["git", "init", directory], check=True, capture_output=True)
            self.assertTrue(
                full_gate_for_event(
                    "push",
                    {
                        "REF_TYPE": "branch",
                        "BEFORE_SHA": "1" * 40,
                        "HEAD_SHA": "2" * 40,
                    },
                    directory,
                )
            )

    def test_pull_request_diff_uses_base_to_head_range(self):
        with tempfile.TemporaryDirectory(prefix="ttc-ci-policy-") as directory:
            root = Path(directory)

            def git(*args: str) -> str:
                return subprocess.run(
                    ["git", *args], cwd=root, check=True, text=True, capture_output=True
                ).stdout.strip()

            git("init", "-b", "master")
            git("config", "user.name", "TTC CI policy test")
            git("config", "user.email", "test@example.invalid")
            (root / "README.md").write_text("base\n", encoding="utf-8")
            git("add", "README.md")
            git("commit", "-m", "base")
            base = git("rev-parse", "HEAD")
            (root / "README.md").write_text("docs\n", encoding="utf-8")
            git("commit", "-am", "docs")
            head = git("rev-parse", "HEAD")
            env = {"BASE_SHA": base, "PR_HEAD_SHA": head}
            self.assertEqual(changed_paths("pull_request", env, str(root)), ["README.md"])
            self.assertFalse(full_gate_for_event("pull_request", env, str(root)))
            self.assertFalse(
                full_gate_for_event(
                    "push",
                    {"REF_TYPE": "branch", "BEFORE_SHA": base, "HEAD_SHA": head},
                    str(root),
                )
            )

            (root / "src").mkdir()
            (root / "src/main.rs").write_text("fn main() {}\n", encoding="utf-8")
            git("add", "src/main.rs")
            git("commit", "-m", "code")
            code_head = git("rev-parse", "HEAD")
            env["PR_HEAD_SHA"] = code_head
            self.assertTrue(full_gate_for_event("pull_request", env, str(root)))
            self.assertTrue(
                full_gate_for_event(
                    "push",
                    {"REF_TYPE": "branch", "BEFORE_SHA": head, "HEAD_SHA": code_head},
                    str(root),
                )
            )


if __name__ == "__main__":
    unittest.main()
