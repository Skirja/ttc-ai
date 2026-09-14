# Support matrix

This is the implemented behavior, not a claim of full RTK parity. All automatic
Codex rewriting is disabled pending compatibility verification. Explicit execution
preserves the original command. Novel lines remain raw in every family. The Codex
integration is partial: hook installation/trust is not counted as completion until
automatic Bash routing produces measurably smaller model-facing output.

| Capability | Implemented behavior | Validation |
|---|---|---|
| Cargo tests/build/check/clippy | Passing records and compile/check progress compacted; diagnostics retained | Golden + execution/exit/signal tests |
| Go tests | Verbose PASS records compacted; package summaries retained | Golden fixtures |
| Vitest/Jest/Playwright | Recognized checkmark success records compacted; unfamiliar reporters retained | Golden fixtures |
| Bun tests | `(pass)` timing records compacted | Golden fixtures |
| pytest | Verbose PASSED records compacted | Golden fixtures |
| Maven/Gradle | Maven download progress compacted; Gradle output currently retained | Golden fixtures for Maven |
| .NET tests | Passing-test timing records compacted | Golden fixtures |
| CMake/Ninja | Object-build progress compacted | Golden fixtures |
| Vite/build tools | Recognized transform/render/gzip progress compacted | Golden fixtures |
| Docker/Podman | Layer completion/cache progress compacted; build errors retained | Golden fixtures |
| Git | Status records and transfer progress compacted; diff/log/show/branch/tag/stash/worktree retained | Structured status and golden transfer fixtures |
| pnpm/pip package output | Recognized resolution/download progress compacted in explicit mode | Golden fixtures |
| PHPUnit/Pest | Dot progress with verified progress counts compacted | Golden fixtures |
| PHPStan/Psalm | Recognized progress bar records compacted; diagnostics retained | Golden fixtures |
| RSpec/RuboCop | Recognized dot/inspection progress compacted | Golden fixtures |
| tox/nox | Recognized environment-install progress compacted | Golden fixtures |
| Swift | Recognized compile/module progress compacted | Golden fixtures |
| ESLint/Biome/oxlint/Prettier/TypeScript | Location-bearing duplicate diagnostics compacted; other output retained | Resolution and diagnostic tests |
| ruff/mypy/pyright and Go linters | Location-bearing duplicate diagnostics compacted; other output retained | Diagnostic tests |
| Filesystem/search tools | Match grouping with explicit -n and -H; other output retained | Exact-output safety tests |
| Forge/cloud/Kubernetes/Terraform/network | Recognized but output retained; structured/pipeline/download streams bypass | Exact-output safety tests |
| npm/pnpm/Yarn/Bun scripts | Actual manifest body, nested scripts, possible lifecycle hooks; original runner remains authoritative | Resolution and execution tests |
| Workspace runners | Basic name/glob selectors, recursive package discovery, Yarn workspace, package-backed Nx/Turbo/Lerna targets | Synthetic monorepo tests |
| Composer scripts | String/array bodies and nested script aliases | Resolver implementation; broader fixtures still needed |
| env/cross-env/timeout/Python wrappers | Literal assignments and supported nested invocation forms | Resolution regressions |
| Make/Just/Task | Static recipes/commands only; dynamic definitions pass through | Static task fixtures |
| Containers with exec/run | Context explicitly unresolved; passthrough | Container regression tests |
| PowerShell/cmd | Narrow literal recognition; no automatic interception | Parser tests only; native validation pending |
| Codex hook/install/uninstall | Infrastructure only: typed fail-open protocol, explicit incompatible profile, and idempotent ownership-aware configuration; no automatic output filtering | Unit tests + isolated installed-version matrix; product acceptance blocked |

The [upstream ledger](research/parity.md) includes additional RTK capabilities that
are not implemented. In particular, RTK's structured diagnostic parsers, search
compaction, diff/log transformations, specialized forge/cloud adapters, custom
filter DSL, and additional harness integrations are not equivalent in this
release. Source/file content, explicit machine output, and pipelines intentionally
remain exact rather than reproducing aggressive transformations.

Current discovery limitations: manager-version-dependent lifecycle settings are
modeled conservatively; complex pnpm dependency/changed-file selectors, Nx
executor configuration, inherited task dependencies, shell functions and dynamic
configuration remain incomplete. A script graph describes possible workloads; it
does not prove every stage executed or succeeded.
