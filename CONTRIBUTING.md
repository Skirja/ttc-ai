# Contributing

Use the pinned stable Rust toolchain. TTC is licensed MIT OR Apache-2.0.

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
python3 scripts/benchmark.py
```

Filters must not execute commands, inject flags, change exit status, or infer a
script implementation from its name. Add a family-specific complete-record rule
only when it is demonstrably noise. Novel diagnostics must survive. Run golden
fixtures at different byte boundaries and retain expected/actual/source context.
Do not use raw output containing secrets as a public fixture.

`python3 scripts/fixtures.py` regenerates the independently authored fixtures.
Review expected-output changes; regeneration is not proof of correctness. The
property tests cover quoting, malformed shell/JSON and recovery IDs. Tests use
fake tools so all ecosystems are not required in CI. Unix execution/signal tests
are platform gated; native Windows validation remains necessary.

`python3 scripts/research.py` refreshes upstream revision and issue metadata;
`python3 scripts/audit_upstream.py` records hashes/interfaces without vendoring
upstream source. Update the parity ledger honestly: recognition-only is not
semantic compaction, and a compiled hook is not a verified transparent integration.

Public API changes should update the versioned JSON explanation contract and
architecture docs. Future harness adapters must preserve original security
boundaries and keep the core independent of harness-specific JSON.

Do not mark the Codex integration complete based on installation, trust, hook
invocation, or explicit `ttc run` behavior. Its Definition of Done requires an
end-to-end Codex test proving automatic Bash routing through TTC, exactly-once
equivalent execution, filtered model-facing output, and measured token reduction.
