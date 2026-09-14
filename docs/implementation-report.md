# TTC-AI implementation report

Status: functional early implementation installed locally for explicit use.
**The Codex token-reduction objective is partial, not complete, and the complete
production/full-parity specification is not finished.** Transparent Codex
optimization is blocked by missing execution metadata and unverified preservation
of original approval rules; the installed hook deliberately passes through and
does not filter model-facing output.

Project acceptance is stricter than installation status: a registered/trusted hook,
successful `{}` smoke response, and explicit `ttc run` invocation do not count as a
working automatic integration. Completion requires automatic Bash rewrite plus a
versioned end-to-end artifact proving equivalent execution and reduced model-facing
tokens.

1. **Architecture:** independent Rust library/CLI with grammar parsing, recursive
   workload graphs, risk/exact-output classification, authoritative original
   execution, streaming filters, SQLite/zstd recovery, local metrics and a harness
   adapter boundary. See [architecture](architecture.md).
2. **Decisions:** MIT OR Apache-2.0; original argv/shell text never replaced by
   discovered script bodies; unknown diagnostics retained; flushed capture before
   suppression; real signals distinguished from numeric exit codes; private,
   bounded recovery and graph caches.
3. **RTK reference:** pinned release v0.49.0 and develop/source revisions recorded
   in [research](research/README.md). The 157-entry ledger separates partial
   coverage from missing parity. No upstream implementation source was copied.
4. **Routing improvements:** real manifest script bodies, recursive/cross-language
   scripts, workspace selection, possible lifecycle hooks, static task discovery,
   environment/wrapper recognition, cache invalidation and fail-open fallback.
5. **Filters:** 23 noise recognizers across test/build/package/container families;
   Git status rendering, explicit filename/line search grouping and complete
   location-bearing diagnostic deduplication. All unknown diagnostic text remains
   visible. [Support matrix](support.md) records recognition-only gaps.
6. **Runtime evidence:** recovery metadata now includes one observed top-level
   process stage with start/finish state, exit code or signal, duration, and
   stdout/stderr counters. Nested script stages remain explicitly unavailable.
7. **Validation:** 66 Rust tests pass; the golden test covers 115 fixtures at three
   chunk boundaries (345 comparisons). `cargo fmt --check`, strict Clippy and the
   locked release build pass on Linux. Four real npm/pnpm success/failure cases
   preserve lifecycle traces, argv forwarding and exits 0/7. macOS/Windows CI is
   configured but has not run here.
8. **Benchmarks:** synthetic success logs reduce output by 99.62–99.96%, including
   TTC summaries/recall hints. Median startup 1.252 ms; classification 1.451 ms;
   composite resolution 1.557 ms. Peak RSS stays about 9.0–9.2 MiB across 1, 16 and
   80 MiB logs. The 80 MiB case exercises capture-limit raw fallback. These are
   fixture results, not expected savings for arbitrary workloads or billing.
   [Measurements](benchmarks.json).
9. **Release binary:** `/home/skirja/Work/Personal/ttc-ai/target/release/ttc`,
   8,061,016 bytes; SQLite and zstd are bundled. Linux linkage only requires
   standard system runtime libraries.
10. **Installed binary:** `/home/skirja/.codex/bin/ttc`. SHA-256 equality with the
   release artifact was verified. The installed binary ran this project's test
   suite successfully; [raw recall was verified](installed-validation.json).
11. **Codex configuration:** added one `PreToolUse`/`^Bash$` command handler in
    `/home/skirja/.codex/config.toml`. Existing settings/comments were compared and
    preserved. Backup: `/home/skirja/.codex/config.ttc-backup-1789276855`.
    Ownership/checksum receipt: `/home/skirja/.codex/ttc-install.json`.
    Reinstallation leaves configuration unchanged.
12. **Codex compatibility:** the versioned report for `0.154.0-alpha.6.2` is
    `incompatible`. Its isolated matrix showed the no-rewrite prefix baseline being
    blocked and the same denial bypassed by a candidate rewrite, plus permission-mode
    mismatch under the temporary trust override and no structured PostToolUse exit
    status. Production rewrite serialization has been removed and the installed
    hook remains `{}`.
13. **Verify:** `/home/skirja/.codex/bin/ttc doctor`. For explicit optimization:
    `/home/skirja/.codex/bin/ttc run -- /home/skirja/.cargo/bin/cargo test --locked`.
    The [Codex guide](codex.md) contains the exact JSON hook smoke test; its current
    expected response is `{}`, not a rewrite.
14. **Trust:** review the installed hook with `/hooks` in Codex. No user hook trust
    or permissive approval rule was written. Trusting this version does not enable
    automatic optimization because compatibility remains gated off.
15. **Limitations:** no automatic Codex optimization; incomplete RTK semantic
    parity and manager/version-specific workspace semantics; no container
    filesystem introspection; narrow Windows shell handling and unverified native
    Windows ACL/console behavior. Recovery from hooks cannot recover text already
    truncated by Codex. Explicit execution captures before TTC compaction.
16. **Next priorities:** implement and validate the automatic Bash-to-TTC path,
    obtain a Codex contract preserving original approval checks plus actual
    shell/cwd/exit status, and record before/after model-facing byte/token evidence;
    then complete semantic parity with independently validated fixtures and
    strengthen native integrations. Do not mark the milestone complete or enable
    wrapping by simply changing the compatibility boolean.
