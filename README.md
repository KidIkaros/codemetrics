# CodeMetrics

<img src="logo.svg" alt="CodeMetrics Logo" width="400"/>

[![Quality Audit](https://github.com/KidIkaros/codemetrics/actions/workflows/codemetrics.yml/badge.svg)](https://github.com/KidIkaros/codemetrics/actions)
[![Docs](https://img.shields.io/badge/docs-available-brightgreen)](./docs/)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange)](https://rust-lang.org)
[![License](https://img.shields.io/badge/License-Apache--2.0%20%7C%20OPL--1.1-blue)](LICENSE)

**AI-native code quality audit toolkit** — 13 automated tools for multi-language analysis, CI/CD integration, and AI agent workflows.

- **Zero config** — auto-detects 15 languages from source files
- **CI-ready** — JSON/SARIF output with severity levels and fix suggestions
- **Production-proven** — dogfooded on the CodeMetrics codebase itself
- **No dependencies** — tree-sitter based, no compilation needed

## Quick Install

```bash
# From crates.io (once published)
cargo install codemetrics-cli

# Or build from source
git clone https://github.com/KidIkaros/codemetrics.git
cd codemetrics && cargo build --release
./target/release/codemetrics --help
```

## One-Minute Demo

```bash
# Full audit (GitHub Security tab compatible)
codemetrics run . --format sarif > results.sarif

# Single tool — CRAP risk scoring
codemetrics crap ./src --recursive

# Mutation testing (Rust only)
codemetrics mutate . -p my-crate --max-mutants 5
```

## Tools at a Glance

| Tool | Purpose | Primary Output |
|------|---------|----------------|
| `crap` | CRAP risk scores (CC × coverage) | risk score per function |
| `mutate` | Mutation testing (surviving = weak tests) | mutation score % |
| `debt` | TODO/FIXME/HACK/XXX markers | debt inventory by author |
| `riskmap` | Churn × complexity hot spots | risk-ranked file list |
| `doccov` | Public API doc coverage (%) | coverage % per module |
| `taint` | Sensitive data flow tracing | taint path reports |
| `fuzz` | Fuzzable function detection | fuzzability scores |
| `coupling` | Module dependency analysis | coupling matrix |
| `dupfind` | Code duplication detection | duplicate blocks |
| `propcov` | Property test coverage | property coverage % |
| `risk` *(batch)* | Combined risk scoring | aggregated severity |

All tools support `--format json` and `--format ndjson` for AI agent pipelines.

## Why CodeMetrics?

| Problem | CodeMetrics Solution |
|---------|---------------------|
| Scattered quality tools | 1 unified binary (`codemetrics`) running 13 tools |
| Language lock-in | 15 languages via tree-sitter (no compilation) |
| CI noise | SARIF output integrates with GitHub Security tab |
| AI agent blindness | Self-contained findings with `help`, `severity`, `rule_id` |

## Multi-Language Support

Analyzes **Rust, Python, JavaScript, TypeScript, Go, C, C++, C#, Java, PHP, Ruby, Swift, Kotlin, Solidity, Vyper, OCaml** directly from source files.

| Language | Status | Tools |
|----------|--------|-------|
| Rust | ✅ Full — including mutation testing | 13 tools |
| Python | ✅ Full | 13 tools |
| JavaScript/TypeScript | ✅ Full | 13 tools |
| Go | ✅ Full | 13 tools |
| C/C++ | ✅ Full | 13 tools |
| C# | ✅ Full | 13 tools |
| Java | ✅ Full | 13 tools |
| PHP | ✅ Full | 13 tools |
| Ruby | ✅ Full | 13 tools |
| Swift | ✅ Full | 13 tools |
| Kotlin | ✅ Full | 13 tools |
| Solidity | ✅ Implemented | 10 tools (taint disabled) |
| Vyper | ⚠️ Partial (parser limited) | 8 tools |
| OCaml | ✅ Implemented | 13 tools |

## GitHub Actions Integration

```yaml
name: Code Quality
on: [push, pull_request]

jobs:
  codemetrics:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install CodeMetrics
        run: cargo install codemetrics-cli
      - name: Run audit
        run: codemetrics run . --format sarif > results.sarif
      - name: Upload SARIF
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: results.sarif
```

## AI Agent Integration

```bash
# Discover all tools, schemas, rule IDs
codemetrics discover --format json

# NDJSON stream for incremental processing
codemetrics run . --format ndjson | jq -c 'select(.severity=="high")'

# Record history for trend analysis
codemetrics run . --format json | codemetrics history record --report /dev/stdin
```

## Hermes Agent Skills

Hermes agents can load CodeMetrics skills directly:

```bash
skill: load(name="codemetrics-analyzer")   # multi-language orchestration
skill: load(name="codemetrics-explore")    # tool discovery & usage
skill: load(name="codemetrics-workspace")  # workspace patterns
```

Skills are organized under `hermes/devops/` and `hermes/software-development/`:

| Skill | Purpose |
|-------|---------|
| **codemetrics-analyzer** | Multi-language orchestration patterns for batch audits across polyglot repos |
| **codemetrics-explore** | CLI discovery, individual tool usage, JSON/NDJSON output parsing |
| **codemetrics-http-debug-patterns** | Debugging `codemetrics-server` API responses and HTTP error patterns |
| **codemetrics-observability-and-ops** | CI/CD integration, metrics collection, health checks, alerting |
| **codemetrics-performance-optimization** | Tuning parser pools, parallelism bounds, memory limits |
| **rust-codemetrics-analysis** | Rust-specific CRAP/debt/doccov/mutation workflows |
| **agent-quality-workflow** | End-to-end detect→fix→verify loop for autonomous agents |
| **codemetrics-workspace** | Building and maintaining a multi-crate Rust workspace of quality tools |

## Documentation

- [User Guide](./docs/user-guide.md) — How to use CodeMetrics to audit and improve your project
- [Developer Guide](./docs/developer-guide.md) — Architecture, adding new tools, testing patterns
- [Metrics Explained](./docs/metrics-explained.md) — Detailed metric definitions and fix strategies
- [Quality Standards](./docs/codemetrics-standards.md) — Thresholds and gating criteria
- [UTCP Integration](./docs/utcp-integration.md) — Universal Tool Context Protocol for AI agents

## Crate Reference

| Crate | Type | Purpose |
|-------|------|---------|
| `codemetrics-cli` | Binary | Main CLI (`codemetrics` command) |
| `codemetrics-common` | Library | Shared utilities (coverage, CRAP, file discovery) |
| `codemetrics-server` | Binary | HTTP JSON-RPC API server |
| `crap-metric` | Binary | CRAP scoring |
| `mutation-test` | Binary | Mutation testing (Rust only) |
| `debt-scan` | Binary | Technical debt scanner |
| `doc-coverage` | Binary | Documentation coverage |
| `duplication` | Binary | Code duplication detection |
| `coupling` | Binary | Module coupling analysis |
| `risk-map` | Binary | Churn × complexity analysis |
| `taint-scan` | Binary | Taint analysis (sensitive data flow) |
| `fuzz-surface` | Binary | Fuzzable function detection |
| `prop-cov` | Binary | Property coverage analysis |

## License

Apache-2.0 OR OPL-1.1 — choose whichever suits your project.