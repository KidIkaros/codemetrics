# quality-tools

Code quality metrics for Rust. Two CLI tools built on a shared AST parsing library.

## Crates

| Crate | Binary | Purpose |
|-------|--------|---------|
| `ast-parse` | (lib) | Shared AST parsing -- cyclomatic complexity, lcov coverage parsing |
| `crap-metric` | `crap` | CRAP score calculator -- maintenance risk scoring |
| `mutation-test` | `mutate` | Mutation testing -- evaluate test suite quality |
| `debt-scan` | `debt` | Technical debt scanner -- TODO/FIXME/HACK tracking with git blame |
| `doc-coverage` | `doccov` | Documentation coverage -- public API doc comment percentage |
| `duplication` | `dupfind` | Code duplication -- AST-based structural similarity detection |
| `coupling` | `coupling` | Coupling analysis -- module dependency graphs, fan-in/fan-out |
| `risk-map` | `riskmap` | Risk map -- churn × complexity cross-reference (the killer feature) |

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
# Analyze a crate (no coverage data)
crap ./crates/my-crate/src --recursive

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

# Run tests
cargo test
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

## License

Apache-2.0 OR OPL-1.1
