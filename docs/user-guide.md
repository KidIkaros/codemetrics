# User Guide — Using quality-tools to Improve Your Project

This guide shows how to use quality-tools to audit and improve your project's code quality.

## Quick Start (5 Minutes)

1. **Install quality-tools**:
   ```bash
   git clone https://github.com/your-repo/quality-tools.git
   cd quality-tools && cargo build --release
   export PATH="$PWD/target/release:$PATH"
   ```

2. **Audit your project**:
   ```bash
   quality run /path/to/your/project --format table
   ```

3. **Interpret results**:
   - Look for `✗` marks (failing checks)
   - Check the "How to Fix" column (if using `--explain`)
   - Focus on High/Critical severity items first

## Using Individual Tools

### CRAP Metric (Maintenance Risk)
```bash
crap ./src --recursive --explain
```
- **Target**: CRAP < 15 per function
- **Fix**: Reduce complexity (split functions) + increase test coverage

### Technical Debt Scan
```bash
debt ./src --recursive --explain
```
- **Target**: 0 TODO/FIXME/HACK markers
- **Fix**: Address each marker or convert to tracked issues

### Documentation Coverage
```bash
doccov ./src --recursive --min 95
```
- **Target**: >95% public API documentation
- **Fix**: Add doc comments to all public functions/types

### Code Duplication
```bash
dupfind ./src --recursive --min-lines 3
```
- **Target**: 0 duplication blocks >3 lines
- **Fix**: Extract duplicated code into shared functions

### Fuzz Surface Analysis
```bash
fuzz ./src --recursive --min-score 30
```
- **Target**: Identify high-value fuzz targets
- **Fix**: Add fuzz harnesses for flagged functions

## Batch Mode (Recommended)

Use the unified CLI for full audits:
```bash
# Generate config
quality init .

# Edit .quality.toml to set your targets
vim .quality.toml

# Run full audit
quality run . --format sarif --baseline .quality-baseline.sarif
```

## CI Integration

Add to your GitHub Actions workflow:
```yaml
- name: Quality Check
  run: |
    quality run . --format sarif
    # Fails if standards not met
```

## Understanding Reports

### CRAP Score Explanation
- **0-5**: Excellent (low risk)
- **5-15**: Good (acceptable)
- **15-30**: Poor (needs improvement)
- **>30**: Critical (must fix)

Formula: `CRAP = complexity² × (1 - coverage/100)³ + complexity`

### Severity Levels
- **Critical**: Immediate fix required (blocks merge)
- **High**: Fix before next release
- **Medium**: Fix when convenient
- **Low**: Optional improvement

## Getting Help

- See [Metrics Explained](./metrics-explained.md) for detailed metric definitions
- See [Quality Standards](./quality-standards.md) for target thresholds
- Open an issue for bugs or feature requests
