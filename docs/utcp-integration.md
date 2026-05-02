# UTCP Integration Guide

This document explains how to integrate quality-tools with AI agents using the Universal Tool Calling Protocol (UTCP).

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

quality-tools provides a UTCP manual at `utcps/quality-tools.json` that defines all tools:
- Tool names and descriptions
- Call syntax for each tool
- Input/output schemas

```bash
# Fetch the manual
cat utcps/quality-tools.json
```

### Option 2: Discover Command

Use the built-in discover command to get tool info:

```bash
# JSON format (for programmatic parsing)
quality discover --format json

# Text format (for humans)
quality discover --format text
```

### Option 3: CLI + JSON

All tools support JSON output for programmatic consumption:

```bash
# Individual tools
cargo run -p crap-metric -- ./src --recursive --format json
cargo run -p debt-scan -- ./src --recursive --format json
cargo run -p risk-map -- . --format json

# Full audit
cargo run -p quality-cli -- run . --format sarif > results.sarif
```

## AI Agent Integration

### Claude Code

Add to `CLAUDE.md` in project root:

```markdown
# Code Quality Tools

This project uses quality-tools. Before significant changes, run:

- `cargo run -p quality-cli -- run . --format sarif` for full audit
- `cargo run -p mutation-test -- . -p <crate> --max 5` to verify tests
```

Claude Code automatically loads `CLAUDE.md` from project root.

### OpenCode

Add to `AGENTS.md` in project root:

```markdown
# Code Quality Tools

Use quality-tools before major changes:

| Tool | Command |
|------|---------|
| Full audit | quality run . --format sarif |
| CRAP scores | cargo run -p crap-metric -- ./src --recursive |
| Tests | cargo run -p mutation-test -- . -p <crate> --max 5 |
```

OpenCode automatically loads `AGENTS.md` from project root.

### Custom UTCP Client

Use the UTCP manual directly:

```python
import json

# Load the manual
with open("utcps/quality-tools.json") as f:
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
| `mutate` | Test quality | `cargo run -p mutation-test -- . -p pkg --max 5` | JSON |
| `debt` | TODOs/FIXMEs | `cargo run -p debt-scan -- ./src --recursive` | JSON |
| `riskmap` | Risk files | `cargo run -p risk-map -- .` | JSON |
| `doccov` | Doc coverage | `cargo run -p doc-coverage -- ./src --recursive` | JSON |
| `taint` | Security | `cargo run -p taint-scan -- ./src --recursive` | JSON |
| `coupling` | Dependencies | `cargo run -p coupling -- .` | JSON |
| `dupfind` | Duplication | `cargo run -p duplication -- ./src --recursive` | JSON |
| `fuzz` | Fuzzable | `cargo run -p fuzz-surface -- ./src --recursive` | JSON |
| `quality` | Full audit | `cargo run -p quality-cli -- run . --format sarif` | SARIF |

## CI/CD Integration

### GitHub Actions

```yaml
- name: Quality Audit
  run: |
    cargo build --release
    quality run . --format sarif > results.sarif
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
- [quality-tools Repository](https://github.com/KidIkaros/quality-tools)
- `utcps/quality-tools.json` - Tool definitions
- `CLAUDE.md` - Claude Code rules
- `AGENTS.md` - OpenCode rules