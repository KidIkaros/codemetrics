---
name: codemetrics-analyzer
description: Multi-language code quality orchestration
version: 1.0.0
maintainer: mo
category: devops
tags: ["code-codemetrics", "rust", "python", "javascript", "static-analysis", "technical-debt"]
dependencies:
  - CodeMetrics (Rust workspace binary at target/release/codemetrics)
  - matplotlib
  - jinja2
  - rich
compatibility:
  hermes: ">=0.6.0"
  python: ">=3.9"
---

## Overview

Comprehensive code quality skill wrapping the **CodeMetrics** Rust workspace (8.4K LOC, 15 crates). Exposes 12 specialized tools with unified Rich terminal output and HTML dashboard reports.

## Architecture

```
Hermes → codemetrics-analyzer (Python subprocess bridge) → codemetrics (Rust binary)
                              └─→ DashboardGen (matplotlib + Jinja2)
```

## Exposed Tools

codemetrics_check(path, recursive, coverage, max_crap, min_doc, max_debt, skip)
  Run all codemetrics checks bundled (codemetrics run CLI)

codemetrics_crap(path, recursive, coverage)
  CRAP metric — change-risk anti-patterns (Rust-only, cargo required)

codemetrics_debt(path, recursive, marker)
  Technical debt scan — TODO/FIXME/HACK markers (tree-sitter)

codemetrics_docs(path, recursive)
  Documentation coverage — public API docs % (tree-sitter)

codemetrics_complexity(path, recursive, min_complexity)
  Cyclomatic complexity — functions exceeding threshold (tree-sitter)

codemetrics_duplication(path, recursive, min_lines)
  Code duplication — copy-pasted blocks (tree-sitter)

codemetrics_coupling(path, min_coupling)
  Module coupling — fan-in/fan-out dependency graphs (tree-sitter)

codemetrics_risk(path, since, min_risk)
  Risk map — git churn × complexity hotspot scoring (tree-sitter)

codemetrics_taint(path, recursive, attribute, severity)
  Taint analysis — sensitive dataflow to sinks (tree-sitter)

codemetrics_mutation(path, files, max_mutants, timeout)
  Mutation testing — test suite codemetrics via intentional bugs (Rust-only)

codemetrics_fuzz(path, recursive, min_score, top)
  Fuzz surface analyzer — functions ideal for fuzzing (Rust-only)

codemetrics_propcov(path, recursive, only_tests, min_coverage)
  Property-based test coverage — proptest/quickcheck macro scan (tree-sitter)

codemetrics_languages()
  Returns language support matrix (Rust-only vs tree-sitter parity)

codemetrics_dashboard(report_json, output_path)
  Generate HTML dashboard from JSON results (matplotlib + Jinja2)
```

## Configuration

```toml
[codemetrics-analyzer]
# Binary location — auto-detected
codemetrics_binary = "target/release/codemetrics"
parallel_jobs = 4                    # Parallel file parsing
timeout_seconds = 120               # Per-tool timeout
dashboard_template = "templates/codemetrics_dashboard.html"
```

## Output

- **Terminal:** Rich live stages with spinners and color-coded status
- **JSON:** Structured ToolResult objects for each module
- **Dashboard:** `quality_report.html` with radar, bar, and hotspot charts

## Usage Example

```python
from hermes_skills.devops import codemetrics_analyzer

# Run bundled check
result = codemetrics_analyzer.codemetrics_check(
  path="$HOME/hermes-agent",
  recursive=True,
  coverage="/path/to/lcov.info",
  max_crap=30.0,
  min_doc=70.0,
  max_debt=100
)

# Generate HTML dashboard
html = codemetrics_analyzer.codemetrics_dashboard(result, "/tmp/report.html")
```

## Differential

vs `persona-spec`: 4 tools only — ~20% surface
vs `codemetrics-analyzer`: 12 tools — full feature matrix, language parity honest, visual dashboard

## Status

Phase 1 complete — skill launched with full toolset. Dashboard polish in progress.
