# TTC Repository Instructions

This file defines repository-wide working rules for coding agents.
It governs development of TTC itself. It is not part of the TTC installation flow and must never be copied into a user's project by `install.sh` or a harness installer.

## Sources of truth

- Read `ai_docs/SPEC.md` before changing product behavior. It is the normative
  source for contracts, safety properties, supported commands, and release
  requirements.
- Read the active milestone in `ai_docs/TODO.md` before implementation. TODO is
  the source for ordering, dependencies, acceptance criteria, verification
  commands, and completion evidence.
- If SPEC and TODO disagree, SPEC wins. Fix SPEC first for an intentional
  contract change, then update TODO in the same change.
- Do not infer requirements from pre-greenfield commits or restore/reuse the old
  TTC implementation. Earlier history is not an implementation source unless
  the user explicitly asks for historical research.

## Implementation order

- Work on one milestone or independently testable slice at a time.
- Respect the dependency graph in TODO. Do not mark a milestone complete while
  any implementation, acceptance, verification, or evidence checkbox remains
  unmet.
- All standalone binary milestones M1–M9 must be complete, including the Linux
  CI artifact, before starting the Codex integration in M10.
- Do not implement future harnesses such as Claude during the Codex MVP. Keep
  only the minimum adapter boundary needed to add another harness later.
- Avoid placeholders that make a command appear supported before its behavior
  and tests are complete.

## Non-negotiable behavior

- Execute the original command exactly once. Never rerun it for classification,
  parsing recovery, storage recovery, or error handling.
- Default to retaining output. Compact only records proven safe by a specific
  recognizer; unknown, ambiguous, malformed, structured, or binary output stays
  raw unless SPEC defines a lossless parser.
- Preserve original exit status, signals, cwd, environment, stdin, per-stream
  byte content, and interactive/TTY behavior as defined by SPEC.
- Preserve every warning, error, failure, diagnostic, assertion diff, stack
  trace, security message, and final summary required by SPEC.
- Keep streaming memory bounded. Do not buffer an entire child output in memory.
- Filtering failures must fail open to raw output without rerunning the child.
- Keep the execution/filter core independent from Codex and other harnesses.
- Keep the MVP as one Rust binary crate. Do not add a daemon, service, database,
  telemetry, remote configuration, LLM filtering, or network access from the
  TTC binary.

## Code and dependency discipline

- Prefer small modules with one responsibility and explicit interfaces between
  CLI, execution, classification, filters, raw storage, and harness adapters.
- Keep dependencies directed inward: harness adapters may depend on core; core
  must not depend on a harness.
- Add production dependencies only when they remove concrete complexity or are
  required for correctness. Explain non-obvious dependency choices in the
  change summary.
- Use pinned toolchain/tool versions where TODO requires reproducible CI or
  ecosystem smoke tests.
- Use task-scoped temporary directories in tests. Do not write fixtures, raw
  captures, install metadata, shell config, or harness config into the user's
  real home directory during ordinary tests.
- Never print, copy, commit, or inspect authentication secrets. Codex E2E may use
  the existing authenticated CLI only through the isolated flow required by M10.

## Verification

- Run the narrowest relevant tests while iterating, then every verification
  command listed by the active milestone before marking it complete.
- Once the Rust crate exists, the baseline repository gate is:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
```

- GitHub Actions runs the complete gate for every code, contract, workflow,
  agent-rule, unknown-path, mixed, or unclassifiable change. The light gate is
  restricted to `README.md`, `LICENSE`, `ai_docs/CURRENT_STATE.md`,
  `ai_docs/TODO.md`, and Markdown files under `ai_docs/steps_done/`. Keep the
  workflow active for these changes and require its final aggregate job to
  reject failed, cancelled, or unexpectedly skipped prerequisites.

- A parser family is not complete with fixtures alone. Add success, failure,
  warning, unknown/edge, and large fixtures plus the pinned real-tool smoke test
  required by TODO.
- Compare passthrough behavior against a direct baseline. Verify stdout and
  stderr independently, invocation count, exit/signal behavior, and retained
  diagnostics.
- Do not claim a command, test, CI run, sandbox mode, byte reduction, or token
  reduction passed unless it was actually run and its evidence is available.
- Codex integration tests belong only to M10: use the installed Codex CLI and a
  production release-mode TTC binary, run with `--ephemeral`, isolate the
  workspace/config, and restore or remove every test artifact afterward.

## CI/CD Rules

### Continuous integration

- Use GitHub Actions. CI runs for pull requests targeting `master` and pushes to
  `master`; do not make feature-branch-only behavior part of a required gate.
- Build the workflow progressively as milestones land. Never add placeholder or
  skipped jobs that make unsupported functionality appear green.
- Required CI starts with format, Clippy with warnings denied, all-target/all-
  feature tests, and a release build. Add the active milestone's fixture,
  integration, real-tool smoke, reduction, and safety jobs before completing
  that milestone.
- Use an explicit Linux runner image, a pinned Rust toolchain, a committed
  `Cargo.lock`, and pinned ecosystem tool versions. Do not use floating `latest`
  versions inside required test matrices.
- Pin third-party GitHub Actions to immutable commit SHAs. Keep workflow-level
  permissions read-only and grant additional permission only to the release job
  that needs it.
- Required jobs must not use `continue-on-error`, ignore failures, or silently
  skip an unavailable tool. A missing prerequisite is a failed gate.
- PR jobs must run without repository or user secrets. Never expose Codex auth,
  user config, signing material, or release credentials to fork/PR code.
- Cache only reproducible build inputs, keyed by lockfile and toolchain. A clean
  build must pass without cache, and release artifacts must be built in the
  release job rather than restored from a development cache.
- Cancel superseded runs for the same pull request, but never cancel an active
  tag/release run.
- Upload test reports and release-mode artifacts only after their producing job
  passes. Evidence linked from TODO must identify the exact commit and CI run.
- Before M10 starts, M9 must have one clean successful `master` run that creates
  and smoke-tests the standalone `x86_64-unknown-linux-gnu` workflow artifact.

### Continuous delivery and release

- Ordinary pushes to `master` may produce CI artifacts but must never publish a
  GitHub Release.
- CD is triggered only by a user-authorized SemVer tag matching `vX.Y.Z` on a
  commit contained in `master`.
- The release job must first verify that the tag version exactly matches
  `Cargo.toml`, then rerun every required gate from a clean checkout.
- Build the public binary in release mode for `x86_64-unknown-linux-gnu`, smoke-
  test it outside the source tree, generate `SHA256SUMS`, and publish the binary,
  checksum, and `install.sh` from that same workflow run.
- Grant `contents: write` only to the publish job and only after all build/test
  dependencies pass. All earlier jobs remain read-only.
- M10 Codex CLI E2E is a local manual release gate because it uses an existing
  authenticated Codex installation. Record sanitized evidence in TODO; never
  move this authenticated test into GitHub-hosted CI.
- Do not create `v0.1.0` until M1–M10 are complete and the user explicitly asks
  for the tag/release action.
- Never move, overwrite, or delete an existing release tag automatically. Stop
  and report a failed release instead of concealing it with a rewritten tag.
- After publication, smoke-test the public `releases/latest` installer from a
  clean temporary environment and attach the result to M10 evidence.

## Documentation and milestone tracking

- Keep product documentation in Indonesian to match SPEC and TODO. Code symbols
  and technical identifiers may remain English.
- Update SPEC whenever public CLI behavior, filtering guarantees, storage,
  installer behavior, supported platform, or release policy changes.
- Update TODO checkboxes only after the corresponding evidence exists. Record
  the actual verification command and artifact; never pre-check future work.
- Keep AGENTS.md concise. Add a nested `AGENTS.md` or `AGENTS.override.md` only
  when a subtree genuinely needs different commands or constraints.

## Git workflow

- The primary branch is `master`.
- Preserve user changes and unrelated work. Never discard, restore, or rewrite
  them to make a task easier.
- For implementation work, create a new `feat/<topic>` branch from `master`.
  Do not commit implementation directly to `master`.
- Keep commits scoped to one milestone or coherent vertical slice, and commit
  only after its relevant verification passes.
- Push the feature branch and open a pull request targeting `master` so the PR
  CI workflow runs. Wait for the required CI jobs, fix failures, and record the
  exact commit and CI run as evidence. The user merges the PR on GitHub; agents
  must not merge it.
- Do not push directly to `master`, create tags, or publish releases unless the
  user explicitly requests that action.
- Treat `v*` tags as release actions: M9 and M10 gates must be complete before a
  `v0.1.0` tag can be created.

## Code Review Rules

Treat any of the following as release-blocking findings:

- a path that can execute the original command more than once;
- loss or rewriting of unknown output, diagnostics, exit status, or signals;
- unbounded buffering or deadlock risk between stdout and stderr;
- filtering machine-readable output without the lossless parser required by
  SPEC;
- weakening a Codex sandbox or silently adding writable permissions;
- overwriting an unowned binary or unrelated shell/harness configuration;
- installer/update behavior that skips checksum or atomic replacement;
- tests that touch real user config, auth, sessions, or state without guaranteed
  backup, isolation, and cleanup;
- Codex work starting before M9 evidence, or a release/tag before M10 evidence.

When reporting a review finding, name the violated SPEC/TODO requirement and
the smallest safe correction.
