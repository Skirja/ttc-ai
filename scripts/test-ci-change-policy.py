#!/usr/bin/env python3
"""Exercise the docs-only gate allowlist and conservative diff handling."""
from __future__ import annotations

from pathlib import Path
from io import StringIO
import json
import os
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from urllib.parse import parse_qs, urlparse

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from ci_change_policy import (  # noqa: E402
    DiffError,
    changed_paths,
    full_gate_for_event,
    full_gate_for_run,
    parse_name_status,
    requires_full_gate,
    previous_push_passed,
)


class ChangePolicyTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="ttc-ci-git-home-")
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        (root / "config").mkdir()
        (root / "git-template").mkdir()
        env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
        env.update({
            "HOME": temporary.name,
            "XDG_CONFIG_HOME": str(root / "config"),
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_TEMPLATE_DIR": str(root / "git-template"),
        })
        isolation = patch.dict(os.environ, env, clear=True)
        isolation.start()
        self.addCleanup(isolation.stop)

    def test_docs_push_replaces_unfinished_code_run_with_full_gate(self):
        env = {"REF_TYPE": "branch"}
        with patch("ci_change_policy.full_gate_for_event", return_value=False):
            for verified in (False, True):
                with self.subTest(predecessor_verified=verified), patch(
                    "ci_change_policy.previous_push_passed", return_value=verified
                ):
                    self.assertEqual(full_gate_for_run("push", env), not verified)
            with patch("ci_change_policy.previous_push_passed") as history:
                self.assertFalse(full_gate_for_run("pull_request", {}))
                history.assert_not_called()
        with patch("ci_change_policy.full_gate_for_event", return_value=True), patch(
            "ci_change_policy.previous_push_passed"
        ) as history:
            self.assertTrue(full_gate_for_run("push", env))
            history.assert_not_called()

    def test_exact_successful_master_push_is_required(self):
        env = {
            "BEFORE_SHA": "1" * 40,
            "GITHUB_REPOSITORY": "example/ttc",
            "GITHUB_RUN_ID": "200",
            "CI_READ_TOKEN": "isolated-test-token",
        }
        successful = {
            "id": 100, "head_sha": env["BEFORE_SHA"], "head_branch": "master",
            "event": "push", "status": "completed", "conclusion": "success",
        }
        cases = (
            ({}, True),
            ({"status": "in_progress", "conclusion": None}, False),
            ({"status": "queued", "conclusion": None}, False),
            ({"conclusion": "cancelled"}, False),
            ({"conclusion": "failure"}, False),
            ({"head_sha": "2" * 40}, False),
            ({"head_branch": "feature"}, False),
            ({"event": "pull_request"}, False),
            ({"id": 200}, False),
        )
        for changes, expected in cases:
            payload = {"workflow_runs": [{**successful, **changes}]}
            with self.subTest(changes=changes), patch(
                "ci_change_policy.urlopen", return_value=StringIO(json.dumps(payload))
            ) as opened:
                self.assertEqual(previous_push_passed(env), expected)
                request = opened.call_args.args[0]
                query = parse_qs(urlparse(request.full_url).query)
                self.assertEqual(query["head_sha"], [env["BEFORE_SHA"]])
                self.assertEqual(query["branch"], ["master"])
                self.assertEqual(query["event"], ["push"])
                self.assertIn("/actions/workflows/ci.yml/runs?", request.full_url)
        for payload in ({"workflow_runs": []}, {}, {"workflow_runs": [None]}):
            with self.subTest(payload=payload), patch(
                "ci_change_policy.urlopen", return_value=StringIO(json.dumps(payload))
            ):
                self.assertFalse(previous_push_passed(env))

    def test_unavailable_history_requires_full_replacement(self):
        env = {
            "BEFORE_SHA": "1" * 40,
            "GITHUB_REPOSITORY": "example/ttc",
            "CI_READ_TOKEN": "isolated-test-token",
        }
        with patch("ci_change_policy.urlopen", side_effect=OSError("API unavailable")):
            self.assertFalse(previous_push_passed(env))
        with patch("ci_change_policy.urlopen", return_value=StringIO("invalid JSON")):
            self.assertFalse(previous_push_passed(env))
        for changes in ({"BEFORE_SHA": ""}, {"BEFORE_SHA": "0" * 40}, {"CI_READ_TOKEN": ""}):
            with self.subTest(changes=changes), patch("ci_change_policy.urlopen") as opened:
                self.assertFalse(previous_push_passed({**env, **changes}))
                opened.assert_not_called()

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

            push_env = {
                "REF_TYPE": "branch", "BEFORE_SHA": base, "HEAD_SHA": head,
                "GITHUB_REPOSITORY": "example/ttc", "GITHUB_RUN_ID": "200",
                "CI_READ_TOKEN": "isolated-test-token",
            }
            predecessor = {
                "id": 100, "head_sha": base, "head_branch": "master", "event": "push",
            }
            for status, conclusion, full in (
                ("in_progress", None, True),
                ("completed", "cancelled", True),
                ("completed", "success", False),
            ):
                payload = {"workflow_runs": [{
                    **predecessor, "status": status, "conclusion": conclusion,
                }]}
                with self.subTest(predecessor_status=status, conclusion=conclusion), patch(
                    "ci_change_policy.urlopen", return_value=StringIO(json.dumps(payload))
                ):
                    self.assertEqual(full_gate_for_run("push", push_env, str(root)), full)

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


class GitIsolationRegressionTests(unittest.TestCase):
    def test_user_signing_hooks_and_git_environment_are_not_used(self):
        with tempfile.TemporaryDirectory(prefix="ttc-ci-hostile-git-") as directory:
            root = Path(directory)
            hooks = root / "hooks"
            hooks.mkdir()
            marker = root / "user-config-was-used"
            reject = hooks / "pre-commit"
            reject.write_text(f"#!/bin/sh\ntouch '{marker}'\nexit 1\n", encoding="utf-8")
            reject.chmod(0o755)
            config = root / ".gitconfig"
            config.write_text(
                f'[core]\n hooksPath = "{hooks}"\n[commit]\n gpgSign = true\n'
                f'[gpg]\n program = "{reject}"\n', encoding="utf-8",
            )
            env = dict(os.environ)
            env.update({
                "HOME": directory,
                "XDG_CONFIG_HOME": str(root / "config"),
                "GIT_CONFIG_GLOBAL": str(config),
                "GIT_CONFIG_COUNT": "1",
                "GIT_CONFIG_KEY_0": "core.hooksPath",
                "GIT_CONFIG_VALUE_0": str(hooks),
                "GIT_TEMPLATE_DIR": str(hooks),
            })
            result = subprocess.run(
                [sys.executable, str(Path(__file__).resolve()), "ChangePolicyTests"],
                env=env, check=False, text=True, capture_output=True,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertFalse(marker.exists(), "The test invoked a user hook or signing program")


if __name__ == "__main__":
    unittest.main()
