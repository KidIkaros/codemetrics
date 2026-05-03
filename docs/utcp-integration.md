# UTCP Integration Guide

This document explains how to integrate CodeMetrics with AI agents using the Universal Tool Calling Protocol (UTCP).

## What is UTCP?

UTCP (Universal Tool Calling Protocol) is a specification that enables AI agents to discover and call tools directly using their native protocols - no wrapper servers required.

**Key Benefits:**
- 🚀 Zero latency overhead - direct tool calls
- 🔒 Native security - use existing auth
- 🌐 Protocol flexibility - HTTP, CLI, and more
- ⚡ Easy integration - minimal changes needed
- 📈 Scalable - leverage existing infrastructure

## Integration Options

### Option 1: UTCP Manual (JSON)

CodeMetrics provides a UTCP manual at `utcps/CodeMetrics.json` that defines all tools:
- Tool names and descriptions
- Call syntax for each tool
- Input/output schemas

```bash
# Fetch the manual
cat utcps/CodeMetrics.json
```

### Option 2: Discover Command

Use the built-in discover command to get tool info:

```bash
# JSON format (for programmatic parsing)
codemetrics discover --format json

# Text format (for humans)
codemetrics discover --format text
```

### Option 3: CLI + JSON

All tools support JSON output for programmatic consumption:

```bash
# Individual tools
cargo run -p crap-metric -- ./src --recursive --format json
cargo run -p debt-scan -- ./src --recursive --format json
cargo run -p risk-map -- . --format json

# Full audit
cargo run -p codemetrics-cli -- run . --format sarif > results.sarif
```

## AI Agent Integration

### Claude Code

Add to `CLAUDE.md` in project root:

```markdown
# Code Quality Tools

This project uses CodeMetrics. Before significant changes, run:

- `cargo run -p codemetrics-cli -- run . --format sarif` for full audit
- `cargo run -p mutation-test -- . -p <crate> --max 5` to verify tests
```

Claude Code automatically loads `CLAUDE.md` from project root.

### OpenCode

Add to `AGENTS.md` in project root:

```markdown
# Code Quality Tools

Use CodeMetrics before major changes:

| Tool | Command |
|------|---------|
| Full audit | codemetrics run . --format sarif |
| CRAP scores | cargo run -p crap-metric -- ./src --recursive |
| Tests | cargo run -p mutation-test -- . -p <crate> --max 5 |
```

OpenCode automatically loads `AGENTS.md` from project root.

### Custom UTCP Client

Use the UTCP manual directly:

```python
import json

# Load the manual
with open("utcps/CodeMetrics.json") as f:
    manual = json.load(f)

# Find a tool
for tool in manual["tools"]:
    if tool["name"] == "crap":
        print(f"Use: {tool['call']['syntax']}")
```

## Tool Reference

| Tool | Purpose | CRAP Command | Output |
|------|---------|-------------|-------|
| `crap` | Risk scores | `cargo run -p crap-metric -- ./src --recursive` | JSON |
| `mutate` | Test codemetrics | `cargo run -p mutation-test -- . -p pkg --max 5` | JSON |
| `debt` | TODOs/FIXMEs | `cargo run -p debt-scan -- ./src --recursive` | JSON |
| `riskmap` | Risk files | `cargo run -p risk-map -- .` | JSON |
| `doccov` | Doc coverage | `cargo run -p doc-coverage -- ./src --recursive` | JSON |
| `taint` | Security | `cargo run -p taint-scan -- ./src --recursive` | JSON |
| `coupling` | Dependencies | `cargo run -p coupling -- .` | JSON |
| `dupfind` | Duplication | `cargo run -p duplication -- ./src --recursive` | JSON |
| `fuzz` | Fuzzable | `cargo run -p fuzz-surface -- ./src --recursive` | JSON |
| `codemetrics` | Full audit | `cargo run -p codemetrics-cli -- run . --format sarif` | SARIF |

## CI/CD Integration

### GitHub Actions

```yaml
- name: Quality Audit
  run: |
    cargo build --release
    codemetrics run . --format sarif > results.sarif
- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: results.sarif
```

### Pre-commit Hook

```bash
# Run quick_checks before commit
cargo run -p debt-scan -- ./src --recursive
cargo run -p crap-metric -- ./src --recursive --min-score 20
```

## See Also

- [UTCP Specification](https://github.com/universal-tool-calling-protocol/utcp-specification)
- [CodeMetrics Repository](https://github.com/KidIkaros/codemetrics)
- `utcps/CodeMetrics.json` - Tool definitions
- `CLAUDE.md` - Claude Code rules
- `AGENTS.md` - OpenCode rules
- `hermes/SKILL.md` - Hermes Agent skill

### Hermes Agent

Install skill locally for use with Hermes:

```bash
# Copy skill to Hermes skills directory
cp -r hermes/ ~/.hermes/skills/CodeMetrics
```

Or reference directly from project when loading:
- Hermes will auto-discover CodeMetrics when in `~/.hermes/skills/`

Skill features:
- Automatic terminal tool passthrough
- Code execution support via `execute_code`
- Platform: macOS, Linux