# Changelog

All notable changes to CodeMetrics are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added — Security & Compliance tools
- `sast` — SAST scanner covering SQL injection, XSS, path traversal, command injection, eval, SSRF, unsafe deserialization (25 rules / 7 categories)
- `crypto-check` — Weak crypto detection: MD5/SHA1, insecure random, hardcoded IVs, ECB mode, deprecated TLS, fast-hash password storage (25+ rules)
- `licenses` — OSS license compliance scanner (Cargo.lock / package.json / requirements.txt); GPL/AGPL deny-list enforcement
- `sbom` — SBOM generator: CycloneDX 1.4 XML and SPDX 2.3 text from lock files
- `vulnscan` — Known CVE audit via `cargo-audit` / `pip-audit`
- `secrets` — Hardcoded credential / API key detection
- `error-handling` — Unhandled error and swallowed exception pattern detection
- `dead-code` — Unused symbol and unreachable branch detection
- `line-length` — Line length violation check
- `complexity` — Cyclomatic complexity violation check
- `type-coverage` — Type annotation coverage (Python/TypeScript)
- `cohesion` — Module cohesion analysis
- `comment-ratio` — Comment density check
- `halstead` — Halstead bug estimate

### Added — CLI commands
- `codemetrics report .` — HTML audit report with sidebar nav, SVG donut gauge, A–F health grade, inline offender drill-downs, executive summary, remediation checklist
- `codemetrics report . --format markdown` — Markdown variant
- `codemetrics report . --from-json check.json` — render from existing JSON snapshot
- `codemetrics report . --open` — auto-launch report in browser after generation
- `codemetrics sbom .` — standalone SBOM generation
- `codemetrics diff old.json new.json` — compare two check snapshots, show regressions/fixes
- `codemetrics check . --only <checks>` — run a specific subset of checks
- `codemetrics check . --ci` — CI shorthand: JSON output + no TTY color/progress
- `codemetrics check . --verbose` — print inline file:line offenders for all checks
- `codemetrics watch . --full` — run all 21 checks every cycle (not just debt/doc/crap)

### Added — UX / Terminal output
- Weighted health score (0–100) and letter grade (A–F) in `╔═╗` summary box
- Inline file:line offenders under each failed check line
- Cycle diff in watch mode: `↑ name now passing` / `↓ name now failing` lines
- `codemetrics init` / `codemetrics init --ci` now print a numbered next-steps block
- Better missing-tool error messages: names the binary and suggests install path

### Changed
- **Rebranded from `quality-tools` to `codemetrics`** — all commands, paths, and references updated
- Unified CLI entry point: `codemetrics <subcommand>` (previously separate binaries)
- Default history directory renamed to `.codemetrics-history/`
- HTML report rebuilt: token-replacement approach avoids Rust 2021 prefixed-literal issues with CSS
- Date arithmetic in report header fixed (was computing wrong month from Unix timestamp)
- `run_watch_checks` now returns results for cycle diff comparison

### Fixed
- CI stabilization: ignored known flaky tests in `ast-parse-ts` and `taint` modules
- ANSI icon width handling in CRAP tool output for consistent test capture
- `load_config_thresholds` now parses all `.quality.toml` keys (was only reading 4 of 23)
- Taint scan: `log-leak` and `Secret::` RHS patterns now detected correctly

---

## [1.0.0] — 2026-05-03

### Added
- Initial public release of CodeMetrics (stable v1)
- Ten analysis engines: `crap`, `mutate`, `debt`, `riskmap`, `doccov`, `taint`, `fuzz`, `coupling`, `dupfind`, `propcov`
- Single-binary CLI (`codemetrics`) with subcommands
- SARIF output support for GitHub Security tab integration
- JSON and NDJSON output formats for machine consumption
- Zero-configuration detection for 15+ programming languages
- Self-hosting: runs on its own codebase with CI validation

### Documentation
- Professional README with problem/solution framing
- User guide (`docs/user-guide.md`) and developer guide (`docs/developer-guide.md`)
- UTCP integration notes (`docs/utcp-integration.md`)
- Project status page (`PROJECT_STATUS.md`) with roadmap and limitations
- SVG logo and social preview assets

### Infrastructure
- GitHub Actions workflow with SARIF upload
- `.editorconfig` and `.pre-commit-config.yaml` for contributor consistency
- `PROJECT_STATUS.md` tracking tool health and known issues
- Hermes Agent skills exported to repo `hermes/` directory

---

## [0.1.0] — Prior to public release (as quality-tools)

### Added (pre-rebrand)
- Separate crate-per-tool architecture with workspace build
- Basic CLI wrappers for each tool
- Proof-of-concept AST-based duplication detection and CRAP metric

---

## Upgrade Guide

See [`UPGRADE.md`](UPGRADE.md) for migration instructions from `quality-tools` to `codemetrics`.
