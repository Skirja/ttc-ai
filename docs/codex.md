# Codex installation and compatibility

The installer writes a native `PreToolUse` handler matching `^Bash$`. It does not
modify `AGENTS.md`, model/context/memory settings, approval rules, or trust records.
If `hooks.json` exists, it merges there; otherwise it edits inline TOML while
preserving comments. Both representations are read by Codex, so avoid manually
duplicating TTC across them.

```sh
cargo build --release --locked
target/release/ttc install codex --dry-run
target/release/ttc install codex
~/.codex/bin/ttc doctor
```

`CODEX_HOME` overrides `~/.codex`. The binary is copied, not moved. The first config
change creates a timestamped backup. Reinstallation is idempotent. A receipt
records owned paths and the binary checksum. Modified unowned binaries are not
overwritten or deleted. `uninstall codex` removes only the recorded command and
owned binary, preserving other handlers and user settings.

Open **`/hooks`** in Codex to inspect and trust the exact installed hook. TTC does
not automate trust. Hook protocol smoke test:

```sh
printf '%s' '{"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"/tmp","permission_mode":"dontAsk","tool_input":{"command":"cargo test"}}' | ~/.codex/bin/ttc hook codex
```

Expected output in this version: `{}` and exit 0. This verifies fail-open protocol
handling, **not automatic optimization**. Use `ttc run -- cargo test` explicitly
for optimization today.

## Completion boundary

The Codex integration is **partial, not complete**. Registration, trust, a passing
hook smoke test, and explicit `ttc run` usage are infrastructure milestones only.
The project considers this integration complete only after eligible Bash commands
are rewritten through TTC automatically, the original command executes exactly
once with equivalent execution semantics, and the model-facing output is proven
to be filtered with a measured token reduction. None of those automatic-output
claims are true for the installed hook today.

## Why automatic rewriting remains disabled

The [official hooks documentation](https://learn.chatgpt.com/docs/hooks) supports
`permissionDecision: "allow"` with `updatedInput.command`. This is a rewrite
protocol, not a documented guarantee that original command policy checks survive.

The pinned public Codex implementation:

- [Tool registry](https://github.com/openai/codex/blob/b04a2c264516ec2e6b3c91dd73ad18a21fd5a88f/codex-rs/core/src/tools/registry.rs)
  replaces the invocation before dispatch.
- [exec_command hook mapping](https://github.com/openai/codex/blob/b04a2c264516ec2e6b3c91dd73ad18a21fd5a88f/codex-rs/core/src/tools/handlers/unified_exec/exec_command.rs)
  exposes only command text to PreToolUse, omitting its explicit shell/workdir
  overrides. The hook's cwd comes from the turn context.
- [Hook runtime](https://github.com/openai/codex/blob/b04a2c264516ec2e6b3c91dd73ad18a21fd5a88f/codex-rs/core/src/hook_runtime.rs)
  returns updated input to the registry; it does not provide TTC with an original
  command approval attestation.

These findings establish missing guarantees, not proof that every rewrite is
unsafe. They also do not prove equivalence with the installed
`0.154.0-alpha.6.2` binary. Consequently its compatibility report is
`incompatible`, `compatibility_verified()` is false, and there is no user
configuration switch that bypasses it. The dormant `/bin/sh` rewrite serializer
has been removed.

Enabling transparent integration requires a verified path that preserves the
actual shell, cwd, and original rule/approval evaluation, with installed-version
integration tests for denied prefixes, project scripts, explicit workdir,
escalation, unknown commands and competing hooks. A trusted diagnostic script is
still arbitrary executable code. Do not substitute an allowlist for these tests.

Windows hook command quoting and native ACL/console behavior also need validation.
The first version keeps all Windows auto-rewrites disabled. Managed hook policies
are not changed; if they prohibit this user hook, Codex skips it normally.

## Installed-version compatibility matrix

Run the matrix after a release build:

```sh
cargo build --release --locked
python3 scripts/codex_probe.py
```

The matrix uses a temporary `CODEX_HOME`, a local mock Responses server, and inert
commands that only print sentinel strings. It does not read or change the real
Codex configuration or trust store. It records that a temporary hook-trust bypass
was requested; the installed build reports `bypassPermissions` for those events
even when `-a never` or `-a on-request` was requested.

The [recorded matrix](research/codex-compatibility.json) for
`0.154.0-alpha.6.2` reports `incompatible`. Safe rewrite executed once, explicit
workdir was preserved, and competing-deny hooks won in both configured orders. The
no-rewrite baseline blocked a synthetic prefix-denied command, but the same command
executed after the candidate rewrite. The explicit shell was not preserved;
permission-mode labeling and structured PostToolUse exit status also failed their
invariants. The production TTC hook never emits that candidate rewrite.

## PostToolUse evidence

The earlier [recorded probe](research/codex-post-probe.json), now covered by the
matrix as well, confirms:

- A real command exited **7** and ran in an explicitly selected subdirectory.
- The hook received only raw text as `tool_response`, with **no exit status**.
- Hook `cwd` was the turn directory, not the actual execution subdirectory.
- `continue: false` successfully replaced the model-facing result with feedback.

Therefore PostToolUse is not enabled as a workaround: replacing the response
would lose the model-visible exit metadata unless a stronger contract is
available. Code-mode results retain their original value in the inspected source,
so replacing model-facing feedback also does not automatically compress code-mode
return values. The probe is compatibility evidence, not a completed transparent
optimization acceptance test.
