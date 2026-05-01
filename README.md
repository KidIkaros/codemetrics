# quality-tools

[![Quality Audit](https://github.com/your-org/quality-tools/actions/workflows/quality.yml/badge.svg)](https://github.com/your-org/quality-tools/actions/workflows/quality.yml)
[![Docs](https://img.shields.io/badge/docs-available-brightgreen)](./docs/)
[![ONBOARDING](https://img.shields.io/badge/onboarding-available-brightgreen)](./ONBOARDING.md)

## Documentation

- [User Guide](./docs/user-guide.md) — How to use quality-tools to audit and improve your project
- [Developer Guide](./docs/developer-guide.md) — Architecture, adding new tools, testing patterns
- [Metrics Explained](./docs/metrics-explained.md) — Detailed metric definitions, what scores mean, how to fix
- [Quality Standards](./docs/quality-standards.md) — "Exceeding Standards" targets and quality gates

Code quality metrics for 10+ languages via `tree-sitter`. All analysis is language-agnostic — no compilation required.

## Crates

| Crate | Binary | Purpose |
|-------|--------|---------|
| `ast-parse-ts` | (lib) | Universal AST parsing (tree-sitter) -- complexity, doc coverage, fingerprints, imports, identifiers for 12+ languages |
| `quality-common` | (lib) | Shared utilities -- coverage parsing, CRAP scoring, source file discovery, output formatting |
| `quality-cli` | `quality` | Unified CLI -- runs all tools in one batch with CI-ready JSON/SARIF output |
| `crap-metric` | `crap` | CRAP score calculator -- maintenance risk scoring (multi-language) |
| `mutation-test` | `mutate` | Mutation testing -- evaluate test suite quality (Rust-only) |
| `debt-scan` | `debt` | Technical debt scanner -- TODO/FIXME/HACK tracking with git blame |
| `doc-coverage` | `doccov` | Documentation coverage -- public API doc comment percentage |
| `duplication` | `dupfind` | Code duplication -- AST-based structural similarity detection |
| `coupling` | `coupling` | Coupling analysis -- module dependency graphs, fan-in/fan-out |
| `risk-map` | `riskmap` | Risk map -- churn × complexity cross-reference (the killer feature) |
| `taint-scan` | `taint` | Taint analysis -- detect sensitive data flow to sinks |
| `fuzz-surface` | `fuzz` | Fuzz surface analysis -- identify fuzzable functions (Python, JS, Go, Rust) |
| `prop-cov` | `propcov` | Property test coverage -- detect property-based and unit tests (multi-language) |

## Multi-Language Support

The `ast-parse-ts` crate uses tree-sitter grammars (pure Rust, no external dependencies) to analyze source files directly — no compilation needed. Now supports 12 languages: Rust, Python, JavaScript, TypeScript, Go, C, C++, C#, Java, PHP, Ruby, Swift, and Kotlin (partial).

## AI-Native Toolkit

Designed for headless AI agent integration with:

- **Tool Discovery**: `quality discover --format json` outputs all available tools, their formats, output fields, and rule IDs for programmatic consumption
- **Self-Contained Findings**: All JSON/NDJSON outputs include `severity`, `help`, and `rule_id` fields so AI agents can explain and fix issues without reading docs
- **Streaming Support**: NDJSON format enables incremental processing for AI pipelines
- **Standardized Output**: Consistent fields across all tools for reliable parsing

| Tool | Rust | Python | JS/TS | Go | C/C++ | C# | Java | PHP | Ruby | Swift |
|------|:----:|:------:|:-----:|:--:|:-----:|:--:|:----:|:---:|:----:|:-----:|
| `debt-scan` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `taint-scan` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `complexity` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `doc-coverage` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `duplication` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `coupling` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `risk-map` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `crap-metric` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `mutation-test` | ✓ | — | — | — | — | — | — | — | — | — |
| `fuzz-surface` | ✓ | ✓ | ✓ | ✓ | — | — | — | — | — | — |
| `prop-cov` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

**Note:** `mutation-test` (`mutate`) requires `cargo` and is Rust-only. All other tools run directly on source code for all supported languages.

## CRAP Metric

The CRAP (Change Risk Anti-Patterns) score estimates maintenance risk by combining cyclomatic complexity with test coverage:

```
CRAP = comp^2 * (1 - coverage/100)^3 + comp
```

- **comp** = cyclomatic complexity (number of decision points)
- **coverage** = percentage of code covered by automated tests
- Score > 30 = "crappy" code that is risky to maintain

### Usage

```bash
# Analyze a Rust crate (no coverage data)
crap ./crates/my-crate/src --recursive

# Analyze Python files
crap ./my-python-project --recursive

# Analyze TypeScript files
crap ./my-web-app --recursive

# With lcov coverage file
crap ./crates/my-crate/src --recursive --coverage coverage.info

# With coverage override
crap ./crates/my-crate/src --recursive --coverage-pct 75

# JSON output
crap ./crates/my-crate/src --recursive --format json

# Only show high-risk functions
crap ./crates/my-crate/src --recursive --min-score 20
```

### Output

```
FUNCTION                       FILE                      LINE COMP   CRAP CATEGORY
──────────────────────────────────────────────────────────────────────────────
parse_era835                   src/lib.rs                 330   54  2970.0 ✗ crappy
carc_description               src/lib.rs                 244   59  3540.0 ✗ crappy
parse_cas                      src/lib.rs                 560    4    20.0 ○ good
parse_svc                      src/lib.rs                 587    2     6.0 ✓ excellent
```

## Mutation Testing

Mutation testing evaluates test suite quality by introducing deliberate changes (mutants) to source code. If tests still pass with a mutation, the test suite has a gap.

### Mutation Strategies

1. **Binary operator swaps**: `+` <-> `-`, `==` <-> `!=`, `&&` <-> `||`, etc.
2. **Boolean literal swaps**: `true` <-> `false`
3. **Boundary mutations**: `<` <-> `<=`, `>` <-> `>=`

### Usage

```bash
# Test a crate (runs cargo test for each mutant)
mutate ./crates/my-crate --max-mutants 20

# Test specific files only
mutate ./crates/my-crate --files src/lib.rs,src/parser.rs

# With custom timeout
mutate ./crates/my-crate --timeout 60

# JSON output
mutate ./crates/my-crate --format json

# With environment variables (e.g., CARGO_TARGET_DIR for FAT32)
CARGO_TARGET_DIR=/tmp/build mutate ./crates/my-crate
```

### Output

```
[1/10] Testing mutant 1 (src/lib.rs:569)... ✗ SURVIVED
[2/10] Testing mutant 2 (src/lib.rs:571)... ✓ KILLED
...

SUMMARY
  Total mutants:  10
  Killed:         6 (60%)
  Survived:       4 (40%)
  Mutation Score: 60%
  Verdict:        Weak — many mutations survived
```

## Building

```bash
# Standard build
cargo build

# FAT32 target directory (if build path doesn't support exec permissions)
CARGO_TARGET_DIR=/tmp/quality-tools-build cargo build

# Run tests (single crate — safe)
cargo test

# Run tests across the entire workspace without freezing your OS
# (uses batched compilation to cap peak memory)
./test.sh

# Ultra-safe mode: one crate at a time, single build job
./test.sh --safe
```

## Other Tools

### Technical Debt Scanner (`debt`)

```bash
# Scan for TODO/FIXME/HACK/XXX markers
debt ./src --recursive

# Only show FIXME and HACK
debt ./src --recursive --marker fixme,hack

# Sort by author
debt ./src --recursive --sort author
```

### Fuzz Surface Analysis (`fuzz`)

```bash
# Analyze Python files for fuzzable functions
fuzz ./my-python-project --recursive

# Analyze JavaScript/TypeScript files
fuzz ./my-web-app --recursive

# Analyze Go files
fuzz ./my-go-service --recursive

# Only show functions with high fuzzability score
fuzz ./src --recursive --min-score 30

# Show top 10 most fuzzable functions
fuzz ./src --recursive --top 10
```

### Documentation Coverage (`doccov`)

```bash
# Check public API documentation
doccov ./src --recursive

# Fail if below 80% coverage
doccov ./src --recursive --min 80
```

### Code Duplication (`dupfind`)

```bash
# Find structural duplicates (min 5 lines)
dupfind ./src --recursive

# Stricter: min 10 lines
dupfind ./src --recursive --min-lines 10
```

### Coupling Analysis (`coupling`)

```bash
# Module dependency graph
coupling ./

# Export as Graphviz dot
coupling ./ --format dot > deps.dot && dot -Tpng deps.dot -o deps.png

# Only show tightly coupled modules
coupling ./ --min-coupling 5
```

### Risk Map (`riskmap`)

```bash
# Cross-reference git churn with complexity
riskmap ./

# Only last 3 months
riskmap ./ --since "3 months ago"

# Only show risk score >= 30
riskmap ./ --min-risk 30
```

### Unified CLI (`quality`)

The `quality` CLI runs all tools in one batch, detects languages automatically, and produces CI-ready JSON/SARIF output.

```bash
# Full audit (auto-detects languages)
quality run . --format json

# Watch mode — re-run checks on .rs file changes
quality watch . --checks debt,doc,crap --debounce-ms 500

# Record run to history
quality run . --format json | quality history record --report /dev/stdin

# Show trend history
quality history show

# Install pre-commit hook
quality install-hooks .
quality uninstall-hooks .
```

**Multi-language example:**
```bash
# Scan a mixed Python/JS/Rust repo
quality run ./my-project --format json
# → summary.languages_detected: ["javascript", "python", "rust"]
```

## Performance

`ast-parse-ts` maintains a **thread-local parser pool** — tree-sitter `Parser` instances are created once per thread per language and reused across files. Heavy tools (`duplication`, `taint-scan`, `coupling`) use **bounded `rayon` parallelism** (2 threads) for file scanning, and the batch runner (`quality`) caps concurrent tool execution at 4 to respect CI RAM limits.

## License

Apache-2.0 OR OPL-1.1
