#!/usr/bin/env python3
"""Offline Codex compatibility matrix using isolated homes and a fake model server.

The candidate rewrite hook exists only inside each temporary directory. Production
TTC remains fail-closed. Test commands print inert sentinel strings; no destructive
operation is used. The real user Codex configuration and hook trust store are never
read or changed.
"""

import argparse
import http.server
import json
import os
import pathlib
import shlex
import subprocess
import tempfile
import threading
import time


ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "docs/research/codex-compatibility.json"


class ProbeServer:
    def __init__(self, command_args):
        self.command_args = command_args
        self.requests = []
        owner = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_POST(self):
                length = int(self.headers["Content-Length"])
                owner.requests.append(json.loads(self.rfile.read(length)))
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.end_headers()

                def event(kind, **fields):
                    payload = json.dumps({"type": kind, **fields})
                    self.wfile.write(f"event: {kind}\ndata: {payload}\n\n".encode())
                    self.wfile.flush()

                request_number = len(owner.requests)
                event(
                    "response.created",
                    response={
                        "id": f"response-{request_number}",
                        "status": "in_progress",
                        "output": [],
                    },
                )
                if request_number == 1:
                    item = {
                        "id": "function-1",
                        "type": "function_call",
                        "name": "exec_command",
                        "call_id": "call-1",
                        "arguments": json.dumps(owner.command_args),
                    }
                else:
                    item = {
                        "id": f"message-{request_number}",
                        "type": "message",
                        "role": "assistant",
                        "status": "completed",
                        "content": [
                            {
                                "type": "output_text",
                                "text": "probe complete",
                                "annotations": [],
                            }
                        ],
                    }
                event("response.output_item.added", output_index=0, item=item)
                event("response.output_item.done", output_index=0, item=item)
                event(
                    "response.completed",
                    response={
                        "id": f"response-{request_number}",
                        "status": "completed",
                        "output": [item],
                        "usage": {
                            "input_tokens": 1,
                            "output_tokens": 1,
                            "total_tokens": 2,
                        },
                    },
                )

        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    @property
    def port(self):
        return self.server.server_port

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_args):
        self.server.shutdown()
        self.thread.join(timeout=5)


def hook_source(event_path, mode, ttc_binary):
    return f"""import json,pathlib,shlex,sys
event=json.load(sys.stdin)
path=pathlib.Path({str(event_path)!r})
with path.open('a',encoding='utf-8') as stream:
    stream.write(json.dumps(event,separators=(',',':'))+'\\n')
mode={mode!r}
if mode == 'rewrite':
    original=event.get('tool_input',{{}}).get('command','')
    wrapped=shlex.quote({str(ttc_binary)!r})+' run --shell /bin/sh --command '+shlex.quote(original)
    print(json.dumps({{'hookSpecificOutput':{{'hookEventName':'PreToolUse','permissionDecision':'allow','updatedInput':{{'command':wrapped}}}}}}))
elif mode == 'deny':
    print(json.dumps({{'hookSpecificOutput':{{'hookEventName':'PreToolUse','permissionDecision':'deny','permissionDecisionReason':'TTC probe competing denial'}}}}))
else:
    print('{{}}')
"""


def parse_json_lines(raw):
    events = []
    for line in raw.decode(errors="replace").splitlines():
        if not line.startswith("{"):
            continue
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            pass
    return events


def command_items(events):
    return [
        event.get("item", {})
        for event in events
        if event.get("type") == "item.completed"
        and event.get("item", {}).get("type") == "command_execution"
    ]


def read_hook_events(path):
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text().splitlines() if line]


def case_definitions(actual, explicit_shell):
    return [
        {
            "name": "safe_diagnostic",
            "mode": "rewrite",
            "permission": "never",
            "command": "printf 'TTC_SAFE_DIAGNOSTIC\\n'",
            "sentinel": "TTC_SAFE_DIAGNOSTIC",
        },
        {
            "name": "prefix_denial_baseline",
            "mode": "noop",
            "permission": "never",
            "command": "ttc-blocked-command",
            "sentinel": "TTC_PREFIX_DENIAL_EXECUTED",
            "rule": 'prefix_rule(pattern=["ttc-blocked-command"], decision="forbidden")\n',
        },
        {
            "name": "prefix_denial",
            "mode": "rewrite",
            "permission": "never",
            "command": "ttc-blocked-command",
            "sentinel": "TTC_PREFIX_DENIAL_EXECUTED",
            "rule": 'prefix_rule(pattern=["ttc-blocked-command"], decision="forbidden")\n',
        },
        {
            "name": "escalation",
            "mode": "rewrite",
            "permission": "on-request",
            "command": "printf 'TTC_ESCALATION_EXECUTED\\n'",
            "sentinel": "TTC_ESCALATION_EXECUTED",
            "tool_overrides": {
                "sandbox_permissions": "require_escalated",
                "justification": "TTC inert compatibility probe",
            },
        },
        {
            "name": "explicit_shell",
            "mode": "rewrite",
            "permission": "never",
            "command": "printf 'TTC_SHELL=%s\\n' \"${TTC_EXPLICIT_SHELL:-missing}\"",
            "sentinel": "TTC_SHELL=",
            "tool_overrides": {"shell": str(explicit_shell)},
        },
        {
            "name": "explicit_workdir",
            "mode": "rewrite",
            "permission": "never",
            "command": "pwd",
            "sentinel": str(actual),
            "tool_overrides": {"workdir": str(actual)},
        },
        {
            "name": "unknown_command",
            "mode": "noop",
            "permission": "never",
            "command": "ttc-command-that-does-not-exist",
            "sentinel": "TTC_UNKNOWN_EXECUTED",
        },
        {
            "name": "competing_deny_first",
            "mode": "rewrite",
            "permission": "never",
            "command": "printf 'TTC_COMPETING_EXECUTED\\n'",
            "sentinel": "TTC_COMPETING_EXECUTED",
            "competing": "deny_first",
        },
        {
            "name": "competing_deny_last",
            "mode": "rewrite",
            "permission": "never",
            "command": "printf 'TTC_COMPETING_EXECUTED\\n'",
            "sentinel": "TTC_COMPETING_EXECUTED",
            "competing": "deny_last",
        },
        {
            "name": "permission_never",
            "mode": "noop",
            "permission": "never",
            "command": "printf 'TTC_PERMISSION_NEVER\\n'",
            "sentinel": "TTC_PERMISSION_NEVER",
            "expected_permission_mode": "dontAsk",
        },
        {
            "name": "permission_on_request",
            "mode": "noop",
            "permission": "on-request",
            "command": "printf 'TTC_PERMISSION_ON_REQUEST\\n'",
            "sentinel": "TTC_PERMISSION_ON_REQUEST",
            "expected_permission_mode": "default",
        },
        {
            "name": "post_exit_status",
            "mode": "post",
            "permission": "never",
            "command": "printf 'TTC_POST_RAW\\n'; exit 7",
            "sentinel": "TTC_POST_RAW",
        },
    ]


def run_case(codex, ttc_binary, root, case):
    home = root / case["name"] / "home"
    work = root / "work"
    home.mkdir(parents=True)
    event_path = root / case["name"] / "hook-events.jsonl"
    candidate = root / case["name"] / "candidate.py"
    hook_mode = "noop" if case["mode"] == "post" else case["mode"]
    candidate.write_text(hook_source(event_path, hook_mode, ttc_binary))
    competing = case.get("competing")
    if competing:
        denial = root / case["name"] / "denial.py"
        denial.write_text(hook_source(event_path, "deny", ttc_binary))
        ordered = [denial, candidate] if competing == "deny_first" else [candidate, denial]
        handlers = [
            {"type": "command", "command": f"python3 {shlex.quote(str(path))}", "timeout": 2}
            for path in ordered
        ]
    else:
        handlers = [
            {
                "type": "command",
                "command": f"python3 {shlex.quote(str(candidate))}",
                "timeout": 2,
            }
        ]
    event_name = "PostToolUse" if case["mode"] == "post" else "PreToolUse"
    hooks = {"hooks": {event_name: [{"matcher": "^Bash$", "hooks": handlers}]}}
    (home / "hooks.json").write_text(json.dumps(hooks))
    if case.get("rule"):
        rules = home / "rules"
        rules.mkdir()
        (rules / "ttc-probe.rules").write_text(case["rule"])

    command_args = {
        "cmd": case["command"],
        "login": False,
        "max_output_tokens": 1000,
        **case.get("tool_overrides", {}),
    }
    with ProbeServer(command_args) as server:
        (home / "config.toml").write_text(
            f'''model = "ttc-probe"
model_provider = "ttc_probe"
[model_providers.ttc_probe]
name = "Offline TTC probe"
base_url = "http://127.0.0.1:{server.port}/v1"
wire_api = "responses"
requires_openai_auth = false
supports_websockets = false
'''
        )
        environment = os.environ.copy()
        environment["CODEX_HOME"] = str(home)
        environment["PATH"] = str(root / "bin") + os.pathsep + environment.get("PATH", "")
        args = [
            codex,
            "--dangerously-bypass-hook-trust",
            "-a",
            case["permission"],
            "exec",
            "--skip-git-repo-check",
            "--ephemeral",
            "--json",
            "-s",
            "read-only",
            "-C",
            str(work),
            "Run the supplied local compatibility command exactly once.",
        ]
        try:
            process = subprocess.run(args, env=environment, capture_output=True, timeout=45)
            timed_out = False
        except subprocess.TimeoutExpired as error:
            process = error
            timed_out = True
        events = parse_json_lines(process.stdout or b"")
        items = command_items(events)
        output = "\n".join(
            str(item.get("aggregated_output", item.get("output", ""))) for item in items
        )
        sanitized_output = output.replace(str(root), "<temporary root>")
        hook_events = read_hook_events(event_path)
        result = {
            "name": case["name"],
            "timed_out": timed_out,
            "codex_exit": None if timed_out else process.returncode,
            "hook_calls": len(hook_events),
            "permission_mode": hook_events[0].get("permission_mode") if hook_events else None,
            "tool_input": hook_events[0].get("tool_input") if hook_events else None,
            "command_exit_codes": [item.get("exit_code") for item in items],
            "sentinel_count": output.count(case["sentinel"]),
            "output_excerpt": sanitized_output[-1000:],
            "requests": len(server.requests),
        }
        if case["name"] == "safe_diagnostic":
            result["passed"] = result["sentinel_count"] == 1 and result["command_exit_codes"] == [0]
        elif case["name"] in {
            "prefix_denial_baseline",
            "prefix_denial",
            "escalation",
            "competing_deny_first",
            "competing_deny_last",
        }:
            result["passed"] = result["sentinel_count"] == 0
        elif case["name"] == "explicit_shell":
            result["passed"] = "TTC_SHELL=preserved" in output and result["sentinel_count"] == 1
        elif case["name"] == "explicit_workdir":
            result["passed"] = str(root / "work" / "actual") in output
        elif case["name"] == "unknown_command":
            result["passed"] = result["sentinel_count"] == 0 and any(
                code not in (None, 0) for code in result["command_exit_codes"]
            )
        elif case["name"].startswith("permission_"):
            result["passed"] = result["permission_mode"] == case["expected_permission_mode"]
        elif case["name"] == "post_exit_status":
            response = hook_events[0].get("tool_response") if hook_events else None
            result["post_tool_response_type"] = type(response).__name__
            result["post_hook_has_structured_exit_status"] = isinstance(response, dict) and any(
                key in response for key in ("exit_code", "status", "outcome")
            )
            result["passed"] = result["post_hook_has_structured_exit_status"]
        else:
            result["passed"] = False
        return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--codex", default="codex")
    parser.add_argument("--ttc", type=pathlib.Path, default=ROOT / "target/release/ttc")
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    if not args.ttc.is_file():
        parser.error(f"TTC binary not found: {args.ttc}; run cargo build --release --locked")
    version = subprocess.run(
        [args.codex, "--version"], capture_output=True, text=True, timeout=10, check=True
    ).stdout.strip()
    with tempfile.TemporaryDirectory(prefix="ttc-codex-matrix-") as temporary:
        root = pathlib.Path(temporary)
        work = root / "work"
        actual = work / "actual"
        actual.mkdir(parents=True)
        binaries = root / "bin"
        binaries.mkdir()
        blocked = binaries / "ttc-blocked-command"
        blocked.write_text("#!/bin/sh\nprintf 'TTC_PREFIX_DENIAL_EXECUTED\\n'\n")
        blocked.chmod(0o700)
        explicit_shell = binaries / "ttc-probe-shell"
        explicit_shell.write_text(
            "#!/bin/sh\nexport TTC_EXPLICIT_SHELL=preserved\nexec /bin/sh \"$@\"\n"
        )
        explicit_shell.chmod(0o700)
        cases = [
            run_case(args.codex, args.ttc.resolve(), root, case)
            for case in case_definitions(actual, explicit_shell)
        ]

    by_name = {case["name"]: case for case in cases}
    invariants = {
        "safe_rewrite_executes_once": by_name["safe_diagnostic"]["passed"],
        "approval_equivalence": by_name["prefix_denial_baseline"]["passed"]
        and by_name["prefix_denial"]["passed"]
        and by_name["escalation"]["passed"],
        "shell_fidelity": by_name["explicit_shell"]["passed"],
        "cwd_fidelity": by_name["explicit_workdir"]["passed"],
        "unknown_passthrough": by_name["unknown_command"]["passed"],
        "competing_hook_behavior": by_name["competing_deny_first"]["passed"]
        and by_name["competing_deny_last"]["passed"],
        "permission_mode_behavior": by_name["permission_never"]["passed"]
        and by_name["permission_on_request"]["passed"],
        "exit_status_available": by_name["post_exit_status"]["passed"],
        "documented_original_command_approval_equivalence": False,
    }
    blockers = [name for name, passed in invariants.items() if not passed]
    report = {
        "schema_version": 2,
        "generated_at_unix": int(time.time()),
        "codex_version": version,
        "ttc_binary": str(args.ttc.resolve()),
        "isolated_codex_home": True,
        "hook_trust_override_requested": True,
        "destructive_commands_used": False,
        "verdict": "compatible" if not blockers else "incompatible",
        "automatic_rewrite_enabled": False,
        "invariants": invariants,
        "blockers": blockers,
        "cases": cases,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    if any(case["timed_out"] or not case["hook_calls"] for case in cases):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
