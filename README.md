# CodeMetrics

<img src="logo.svg" alt="CodeMetrics Logo" width="400"/>

[![CodeMetrics CI](https://github.com/KidIkaros/codemetrics/actions/workflows/codemetrics.yml/badge.svg)](https://github.com/KidIkaros/codemetrics/actions)
[![Docs](https://img.shields.io/badge/docs-available-brightgreen)](./docs/)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange)](https://rust-lang.org)
[![License](https://img.shields.io/badge/License-Apache--2.0%20%7C%20OPL--1.1-blue)](LICENSE)

**AI-native code quality audit toolkit** — 10 automated analysis tools unified under one CLI, built for CI/CD pipelines and autonomous AI agent workflows.

## Why This Exists

Most quality tools are either:
- **Batteries-included monoliths** (slow, all-or-nothing)
- **Point solutions** (one tool per problem → 10 tools to manage)

CodeMetrics gives you **best-of-both-worlds**: ten focused tools that compose, with structured output designed for machine consumption from day one.

---

## Highlights

| | | |
|---|---|---|
| 🧠 **AI-Ready** | JSON/NDJSON output, Hermes Agent skill integration, deterministic exit codes |
| ⚡ **Zero Config** | Auto-detects 15 languages, sensible defaults, no YAML required |
| 🔒 **No External Services** | Pure local analysis — tree-sitter, no cloud API keys |
| 🛡️ **Production Dogfood** | Runs on its own codebase at `github.com/KidIkaros/codemetrics` |
| 📦 **Bootstrap Friendly** | Cargo install or build from source — no external dependencies beyond Rust |

---

## Quick Install

```bash
# From source (recommended — latest)
git clone https://github.com/KidIkaros/codemetrics.git
cd codemetrics
cargo build --release
./target/release/codemetrics --help

# Or install binary if/when published
cargo install codemetrics-cli
```

## One-Minute Demo

```bash
# Full security-audit style scan (SARIF for GitHub Security tab)
codemetrics run . --format sarif > results.sarif

# Focus on maintainability risk
codemetrics crap ./src --recursive

# Measure test suite strength via mutation testing
codemetrics mutate . --max-mutants 20

# JSON output for AI agent consumption
codemetrics run . --format json --output report.json
```

All tools accept `--format json` or `--format ndjson`; exit codes reflect pass/fail for CI gating.

---

## The 10 Tools

| Tool | What It Does | Output |
|------|--------------|--------|
| `crap` | CRAP (Change Risk Anti-Patterns) scoring — complexity × coverage | function-level risk scores |
| `mutate` | Mutation testing — how many mutants do your tests catch? | mutation score % |
| `debt` | TODO/FIXME/HACK/XXX debt inventory grouped by author | debt heatmap |
| `riskmap` | Churn × complexity hot spots (high-risk files) | ranked file list |
| `doccov` | Public API documentation coverage (%) | module coverage % |
| `taint` | Sensitive data flow tracing (log leaks, secret exposure) | taint paths |
| `fuzz` | Fuzzable function detection | fuzzability scores |
| `coupling` | Module dependency analysis (cycles, fan-in/out) | coupling matrix |
| `dupfind` | Code duplication detection (AST-based) | duplicate blocks |
| `propcov` | Property test coverage | coverage % |

Run them individually or use `codemetrics run .` to execute the full battery.

---

## AI Agent Integration

CodeMetrics ships with **first-class AI agent support** — skills for Hermes Agent are bundled directly in this repo under `hermes/`.

Agents can invoke tools autonomously, parse structured JSON results, and act on risk thresholds without human intervention.

See [`hermes/README.md`](hermes/README.md) for the full skill catalog.

---

## Use Cases

- **Pre-commit Quality Gate** — fail PRs if CRAP > 30 or mutation score < 70%
- **Refactoring Prioritization** — `riskmap` identifies the most dangerous files
- **Security Audit** — `taint` traces sensitive data paths across languages
- **Documentation Gaps** — `doccov` highlights undocumented public APIs
- **Test Strength** — `mutate` quantifies suite weakness beyond coverage %

---

## Project Status

Current state: **Stable v1.0.0** (public GitHub)

- ✅ All 10 tools green in CI
- ✅ JSON/SARIF output validated against schemas in `schemas/`
- ✅ Hermes Agent skills exported for AI-native workflows
- 📦 License: Apache-2.0 for core, OPL-1.1 for extensions

See [`PROJECT_STATUS.md`](PROJECT_STATUS.md) for roadmap and known limitations.

---

## Getting Help

- 📚 [Documentation](./docs/) — user guide, developer guide, integration patterns
- 🐛 [Issues](https://github.com/KidIkaros/codemetrics/issues) — bug reports, feature requests
- 💬 [Discussions](https://github.com/KidIkaros/codemetrics/discussions) — questions, show-and-tell
