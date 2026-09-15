#!/usr/bin/env python3
"""Promote a passing production-hook matrix into TTC's compiled profile set."""

import argparse
import json
import pathlib


ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_REPORT = ROOT / "docs/research/codex-compatibility.json"
DEFAULT_PROFILES = ROOT / "profiles/codex.json"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=pathlib.Path, default=DEFAULT_REPORT)
    parser.add_argument("--profiles", type=pathlib.Path, default=DEFAULT_PROFILES)
    args = parser.parse_args()

    report = json.loads(args.report.read_text())
    if report.get("verdict") != "compatible" or report.get("blockers"):
        parser.error("compatibility report still has blockers")
    if not report.get("production_hook"):
        parser.error("only a production-hook report can be promoted")
    if report.get("hook_protocol_version") != 2:
        parser.error("report does not attest hook protocol v2")

    invariants = report["invariants"]
    profile = {
        "codex_version": report["codex_version"],
        "os": report["os"],
        "arch": report["arch"],
        "hook_protocol_version": 2,
        "approval_equivalence": invariants["approval_equivalence"],
        "shell_fidelity": invariants["shell_fidelity"],
        "cwd_fidelity": invariants["cwd_fidelity"],
        "login_fidelity": invariants["login_fidelity"],
        "sandbox_fidelity": invariants["sandbox_fidelity"],
        "competing_hook_behavior": invariants["competing_hook_behavior"],
        "model_output_fidelity": invariants["model_output_fidelity"],
        "exit_status_fidelity": invariants["exit_status_fidelity"],
    }
    profiles = json.loads(args.profiles.read_text())
    retained = [
        existing
        for existing in profiles.get("profiles", [])
        if (existing["codex_version"], existing["os"], existing["arch"])
        != (profile["codex_version"], profile["os"], profile["arch"])
    ]
    retained.append(profile)
    profiles = {"schema_version": 1, "profiles": retained}
    args.profiles.write_text(json.dumps(profiles, indent=2) + "\n")
    print(json.dumps(profile, indent=2))


if __name__ == "__main__":
    main()
