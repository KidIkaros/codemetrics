# CodeMetrics Examples

This directory contains example configurations and usage patterns for CodeMetrics.

## Files

- `.quality.toml` — Sample configuration file with thresholds for all supported checks

## Usage Examples

### Basic Check

```bash
# Run all quality checks
codemetrics check .

# Run with JSON output (for CI/automation)
codemetrics check . --format json

# Run with SARIF output (for GitHub Security tab)
codemetrics check . --format sarif > results.sarif
```

### Generate HTML Report

```bash
# Generate and open HTML report
codemetrics report . --open

# Generate markdown report
codemetrics report . --format markdown > quality-report.md
```

### Initialize Project

```bash
# Auto-detect project type and generate .quality.toml
codemetrics init

# Full CI bootstrap (config + GitHub Actions + pre-commit hook)
codemetrics init --ci
```

### Run Specific Checks

```bash
# Run only specific checks
codemetrics check . --only crap,debt,doc

# Run in CI mode (JSON output, no colors)
codemetrics check . --ci
```

### Watch Mode (Development)

```bash
# Watch for changes and run checks automatically
codemetrics watch .

# Watch with all checks (not just debt/doc/crap)
codemetrics watch . --full
```
