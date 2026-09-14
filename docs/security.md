# Security and recovery

Raw command output and command display strings may contain secrets. TTC keeps
them locally and never uploads them. Unix data directories are mode 0700 and
files 0600. The private-directory boundary is essential; TTC does not defend
against a process already running as the same user. Windows relies on inherited
user-directory ACLs, which still need native CI verification.

Recovery is a safety net, not a reason to erase failure diagnostics. Each stream
is captured as original bytes, including ANSI, Unicode, binary bytes and missing
newlines. Framed zstd records contain a channel tag, little-endian length, and
payload; readers reject invalid frame sizes and channels. Default recall escapes
controls; `--raw` is byte output and should be redirected for binary data.

TTL, count and quota limits apply locally. Active leases reserve storage and
protect running captures. Capture limits keep the prefix recoverable
and switch the remainder to raw forwarding. Partial records are labeled explicitly.
Storage failures never cause the original command to run again.

Unknown commands, pipelines, redirections, source reads, machine-output flags,
TTY-sensitive commands and unresolved scripts pass through. Dangerous operations
are never automatically wrapped. Explicit `ttc run` retains the user's command
semantics; it is not an authorization system or a sandbox.

Codex hooks currently remain no-op. An automatic wrapper can change prefix-rule
matching even when it contains visible original command text. A tool-family
allowlist is insufficient to prove that arbitrary build/test scripts are safe.
No permissive execution rule or hook trust bypass is installed.

The isolated Codex matrix demonstrates the concrete risk on the installed build:
the execpolicy prefix rule blocked an inert command in the no-rewrite baseline,
but the same command ran after a candidate `PreToolUse` rewrite returned `allow`.
The executable only prints a sentinel. This is evidence against enabling the
profile, not an attempt to use TTC as an authorization layer.

For sensitive work: use exclusions, `TTC_RECOVERY=0`, or `ttc raw`. Purge completed
logs with `ttc cache clean --all`. Do not attach raw captures to public issues
without inspecting and redacting their content.
