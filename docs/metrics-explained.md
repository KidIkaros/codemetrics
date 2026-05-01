# Metrics Explained — Understanding Your Quality Report

This document explains each metric, what it means, why it matters, and how to fix issues.

## 1. CRAP Score (Change Risk Anti-Patterns)

### What it is
Estimates maintenance risk by combining cyclomatic complexity with test coverage.

Formula: `CRAP = complexity² × (1 - coverage/100)³ + complexity`

### Thresholds
| Score | Category | Action |
|-------|----------|--------|
| 0-5 | Excellent | No action needed |
| 5-15 | Good | Acceptable |
| 15-30 | Poor | Should improve |
| >30 | Critical | Must fix immediately |

### Why it matters
High CRAP scores indicate code that is:
- Hard to understand (high complexity)
- Not well-tested (low coverage)
- Risky to modify (changes likely to introduce bugs)

### How to fix
1. **Reduce complexity** (preferred):
   - Split large functions into smaller ones
   - Extract complex logic into helper functions
   - Reduce nesting (early returns, guard clauses)

2. **Increase test coverage**:
   - Add unit tests for untested branches
   - Use property-based testing for edge cases
   - Aim for >90% coverage

Example fix:
```rust
// Before: CRAP 25 (complexity 10, coverage 60%)
fn process(data: &[i32]) -> i32 {
    let mut total = 0;
    for &x in data {
        if x > 0 { total += x; } // Complex nested logic
        else if x < 0 { total -= x; }
        else { total += 0; }
    }
    total
}

// After: CRAP 3 (complexity 2, coverage 95%)
fn process(data: &[i32]) -> i32 {
    data.iter().map(|&x| if x > 0 { x } else { -x }).sum()
}
```

## 2. Cyclomatic Complexity

### What it is
Counts decision points (if, for, while, match, etc.) in a function.

### Thresholds
| Complexity | Action |
|------------|--------|
| <5 | Excellent |
| 5-10 | Acceptable |
| 10-15 | Should simplify |
| >15 | Must refactor |

### Why it matters
- High complexity = harder to test (exponential test cases)
- High complexity = more likely to have bugs
- High complexity = harder for new developers to understand

### How to fix
1. Extract methods (split into smaller functions)
2. Use early returns to reduce nesting
3. Replace nested conditionals with polymorphism/strategy pattern
4. Use data-driven approaches instead of long if-else chains

## 3. Technical Debt (TODO/FIXME/HACK/XXX)

### What it is
Markers in code indicating known issues that need future attention.

### Thresholds
- **Target**: 0 markers (zero tolerance)
- **Why**: Debt markers indicate known issues that accumulate and rot code

### How to fix
1. **TODO**: Implement the planned feature or remove if no longer needed
2. **FIXME**: Fix the bug immediately or create a tracked issue
3. **HACK**: Refactor to proper solution
4. **XXX**: Address the critical problem immediately

Tools:
```bash
# Find all debt markers
debt ./src --recursive --explain

# Auto-fix (if possible):
# 1. Create GitHub issues for each marker
# 2. Remove marker after fixing
```

## 4. Documentation Coverage

### What it is
Percentage of public APIs (functions, structs, traits) with doc comments.

### Thresholds
| Coverage | Action |
|----------|--------|
| >95% | Excellent |
| 80-95% | Good |
| 50-80% | Needs improvement |
| <50% | Critical |

### Why it matters
- Documentation helps new developers adopt your code
- Well-documented APIs are easier to maintain
- Doc comments can include examples that double as tests

### How to fix
1. Add `///` doc comments to all public items
2. Include examples in doc comments (they run as tests!)
3. Use `//!` for module-level docs

Example:
```rust
/// Calculates the CRAP score for a function.
///
/// # Arguments
/// * `complexity` - Cyclomatic complexity (decision points)
/// * `coverage` - Test coverage percentage (0-100)
///
/// # Returns
/// CRAP score (higher = riskier to maintain)
///
/// # Example
/// ```
/// let score = crap_score(10, 80.0);
/// assert!(score < 30.0);
/// ```
pub fn crap_score(complexity: u32, coverage: f64) -> f64 {
    // ...
}
```

## 5. Code Duplication

### What it is
Identical or nearly identical code blocks across files.

### Thresholds
- **Target**: 0 blocks >3 lines
- **Why**: Duplication increases maintenance burden (fix in N places)

### How to fix
1. Extract duplicated code into shared functions/modules
2. Use generics or macros to reduce repetition
3. Apply DRY (Don't Repeat Yourself) principle

Tools:
```bash
# Find duplicates
dupfind ./src --recursive --min-lines 3

# Fix: Extract duplicate into function
```

## 6. Mutation Score

### What it is
Percentage of mutants (deliberate bugs) that are caught by tests.

### Thresholds
| Score | Quality |
|-------|----------|
| >80% | Excellent |
| 60-80% | Good |
| 40-60% | Weak |
| <40% | Poor |

### Why it matters
- Low mutation score = tests don't catch regressions
- High mutation score = tests are effective at catching bugs

### How to fix
1. Add tests that would fail if logic changes
2. Test edge cases and error paths
3. Use property-based testing to cover more cases

## 7. Fuzz Surface

### What it is
Identifies functions that process external input and are good fuzzing targets.

### Scoring
- Higher score = more fuzzable (processes external input)
- Focus on high-score functions first

### How to fix (improve security)
1. Add fuzz harnesses for high-score functions
2. Use `cargo-fuzz` (Rust) or `go-fuzz` (Go)
3. Run fuzzing continuously (OSS-Fuzz)

## 8. Coupling (Module Dependencies)

### What it is
Measures how tightly modules depend on each other (fan-in/fan-out).

### Thresholds
- **High coupling** = changes ripple across modules
- **Target**: Low coupling, high cohesion

### How to fix
1. Use dependency inversion (interfaces/traits)
2. Reduce direct dependencies between modules
3. Apply hexagonal architecture patterns
