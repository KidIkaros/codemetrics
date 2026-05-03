# Publishing to crates.io

CodeMetrics follows standard Rust crate publishing. This guide walks through publishing all workspace crates to [crates.io](https://crates.io).

---

## Prerequisites

1. **crates.io account** — register at https://crates.io
2. **API token** — generate at https://crates.io/settings/tokens (scope: `crates:write`)
3. **Git tag pushed** — v1.0.0 tag must exist on GitHub (done: `git push origin v1.0.0`)
4. **Clean workspace** — no uncommitted changes, all tests pass

---

## Step-by-Step

### 1. Login to crates.io

```bash
cargo login
# Paste your API token when prompted
```

This stores the token in `~/.cargo/credentials`.

---

### 2. Publish in Dependency Order

Workspace crates have inter-dependencies. Publish in this order to avoid "dependency not found" errors:

```bash
# 1. Common utilities (used by everything)
cargo publish -p codemetrics-common

# 2. Core analysis crates (order not critical among these)
cargo publish -p ast-parse-ts
cargo publish -p crap-metric
cargo publish -p mutation-test
cargo publish -p debt-scan
cargo publish -p doc-coverage
cargo publish -p duplication
cargo publish -p coupling
cargo publish -p risk-map
cargo publish -p fuzz-surface
cargo publish -p prop-cov
cargo publish -p taint-scan

# 3. Binaries (depend on the above)
cargo publish -p codemetrics-cli

# 4. Optional server component (UTCP bridge)
cargo publish -p codemetrics-server
```

Each `cargo publish` will build, verify, and upload the crate. Wait for each to complete before the next.

**Tip:** You can script this:
```bash
for crate in codemetrics-common ast-parse-ts crap-metric mutation-test debt-scan doc-coverage duplication coupling risk-map fuzz-surface prop-cov taint-scan codemetrics-cli codemetrics-server; do
    echo "=== Publishing $crate ==="
    cargo publish -p "$crate"
done
```

---

### 3. Post-Publish

- Verify crates appear on https://crates.io with correct names
- Check `cargo search codemetrics` returns all published crates
- Users can now install: `cargo install codemetrics-cli`

---

## Troubleshooting

| Issue | Fix |
|-------|-----|
| `failed to select a version for ...` | Ensure you published dependencies first; check `cargo publish -p codemetrics-common` succeeded |
| `missing metadata: repository` | Verify each crate has `repository = { workspace = true }` (done in v1.0.0) |
| `configuration 'debug' is not published` | Ensure you're in release mode: `cargo publish --dry-run` will validate without uploading |
| `API token not authorized` | Regenerate token with `crates:write` scope |

---

## Notes

- Version is unified across workspace at `1.0.0` (set in root `Cargo.toml`)
- License: `Apache-2.0 OR OPL-1.1` (dual)
- Git tag `v1.0.0` is already pushed — crates.io will link to it automatically

---

## After Publishing

Update the README with:
```bash
cargo install codemetrics-cli
```
instead of "when published". You may also add install statistics badge if desired.
