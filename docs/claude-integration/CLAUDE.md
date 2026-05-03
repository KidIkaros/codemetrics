# Code Quality Tools

This project uses **codemetrics** for automated code quality analysis. All AI collaborators should use these tools when requested or when making significant code changes.

## Quick Reference

| Tool | Purpose | Command |
|------|---------|---------|
| `crap` | Find high-risk functions | `cargo run -p crap-metric -- ./src --recursive` |
| `mutate` | Test suite quality | `cargo run -p mutation-test -- . -p <pkg> --max 5` |
| `debt` | Find TODOs/FIXMEs | `cargo run -p debt-scan -- ./src --recursive` |
| `riskmap` | Complex/churned files | `cargo run -p risk-map -- .` |
| `doccov` | Doc coverage | `cargo run -p doc-coverage -- ./src --recursive` |
| `quality` | Full audit | `cargo run -p codemetrics-cli -- run . --format sarif` |

## When to Use

- **Before PR**: Run `mutate . -p <crate> --max 5` to verify tests catch mutants
- **After major changes**: Run `cargo run -p codemetrics-cli -- run . --format sarif`
- **Finding bugs**: Use `riskmap` to identify high-risk files
- **Documentation task**: Use `doccov` to check coverage

## Tool-Specific Notes

### mutate
- Requires `cargo test` to pass on original code first
- Won't work on crates with failing tests
- Use `-p crate-name` to specify target package

### crap
- Functions with CRAP > 15 are risky to maintain
- CRAP > 30 = immediately fix before proceeding

### debt
- Zero tolerance: fix or track TODOs/FIXMEs before committing
- Use git blame to see who created the debt

## Output Formats

- `--format json` for programmatic parsing
- `--format sarif` for GitHub Security tab
- `--format ndjson` for streaming (quality tool)

## Building

```bash
cargo build --release  # Builds all tools
```

## See Also

- `utcps/codemetrics.json` - UTCP manual for tool definitions
- `docs/quality-standards.md` - Quality targets and gates