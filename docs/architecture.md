# Architecture

The `ttc_ai` library contains the harness-neutral core. `ttc` supplies CLI I/O.

```text
Invocation (original argv or shell text)
  -> grammar parser / static project discovery
  -> workload graph + confidence + risk + exact-output intent
  -> original execution (never the discovered script bodies)
  -> bounded dual-stream capture
  -> semantic complete-record recognizers
  -> retained diagnostics + measured compaction + local recall
```

`Invocation` holds the authoritative execution request. `CommandAst` uses Bash
Tree-sitter grammar nodes and byte spans; shell-words decodes individual simple
commands. Embedded control characters are rejected before entering the native
parser. Windows has a separate narrow literal grammar; unsupported syntax is
uncertain. Parsing never performs shell expansion or runs project code.

The resolver reads JSON/YAML manifests and static task recipes. Package scripts
and possible pre/post hooks are represented as child workloads. Recursion is
bounded to 32 levels and 4,096 nodes; cycles and unknown nodes propagate
uncertainty. Manifest parsing is process-local and content hashed. A persistent graph cache
revalidates manifest/config contents and workspace membership across invocations.
It is bounded to 32 entries of at most 256 KiB each; oversized discoveries skip
caching. Raw/bypass execution skips discovery entirely. The workload graph stores planned work, not execution evidence.

Execution inherits stdin/cwd/environment. Native argv is passed without a shell;
explicit shell text is handed unchanged to the selected shell. On Unix,
passthrough replaces TTC with the original executable. Captured children have a
separate process group for signal forwarding; TTC re-raises actual terminating
signals after reaping the child. Numeric exit codes are never interpreted as
signals. Windows signal/console semantics need platform validation.

Two reader threads send bounded 16-KiB events over a bounded channel. Events retain
stdout/stderr tags and are written to a zstd stream. Output is filtered incrementally after each raw frame is flushed; memory use
does not grow with total log size. Watch/interactive commands bypass capture.
Unrecognized diagnostics stream immediately; aggregate compaction counts print at
completion or when a capture limit switches the remaining output to raw.

Filtering is independent of execution and exit status. Rules recognize complete
passing records/progress lines only for resolved families. Diagnostic language
vetoes suppression. Novel lines are retained exactly; composite workloads feed
all known families to the same pipeline. No stage-success summary is fabricated.
The line buffer flushes long lines unchanged after 1 MiB.

Captured execution also records `ExecutionEvidence` for the one top-level child
process TTC actually spawned: start/finish state, exit code or signal, duration,
and per-stream event/byte counts. Nested workload attribution is explicitly
`unavailable`; lifecycle scripts, shell operators, and workload-graph children are
never promoted to executed stages without runtime evidence. Unix passthrough uses
`exec`, so it leaves no synthetic evidence record behind.

SQLite records capture metadata and local metrics. Files use random 128-bit IDs,
private permissions, atomic publication and quota reservations. A lock serializes
quota allocation/publication/cleanup. Active leases prevent deletion of running
captures. Compression flushes incrementally so storage failure preserves the readable prefix and switches subsequent output to
raw without rerunning the child.

The Codex adapter is deliberately isolated. Its installed hook validates the
typed event and returns `{}` because no version-specific compatibility profile is
verified. There is no dormant rewrite serializer; a future implementation must
add its execution-context preservation and compatibility evidence together.
Operationally, this is hook infrastructure only: it does not place TTC in the Bash
execution path, compact model-facing output, or reduce Codex tokens. The integration
remains partial until all three behaviors are demonstrated end-to-end.
