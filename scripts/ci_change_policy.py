"""Classify workflow changes conservatively, allowing only ordinary docs a light gate."""
from __future__ import annotations

from collections.abc import Mapping, Sequence
import json
import re
import subprocess
from urllib.parse import urlencode
from urllib.request import Request, urlopen

_SHA = re.compile(r"\A[0-9a-fA-F]{40,64}\Z")
_LIGHT_DOC_ROOTS = frozenset({"README.md", "LICENSE", "ai_docs/CURRENT_STATE.md", "ai_docs/TODO.md"})
_LIGHT_DOC_ARCHIVE = "ai_docs/steps_done/"


class DiffError(ValueError):
    """The requested diff could not be interpreted safely."""


def is_light_documentation_path(path: str) -> bool:
    """Return true only for the explicit documentation-only allowlist."""
    return path in _LIGHT_DOC_ROOTS or (
        path.startswith(_LIGHT_DOC_ARCHIVE) and path.endswith(".md")
    )


def parse_name_status(data: bytes) -> list[str]:
    """Decode Git's NUL-delimited --name-status format, including both rename paths."""
    fields = data.split(b"\0")
    if fields and fields[-1] == b"":
        fields.pop()
    paths: list[str] = []
    index = 0
    while index < len(fields):
        try:
            status = fields[index].decode("ascii")
        except UnicodeDecodeError as error:
            raise DiffError("Git returned an invalid change status") from error
        index += 1
        if not status or status[0] not in "ACDMRTUXB":
            raise DiffError(f"Git returned an unknown change status: {status!r}")
        path_count = 2 if status[0] in "RC" else 1
        if index + path_count > len(fields):
            raise DiffError("Git returned an incomplete rename/copy record")
        for raw_path in fields[index : index + path_count]:
            try:
                path = raw_path.decode("utf-8")
            except UnicodeDecodeError:
                # A non-UTF-8 path is outside the allowlist by definition.
                path = ""
            paths.append(path)
        index += path_count
    return paths


def requires_full_gate(paths: Sequence[str]) -> bool:
    """Use the light gate only for a non-empty diff containing allowlisted docs."""
    return not paths or not all(is_light_documentation_path(path) for path in paths)


def changed_paths(event: str, env: Mapping[str, str], cwd: str = ".") -> list[str]:
    """Read the event's complete diff; fail closed on absent commits or Git errors."""
    if event == "pull_request":
        base = env.get("BASE_SHA", "")
        head = env.get("PR_HEAD_SHA", "")
        if not _SHA.fullmatch(base) or not _SHA.fullmatch(head):
            raise DiffError("Pull request base/head SHA is missing or invalid")
        revision = f"{base}...{head}"
    elif event == "push":
        before = env.get("BEFORE_SHA", "")
        head = env.get("HEAD_SHA", "")
        if not _SHA.fullmatch(before) or not _SHA.fullmatch(head) or set(before) == {"0"}:
            raise DiffError("Push range is missing or invalid")
        revision = f"{before}..{head}"
    else:
        raise DiffError(f"Event {event!r} always uses the full gate")

    result = subprocess.run(
        ["git", "diff", "--name-status", "-z", "--find-renames", revision, "--"],
        cwd=cwd,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        raise DiffError("Git could not read the complete change range")
    return parse_name_status(result.stdout)


def full_gate_for_event(event: str, env: Mapping[str, str], cwd: str = ".") -> bool:
    """Tags, unknown events, and unreadable diffs always take the full gate."""
    if event == "push" and env.get("REF_TYPE") == "tag":
        return True
    if event not in {"pull_request", "push"}:
        return True
    try:
        return requires_full_gate(changed_paths(event, env, cwd))
    except (DiffError, OSError, ValueError):
        return True


def previous_push_passed(env: Mapping[str, str]) -> bool:
    """Require a successful master run for the exact push base; uncertainty is false."""
    before = env.get("BEFORE_SHA", "")
    repository = env.get("GITHUB_REPOSITORY", "")
    token = env.get("CI_READ_TOKEN", "")
    if (
        not _SHA.fullmatch(before)
        or set(before) == {"0"}
        or not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository)
        or not token
    ):
        return False
    api = env.get("GITHUB_API_URL", "https://api.github.com").rstrip("/")
    query = urlencode({"branch": "master", "event": "push", "head_sha": before, "per_page": 100})
    request = Request(
        f"{api}/repos/{repository}/actions/workflows/ci.yml/runs?{query}",
        headers={"Accept": "application/vnd.github+json", "Authorization": f"Bearer {token}"},
    )
    try:
        with urlopen(request, timeout=15) as response:
            runs = json.load(response)["workflow_runs"]
        return any(
            str(run["id"]) != env.get("GITHUB_RUN_ID", "")
            and run["head_sha"] == before
            and run["head_branch"] == "master"
            and run["event"] == "push"
            and run["status"] == "completed"
            and run["conclusion"] == "success"
            for run in runs
        )
    except (OSError, ValueError, KeyError, TypeError):
        # Includes API denial, timeout, malformed results, and unavailable history.
        return False


def full_gate_for_run(event: str, env: Mapping[str, str], cwd: str = ".") -> bool:
    """A docs push can replace a code run only by completing its full verification."""
    if full_gate_for_event(event, env, cwd):
        return True
    return event == "push" and not previous_push_passed(env)
