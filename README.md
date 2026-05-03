# CodeMetrics

<img src="logo.svg" alt="CodeMetrics Logo" width="400"/>

[![CI](https://github.com/KidIkaros/codemetrics/actions/workflows/codemetrics.yml/badge.svg)](https://github.com/KidIkaros/codemetrics/actions)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange)](https://rust-lang.org)
[![License](https://img.shields.io/badge/License-Apache--2.0%20%7C%20OPL--1.1-blue)](LICENSE)

**Production-grade code quality auditing** — ten specialized analysis engines unified under a single CLI, designed for CI/CD gatekeeping and autonomous AI agent integration.

---

## Problem

Most quality tooling falls into one of two buckets:

| Monolithic Suites | Point Solutions |
|---|---|
| All-or-nothing bundles, slow, opaque internals | Single-problem tools requiring cobbled-together pipelines |
| Ship every check whether you need it or not | No coordination, inconsistent output formats |

Both approaches struggle when you need **composable, machine-readable quality gates** that actually fit into modern automated workflows.

---

## Solution

**CodeMetrics** is a deliberate collection of focused analyzers — each does one thing well, all speak the same language (JSON/NDJSON/SARIF), and compose cleanly into pipelines.

Built in Rust with zero external runtime dependencies, designed from the start for **AI agent consumption** and **CI/CD integration**.

---

## Ten Engine Suite

| Engine | Promise | Output |
|---|---|---|
| **crap** | CRAP score per function (complexity × coverage risk) | Function-level rankings |
| **mutate** | Mutation testing — test suite kill rate | Score % + surviving mutants |
| **debt** | Technical debt inventory (TODO/FIXME/HACK/XXX) | Author-grouped heatmap |
| **riskmap** | Churn × complexity hot spot identification | Ranked file list |
| **doccov** | Public API documentation coverage | Module-level % |
| **taint** | Sensitive data flow tracing (secrets, logs) | Paths with source→sink |
| **fuzz** | Fuzzable entry point detection | Fuzzability scores |
| **coupling** | Dependency analysis (cycles, fan-in/out) | Coupling matrix |
| **dupfind** | AST-based duplication detection | Duplicate blocks |
| **propcov** | Property test coverage | Coverage % |

Invoke individually (`codemetrics crap src/`) or run the full battery (`codemetrics run .`).

---

## Why CodeMetrics

### AI-Native Architecture
- Structured JSON/NDJSON output — consumable by agents without text parsing
- Deterministic exit codes signal pass/fail for autonomous decision-making
- First-class Hermes Agent skills included under `hermes/`

### Production-Ready Rigor
- CI-validated on its own codebase (self-hosting)
- Output schemas available in `schemas/` for validation
- SARIF support for GitHub Security tab integration

### Practical Design
- Zero configuration — auto-detects 15 languages out of the box
- No cloud services — tree-sitter based, fully local analysis
- Single dependency chain (Rust toolchain only)

---

## Getting Started

```bash
# Clone and build
git clone https://github.com/KidIkaros/codemetrics.git
cd codemetrics
cargo build --release

# Run full suite on current project
./target/release/codemetrics run .

# Single-tool example (CRAP scoring)
./target/release/codemetrics crap ./src --recursive

# JSON output for script/agent consumption
./target/release/codemetrics run . --format json --output report.json
```

Install from crates.io when published:
```bash
cargo install codemetrics-cli
```

---

## Typical Workflows

- **Pre-commit quality gate** — fail PRs if `crap` threshold exceeded or mutation score drops
- **Refactoring prioritization** — `riskmap` pinpoints files with highest change-complexity risk
- **Security audit** — `taint` traces credential leakage paths across language boundaries
- **Documentation audit** — `doccov` surfaces undocumented public APIs before release
- **Test strength assessment** — `mutate` measures defect detection capability beyond coverage numbers

---

## AI Agent Integration

CodeMetrics ships with **Hermes Agent skill definitions** under `hermes/`. Each skill wraps a tool, parses structured results, and exposes thresholds for autonomous operation.

See [`docs/agents/AGENTS.md`](docs/agents/AGENTS.md) for the skill catalog and usage patterns.

---

## Output Formats

| Format | Use Case |
|---|---|
| **JSON** | Structured parsing, agent workflows |
| **NDJSON** | Streaming pipelines, log aggregation |
| **SARIF** | GitHub Security tab, static analysis tooling ecosystem |
| **Human** | Terminal review (default) |

All tools accept `--format <json\|ndjson\|sarif\|text>`.

---

## Project Health

| Status | Detail |
|---|---|
| Current Release | Stable v1.0.0 |
| CI Pipeline | ✅ All 10 tools green |
| Schema Validation | ✅ JSON schemas published in `schemas/` |
| Test Suite | `cargo test` — workspace-wide passing (2 known flaky edge cases ignored) |
| Self-Hosting | Runs on its own codebase continuously |

See [`PROJECT_STATUS.md`](PROJECT_STATUS.md) for roadmap and known limitations.

---

## Documentation

- [User Guide](./docs/user-guide.md) — CLI reference and output interpretation
- [Developer Guide](./docs/developer-guide.md) — crate architecture and extending the suite
- [Hermes Integration](./docs/utcp-integration.md) — wiring CodeMetrics into AI agent workflows
- [Schema Reference](./schemas/) — JSON/NDJSON/SARIF output contracts

---

## Contributing

CodeMetrics is open source under **Apache-2.0 / OPL-1.1** dual licensing. Contributions welcome — see `docs/developer-guide.md` for development setup and contribution guidelines.
