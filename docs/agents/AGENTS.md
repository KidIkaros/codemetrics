# Code Quality Tools

This project uses **CodeMetrics** for automated code quality analysis. AI agents should use these tools to audit code quality.

## Quality Tools Overview

| Tool | What It Does | When to Use |
|------|-------------|-------------|
| **crap** | CRAP risk scores (# > 30 = fix before proceeding) | Before merge on high-risk code |
| **mutate** | Mutation testing (surviving mutants = weak tests) | Evaluate test suite quality |
| **debt** | Technical debt markers (TODO/FIXME/HACK/XXX) | Before PR, zero tolerance |
| **riskmap** | Files changing often AND complex | Identify bug risk |
| **doccov** | Documentation coverage (%) | Check public API docs |
| **taint** | Sensitive data flow | Security audit |
| **codemetrics** | Full batch audit | CI/CD pipeline |

## Usage Commands

### Full Audit (Recommended)

```
codemetrics run . --format sarif > results.sarif
```

### Individual Tools

```
# CRAP scoring
cargo run -p crap-metric -- ./src --recursive --format json

# Mutation testing
cargo run -p mutation-test -- . -p crate-name --max-mutants 5

# Risk map  
cargo run -p risk-map -- . --min-risk 30

# Technical debt
cargo run -p debt-scan -- ./src --recursive
```

## Important Notes

- **mutate** requires `cargo test` to pass first - won't work on broken tests
- Use `-p crate-name` with mutate for workspace crates
- SARIF format works with GitHub Security tab
- All tools support `--format json` for programmatic parsing

## Tool Priority for AI Agents

1. **crap** - Find high-risk functions needing tests
2. **mutate** - Verify test suite catches mutations
3. **debt** - Report any TODOs/FIXMEs
4. **riskmap** - Identify complex/churned files

## See Also

- `utcps/codemetrics.json` - UTCP tool definitions
- `docs/quality-standards.md` - Quality targets and thresholds