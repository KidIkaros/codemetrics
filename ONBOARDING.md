# ONBOARDING - Quickstart Guide

Welcome to quality-tools! This guide will get you up and running in 5 minutes.

## 5-Minute Quickstart

### 1. Build the Tools
```bash
cargo build --workspace
```

### 2. Run Your First Audit
```bash
# Analyze a Rust project
./target/debug/quality run ./your-project

# Or use individual tools
./target/debug/crap ./your-project/src --recursive
./target/debug/doccov ./your-project/src --recursive
./target/debug/debt ./your-project/src --recursive
```

### 3. Understand the Output
The `quality run` command outputs:
- **Table mode** (default): Human-readable summary
- **JSON mode**: Machine-readable for CI (`--format json`)
- **SARIF mode**: GitHub Security tab integration (`--format sarif`)

## How to Interpret Quality Reports

### CRAP Score (crap-metric)
- **What it means**: Combination of complexity + test coverage
- **Formula**: `CRAP = complexity² × (1 - coverage/100)³ + complexity`
- **Good**: < 15 (exceeding standard), < 30 (industry standard)
- **Fix**: Reduce complexity OR increase test coverage

### Technical Debt (debt-scan)
- **What it means**: TODO/FIXME/HACK markers in code
- **Target**: 0 markers (zero tolerance)
- **Fix**: Create issues in tracker, remove markers from code

### Documentation Coverage (doc-coverage)
- **What it means**: Percentage of public APIs with doc comments
- **Target**: > 95% (exceeding standard)
- **Fix**: Add `///` or `/*! */` doc comments to public items

### Complexity (ast-parse-ts)
- **What it means**: Cyclomatic complexity (decision points)
- **Target**: < 5 per function (exceeding standard)
- **Fix**: Split complex functions, reduce nesting

## Common Fixes for Typical Findings

### "Function X has CRAP score 25"
1. Check complexity: `crap --format json | jq '.functions[] | select(.name=="X")'`
2. If complexity > 5: Split the function into smaller parts
3. If coverage < 90%: Add tests for untested lines

### "Technical debt marker found: TODO"
1. Create a GitHub issue describing the work
2. Remove the TODO/FIXME/HACK marker from code
3. Commit with: "Track work in #123, remove TODO marker"

### "Documentation coverage at 60%"
1. Run `doccov ./src --recursive` to see gaps
2. Add doc comments to public functions/structs
3. Target: > 95% coverage for public APIs

### "Function Y has complexity 12"
1. Identify decision points (if/else, loops, match)
2. Extract complex logic into helper functions
3. Use early returns to reduce nesting

## Next Steps

1. Read [docs/user-guide.md](docs/user-guide.md) for detailed usage
2. Read [docs/quality-standards.md](docs/quality-standards.md) for target metrics
3. Run `quality run .` on this repo to see self-audit in action
4. Set up pre-commit hooks: `quality install-hooks .`

## Need Help?

- Check [docs/metrics-explained.md](docs/metrics-explained.md) for metric details
- Check [docs/developer-guide.md](docs/developer-guide.md) for architecture
- Open an issue at: https://github.com/your-org/quality-tools/issues
