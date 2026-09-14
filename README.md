# TTC-AI — Token Tight Command

`ttc` is a local Rust command-output optimizer. It resolves project scripts and
nested wrappers, executes the **original invocation once**, retains actionable
output, and stores recoverable raw logs. No runtime LLM, network service, or
telemetry is used.

**Status: early implementation, not full RTK parity or a production-complete
release.** Explicit execution and recall work. The Codex installer registers a
native hook, but automatic command rewriting is disabled: shell/workdir metadata
and approval equivalence have not been established for the installed Codex
version. Installing the hook does not currently reduce Codex output automatically.
See [Codex compatibility](docs/codex.md) and [support matrix](docs/support.md).
The installed-version matrix proves that a candidate rewrite can bypass a prefix
denial, so fail-closed behavior is an observed security requirement.

**Codex integration status: partial, not complete.** For this project's Definition
of Done, an installed/trusted hook and explicit `ttc run` usage do not count as a
working Codex integration. Completion requires eligible Bash commands to be routed
through TTC automatically and an end-to-end measurement showing that the
model-facing output is filtered and uses fewer tokens.

## Build and use

```sh
cargo build --release --locked
./target/release/ttc run -- cargo test
./target/release/ttc run -- pnpm check
./target/release/ttc run --shell /bin/bash --command 'pnpm lint && cargo test'
./target/release/ttc explain 'pnpm --filter api check'
./target/release/ttc gain --history
```

In a terminal, TTC preserves the child's TTY and passes through. Filtering is
intended for harness pipes. For a manual demonstration, redirect TTC's output to
a file; the original child invocation still runs once. Commands containing their
own pipes/redirections remain exact passthrough.

A script named `lint` is not assumed to mean ESLint. Given:

```json
{"scripts":{"check":"biome check . && tsc --noEmit && cargo clippy && go vet ./..."}}
```

`ttc explain 'pnpm check'` discovers all four workloads. `ttc run -- pnpm check`
still invokes pnpm, preserving its script body, lifecycle, environment, and flags.
Unknown or dynamically resolved workloads pass through rather than being guessed.

## Capture and recall

Recognized passing test records and progress lines are compacted. Unknown text,
compiler errors, stack traces, warnings, and test summaries remain visible. A
non-zero child status stays non-zero; a numeric 254 is not described as OOM.

```text
TTC: 10000 passing records, 0 progress lines compacted
raw: ttc recall <32-character-id>
```

```sh
ttc recall <id>
ttc recall <id> --stderr --tail 100
ttc recall <id> --stdout --grep panic
ttc recall <id> --lines 120:220
ttc recall <id> --raw > original-output.bin
ttc cache status
ttc cache clean --all
```

Default recall safely escapes control characters. `--raw` restores original bytes.
Separate streams are byte-exact; combined recall follows observed read order.
Selective recall uses one-based inclusive line ranges and literal grep matching.

Raw logs can contain secrets. Storage is local and private, with a 24-hour TTL,
100 completed captures, 250 MiB quota, and 64 MiB per-command raw cap. Active
captures reserve space; when storage is unavailable, execution passes through.
Read [security and recovery](docs/security.md).

## Bypass and configure

```sh
TTC_BYPASS=1 ttc run -- cargo test
ttc raw -- cargo test
TTC_RECOVERY=0 ttc run -- cargo test
ttc config show
```

Configuration is TOML. See [configuration](docs/configuration.md) for precedence,
project exclusions, and paths. Savings are estimates of output/context reduction,
not provider billing savings.

## Codex installation

```sh
./target/release/ttc install codex --dry-run
./target/release/ttc install codex
~/.codex/bin/ttc doctor
```

The installer honors `CODEX_HOME`, backs up existing configuration, preserves
unrelated hooks/settings, and writes an ownership receipt. Review the hook in
Codex with `/hooks`. TTC never writes trust records or permissive approval rules.
Automatic rewriting remains disabled pending the compatibility requirements above.
The trusted hook currently returns `{}` and provides no automatic output filtering
or token reduction inside Codex.

```sh
~/.codex/bin/ttc uninstall codex --dry-run
~/.codex/bin/ttc uninstall codex
```

## Architecture and development

[Status, gap, dan roadmap](docs/status-and-roadmap.md) · [Implementation report](docs/implementation-report.md) · [Architecture](docs/architecture.md) · [Contributing](CONTRIBUTING.md) ·
[Benchmarks](docs/benchmarks.json) · [Upstream research](docs/research/README.md)

RTK is a behavioral reference, not a code base fork. TTC separates command
understanding from execution and focuses on recursive scripts, conservative
filtering, and capture/recall. No RTK source is incorporated. This implementation
uses the dual `MIT OR Apache-2.0` license.

The crates.io name `ttc` is already used by an unrelated task/time-tracking tool.
Our package is `ttc-ai`; its executable is `ttc`. The installer refuses to overwrite
an unowned binary at its target location.
