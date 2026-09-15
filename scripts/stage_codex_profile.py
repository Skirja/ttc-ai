#!/usr/bin/env python3
"""Create a temporary build-time profile from a passing contract matrix."""

import argparse
import json
import pathlib


ROOT = pathlib.Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--report",
        type=pathlib.Path,
        default=ROOT / "docs/research/codex-compatibility.json",
    )
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    report = json.loads(args.report.read_text())
    if report.get("verdict") != "compatible" or report.get("blockers"):
        parser.error("contract compatibility report still has blockers")
    invariants = report["invariants"]
    profile = {
        "codex_version": report["codex_version"],
        "os": report["os"],
        "arch": report["arch"],
        "hook_protocol_version": report["hook_protocol_version"],
        "approval_equivalence": invariants["approval_equivalence"],
        "shell_fidelity": invariants["shell_fidelity"],
        "cwd_fidelity": invariants["cwd_fidelity"],
        "login_fidelity": invariants["login_fidelity"],
        "sandbox_fidelity": invariants["sandbox_fidelity"],
        "competing_hook_behavior": invariants["competing_hook_behavior"],
        "model_output_fidelity": invariants["model_output_fidelity"],
        "exit_status_fidelity": invariants["exit_status_fidelity"],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps({"schema_version": 1, "profiles": [profile]}, indent=2) + "\n")
    print(args.output)


if __name__ == "__main__":
    main()
