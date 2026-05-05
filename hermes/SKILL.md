---
name: codemetrics
description: AI-native code quality, security, and compliance audit toolkit - 21 automated checks for CI/CD pipelines and AI agents
version: 0.1.0
author: KidIkaros
license: Apache-2.0 OR OPL-1.1
platforms: [macos, linux]
metadata:
  hermes:
    tags: [Code-Quality, Rust, Metrics, Security, Testing, CI-CD]
    related_skills: [claude-code, opencode, codex]
    requires_toolsets: [terminal]
    fallback_for_tools: []
---

# codemetrics

21-check audit toolkit covering quality, security, and compliance. Designed for CI/CD pipelines and AI agents.

## When to Use

- User asks to audit code quality, security, or license compliance
- Before merging significant changes
- User asks about test coverage, risk, or hardcoded secrets
- Setting up CI/CD quality gates
- Generating an SBOM or checking OSS license exposure
- Evaluating code maintainability

## Quick Reference

| Command | Purpose |
|---------|----------|
| `codemetrics init` | Detect ecosystem, write `.quality.toml` |
| `codemetrics init --ci` | Full CI wiring (GHA + hook + baseline) |
| `codemetrics check .` | All 21 checks, auto-loads `.quality.toml` |
| `codemetrics check . --format json` | Machine-readable results for agents |
| `codemetrics check . --only sast,secrets,crypto` | Run specific checks only |
| `codemetrics check . --verbose` | Print file:line offenders for all checks |
| `codemetrics check . --ci` | CI mode: JSON out, no colors, exits 1 on fail |
| `codemetrics report .` | Generate HTML audit report |
| `codemetrics report . --open` | Generate + open in browser |
| `codemetrics diff old.json new.json` | Compare two check snapshots |
| `codemetrics watch . --no-tests` | Fast metrics-only watch loop |
| `codemetrics watch . --full` | Watch with all 21 checks |
| `codemetrics install-hooks --fast` | Lightweight pre-commit hook |
| `codemetrics run . --format sarif` | Full batch audit (SARIF output) |
| `codemetrics crap ./src --recursive` | CRAP scores only |
| `codemetrics mutate . -p {crate} --max-mutants 5` | Test quality |
| `codemetrics riskmap . --format json` | High-risk files |
| `codemetrics debt ./src --recursive` | TODOs/FIXMEs |
| `codemetrics doccov ./src --recursive` | Doc coverage |
| `codemetrics taint ./src --recursive` | Security taint |
| `codemetrics sbom .` | Generate SBOM (CycloneDX / SPDX) |

## Prerequisites

Build and install the binary:

```bash
cargo build --release
# Or install to PATH:
cargo install --path crates/codemetrics-cli
```

## Procedure

### 0. Zero-Config Setup (do once per repo)

```bash
codemetrics init        # detect ecosystem, write .quality.toml
codemetrics init --ci  # also wire GitHub Actions + pre-commit hook + baseline
```

### 1. Full Audit (Recommended for CI/CD)

```bash
# Run all 10 tools, output SARIF for GitHub Security tab
codemetrics run . --format sarif > results.sarif

# Or quick gate with .quality.toml thresholds
codemetrics check . --format json
```

### 2. Quick Risk Check

```bash
# Find high-risk functions (CRAP > 15)
codemetrics crap ./src --recursive --format json

# Find complex/churned files
codemetrics riskmap . --format json
```

### 3. Test Quality Check

```bash
# Requires: cargo test must pass first
codemetrics mutate . -p ast-parse-ts --max-mutants 5 --format json
```

### 4. Technical Debt

```bash
codemetrics debt ./src --recursive --format json
```

### 5. Security Spot-Check

```bash
# Run only security checks
codemetrics check . --only sast,secrets,crypto,taint,errhandle,vulnscan
```

### 6. License / Compliance Audit

```bash
codemetrics check . --only licenses,sbom
codemetrics sbom .   # standalone SBOM output
```

### 7. Watch Mode (live dev loop)

```bash
codemetrics watch .            # runs tests + coverage + checks on every change
codemetrics watch . --full     # all 21 checks every cycle
codemetrics watch . --no-tests # metrics-only, faster
```

### 8. Snapshot comparison

```bash
codemetrics check . --format json > before.json
# ... make changes ...
codemetrics check . --format json > after.json
codemetrics diff before.json after.json
```

## Tool Details

### crap (CRAP Score Calculator)
- **Purpose**: Find functions with high maintenance risk
- **Formula**: CRAP = comp² × (1 - coverage/100)³ + comp
- **Threshold**: > 15 is risky, > 30 is critical
- **Requires**: Test coverage data (optional)

### mutate (Mutation Testing)
- **Purpose**: Evaluate test suite quality
- **Precondition**: `cargo test` must pass
- **Output**: Mutation score (0-100%)
- **Notes**: Won't work on crates with failing tests

### riskmap (Risk Map)
- **Purpose**: Identify files that change often AND are complex
- **Data**: Cross-references git churn with code complexity
- **Use case**: Prioritize code reviews

### taint (Taint Analysis)
- **Purpose**: Detect sensitive data flow
- **Checks**: passwords, keys, PII to sinks
- **Use case**: Security audits

### sast (Static Application Security Testing)
- **Purpose**: Detect injection flaws and dangerous patterns
- **Checks**: SQL injection, XSS, path traversal, command injection, eval, SSRF (25 rules)
- **Severity**: Critical / High / Medium per finding

### secrets
- **Purpose**: Find hardcoded credentials and API keys
- **Output**: File:line location of each finding

### crypto
- **Purpose**: Detect weak cryptography
- **Checks**: MD5/SHA1, insecure random, ECB mode, hardcoded IVs, deprecated TLS

### licenses
- **Purpose**: OSS license compliance
- **Sources**: Cargo.lock, package.json, requirements.txt
- **Output**: GPL/AGPL/LGPL/MIT/Apache classification; deny-list violations

### sbom
- **Purpose**: Generate Software Bill of Materials
- **Formats**: CycloneDX 1.4 XML, SPDX 2.3 text
- **Sources**: Same lock files as `licenses`

## CI/CD Integration

### GitHub Actions (auto-generated by `codemetrics init --ci`)

```yaml
- name: Quality Audit
  run: |
    codemetrics run . --format sarif > results.sarif
- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: results.sarif
```

### Pre-commit Hook

```bash
# Install full hook (runs tests + coverage + check)
codemetrics install-hooks

# Install fast hook (metrics only, no test run)
codemetrics install-hooks --fast
```

## MCP / Hermes Server Integration

The `codemetrics-server` crate exposes all tools as an MCP stdio server:

```bash
# Start MCP server (stdio transport)
codemetrics-server --mode stdio

# Start with TCP transport
codemetrics-server --mode tcp --port 9876
```

Hermes MCP tool provider config:
```json
{ "mcpServers": { "codemetrics": { "command": "codemetrics-server", "args": ["--mode", "stdio"] } } }
```

The server supports both legacy `tools/run` (JSON-RPC) and standard MCP `tools/call` — existing Hermes skills using `tools/run` continue to work.

## Output Formats

| Format | Use Case | Command |
|--------|----------|---------|
| `json` | Programmatic | `--format json` |
| `sarif` | GitHub Security | `--format sarif` |
| `ndjson` | Streaming | `--format ndjson` |
| `text` | Human readable | `--format text` |

## Pitfalls

1. **mutate fails**: Ensure `cargo test` passes first; use `-p crate-name` in workspaces
2. **Coverage required for accurate CRAP**: Run `codemetrics init` to auto-detect coverage command
3. **No `.quality.toml`**: Run `codemetrics init` — `check` will use generic defaults without it
4. **Binary not on PATH**: Run `cargo install --path crates/codemetrics-cli` and `cargo install --path crates/codemetrics-server`

## Verification

```bash
# Check all tools work
codemetrics run . --format json | jq '.summary'

# Check specific tool
codemetrics crap ./src --recursive --format json | head

# Verify MCP server responds
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}' | codemetrics-server --mode stdio
```

## Rules

1. Run `codemetrics check .` before every PR (exit 0 = good to merge)
2. Fix CRAP > 30 immediately before proceeding
3. Use `mutate` to verify test suites catch bugs
4. Zero tolerance for TODO/FIXME in production code
5. Address riskmap findings in code reviews

## See Also

- Repository: https://github.com/KidIkaros/codemetrics
- UTCP Manual: `docs/utcp/codemetrics.json`
- Claude Code: `CLAUDE.md` (repo root)
- OpenCode: `AGENTS.md` (repo root)
- MCP / UTCP integration: `docs/utcp-integration.md`