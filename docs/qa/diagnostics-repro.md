# Bug report: diagnostics bundle and repro template (T905)

## One-command export

From a shell in any directory (logs default to the platform log dir, e.g. `%APPDATA%\envr\logs` on Windows, unless `ENVR_LOG_DIR` is set):

```bash
envr diagnostics export
# or explicit path:
envr diagnostics export --output ./envr-bug.zip
```

The zip contains:

- **`doctor.json`** — same structure as `envr doctor --format json` `data` (runtime root, per-language counts, issues, recommendations).
- **`system.txt`** — CLI version, compile target (when available), OS/arch.
- **`environment.txt`** — allowlisted `ENVR_*` values plus redacted entries for other `ENVR_*` keys; resolved runtime root from doctor.
- **`logs/*.log`** — up to eight most recent `*.log` files from the log directory, each truncated (see implementation cap).

Attach the zip to the issue (remove sensitive paths first if needed).

## JSON output

`envr diagnostics export --format json` prints the standard success envelope with `data.path` pointing at the written zip.

## Repro template (paste into issues)

```markdown
### Environment
- OS / arch:
- envr version (`envr --version`):

### What happened

### Expected

### Steps
1.
2.

### Diagnostics
- [ ] Attached `envr diagnostics export` zip (or describe why not)

### Extra
- Relevant `ENVR_*` (no secrets):
```

## Related

- Unified CLI errors: `crates/envr-cli/src/output.rs` (JSON envelope).
- Logging directory: `envr_core::logging::resolve_log_dir` (`ENVR_LOG_DIR` or platform default log dir).
