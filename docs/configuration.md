# Configuration

Precedence: CLI execution options > environment restrictions > nearest project
`.ttc.toml` > global configuration > defaults. Discovery stops at a Git boundary.
Project hooks cannot enable what global configuration disabled; global project
exclusions remain enforced.

Global configuration: the OS config directory under `ttc-ai/config.toml`.
Data: the OS local-data directory under `ttc-ai/`. Override with `TTC_CONFIG` and
`TTC_DATA_DIR`. Codex installation honors `CODEX_HOME` (default `~/.codex`).

```toml
enabled = true
metrics = true
exclusions = ["/path/to/sensitive/**"]

[recovery]
enabled = true
ttl_hours = 24
max_entries = 100
max_total_mb = 250
max_capture_mb = 64

[hooks]
enabled = false
```

`TTC_BYPASS=1` disables filtering. `TTC_RECOVERY=0` disables storage and therefore
filtering, preserving recoverability. `TTC_METRICS=0` disables metrics. Invalid
configuration causes execution to pass through, with an error on stderr.

`hooks.enabled` is necessary but not sufficient for rewriting: no compatible
Codex execution profile is enabled in this version. Project configuration cannot
relax this guard. It is not an automatic-filtering switch: with the current build,
the trusted hook still returns `{}`, Bash bypasses TTC, and Codex receives unfiltered
command output.

The total quota concerns capture storage. SQLite metrics retain at most 100,000
records. Active captures reserve worst-case space and can cause earlier
passthrough than the completed compressed-byte count suggests. Set zero capture
capacity to force raw output. TTL/count cleanup runs around captured invocations;
manual `ttc cache clean` also applies retention. `--all` purges completed captures and discovery caches,
not running captures or metrics. `gain --today` uses UTC day boundaries.

Discovery graph caches occupy at most 32 entries of 256 KiB each, separately from
the capture quota. Their contents are private and may contain script/command text.
Explicit raw/bypass execution does not create discovery entries.
