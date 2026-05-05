# CodeMetrics — AI Agent Quick Reference

This project uses **CodeMetrics** for automated code quality analysis.
Run `codemetrics init` once and it handles everything else automatically.

## Zero-Config Bootstrap

```bash
# 1. Detect project type and write .quality.toml with language-tuned thresholds
codemetrics init

# 2. Full CI bootstrap: config + GitHub Actions workflow + pre-commit hook + baseline SARIF
codemetrics init --ci
```

## Command Reference

| Command | What it does | When to use |
|---------|-------------|-------------|
| `codemetrics init` | Detect ecosystem, write `.quality.toml` | First time setup |
| `codemetrics init --ci` | Full CI wiring (GHA + hook + baseline) | Repo bootstrap |
| `codemetrics check .` | Run all checks, auto-loads `.quality.toml` | Before PR / after changes |
| `codemetrics check . --format json` | Machine-readable check results | Agent consumption |
| `codemetrics watch .` | Watch for changes, run tests + coverage + checks | Local dev loop |
| `codemetrics watch . --no-tests` | Watch without running tests | Fast metrics-only loop |
| `codemetrics install-hooks` | Install full pre-commit hook (tests + coverage + check) | Once per repo |
| `codemetrics install-hooks --fast` | Install lightweight hook (metrics only) | Fast commit workflow |
| `codemetrics run . --format sarif` | Full 10-tool batch audit | CI / deep audit |
| `codemetrics run . --format json` | Full audit, JSON output | Agent / pipeline |

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | All checks passed |
| `1` | One or more checks failed |
| `2` | Error (binary not found, parse failure, etc.) |

## Agent Consumption Pattern

```bash
# Quick gate — parse JSON, check exit code
result=$(codemetrics check . --format json)
if [ $? -eq 0 ]; then
  echo "Quality passed"
else
  echo "Failed checks:"
  echo "$result" | jq '.checks[] | select(.passed==false) | {name, message, score, threshold}'
fi
```

## Individual Tools (deep-dive)

```bash
codemetrics crap ./src --recursive --format json     # CRAP scores only
codemetrics debt ./src --recursive --format json     # Technical debt markers
codemetrics doccov ./src --recursive                 # Documentation coverage
codemetrics complexity ./src --recursive             # Cyclomatic complexity
```

## Key Thresholds (Rust defaults from `codemetrics init`)

| Metric | Threshold | Meaning |
|--------|-----------|---------|
| CRAP avg | ≤ 15 | > 15 means high maintenance risk |
| Debt markers | 0 | Zero tolerance for TODO/FIXME/HACK |
| Doc coverage | ≥ 95% | Public API must be documented |
| Complexity violations | 0 | No functions with CC ≥ 10 |

## Output Formats

| Flag | Use Case |
|------|----------|
| `--format json` | Programmatic / agent parsing |
| `--format sarif` | GitHub Security tab upload |
| `--format ndjson` | Streaming / incremental pipeline |
| `--format text` | Human-readable terminal output |

## MCP Server (for MCP-compatible agents)

```bash
# Start as MCP stdio server (Claude Desktop, Cursor, Windsurf)
codemetrics-server --mode stdio
```

Add to MCP config:
```json
{ "mcpServers": { "codemetrics": { "command": "codemetrics-server", "args": ["--mode", "stdio"] } } }
```

## Notes for Agents

- `codemetrics check .` **automatically loads** `.quality.toml` if present — no need to pass threshold flags
- CLI flags **override** `.quality.toml` values when both are present
- `codemetrics watch` **auto-detects** the test runner (Cargo, pytest, jest/vitest, go test)
- `--recursive` is implied for `check` and `run`; individual tools need it explicitly
- `mutate` requires `cargo test` to pass on the original code first
- Use `-p crate-name` with `mutate` in workspace crates

## See Also

- `docs/utcp/codemetrics.json` — UTCP tool definitions (machine-readable)
- `codemetrics discover --format json` — live tool catalog
- `docs/utcp-integration.md` — MCP / Hermes / UTCP integration guide
- `docs/quality-standards.md` — threshold rationale
