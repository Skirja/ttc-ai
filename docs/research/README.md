# Research record

The immutable revisions and retrieval timestamp are in [upstream.json](upstream.json).
RTK release `v0.49.0` was published September 11, 2026; its develop branch was a
different revision. RTK's Apache-2.0 license was inspected. No RTK implementation
source is incorporated; source hashes and public interface names are evidence,
not vendored implementations.

[Parity ledger](parity.md) enumerates 157 release command/filter/integration files.
[Interface inventory](interfaces.json) records the pinned source hashes and
public entrypoints. [Issues](issues.json) records current issue state and links.
The [Codex compatibility matrix](codex-compatibility.json) records installed-version
approval, context, competing-hook, permission-mode, and PostToolUse evidence.

Observed problem classes:

- #2094: replacing package-manager lint scripts discards script bodies/flags.
- #1489, #2879: lint-name routing can choose the wrong linter and misdescribe exits.
- #2832: warnings must not become “no issues.”
- #2358, #3675: custom/bare-script coverage differs from explicit run forms.
- #259, #444: workspace options and nested monorepo execution need structural routing.
- #2878: stdout-only handling can miss real formatter failures on stderr.

TTC regressions exercise those behavioral classes independently. The current
implementation preserves original execution, includes stderr, distinguishes
numeric codes from signals, resolves project scripts, and retains unknown output.

RTK's recovery implementation now includes SQLite-backed recall and legacy tee
paths. TTC is not claiming to introduce recall: its design uses separate tagged
zstd streams, bounded active reservations, and local statistics. RTK's integration
inventory includes actual hooks/plugins and prompt-level adapters; TTC currently
implements only the Codex protocol adapter, with automatic rewriting gated off.

Naming: crates.io already contains `ttc` (unrelated task/time tracker, v0.0.5).
The `ttc-ai` crates.io API returned 404 at inspection time; no package was published
or reserved. These are availability observations, not a trademark clearance.

Priority remaining work: verified transparent Codex execution integration; full
semantic filter parity; workspace-selector/version semantics; native Windows compatibility. The current release does not claim the full
original specification is complete.
