# Changelog

## Unreleased

### Removed
- **`ast-parse` crate** — fully migrated to `ast-parse-ts` (tree-sitter) for universal multi-language support. All references to `ast-parse` and `syn` have been removed from the workspace.

### Added
- **Multi-language fixture tests** — integration tests using `assert_cmd` + `predicates` for:
  - `doc-coverage`
  - `duplication`
  - `coupling`
  - `taint-scan`
  - `prop-cov`
- **Multi-language fixture files** in `crates/fixtures/` (Python, JavaScript, TypeScript, Go, C, Java, Rust) to validate cross-language parsing.

### Changed
- **Parser pool** — `ast-parse-ts` already maintains a `thread_local!` parser cache per language, avoiding expensive `Parser::new()` + `set_language()` calls on every file.
- **Bounded `rayon` parallelism** — heavy tools now use a custom `rayon::ThreadPool` capped at 2 threads:
  - `duplication` (`dupfind`)
  - `taint-scan` (`taint`)
  - `coupling` (`coupling`)
- **Batch runner throttling** — `quality-cli` already caps concurrent tool execution at 4 threads to respect CI RAM limits.
- **README** updated with complete crate list, expanded language matrix (C/C++, C#, Java, PHP), and Performance section.

### Fixed
- **Doc-coverage Rust detection** — `has_doc_comment_before` now skips `#[derive(...)]` and other `#[...]` attribute lines when searching for `///` doc comments, fixing false negatives on all Rust structs and enums with derive macros.
- **Missing doc comments** — added `///` doc comments to `coverage_pct`, `Column::left`, `Column::right`, SARIF structs, and `SarifLog::add_run` across `ast-parse-ts` and `quality-common`. Both crates now report 100% public API documentation coverage.
- **Coupling false positives** — `coupling` now filters out external crate imports (e.g., `clap::Parser`, `serde::Serialize`, `assert_cmd::Command`) and wildcard imports (`super::*`, `predicates::prelude::*`), reporting only workspace-local module dependencies.
- **CRAP category alignment** — `codemetrics_common::crap_category` thresholds restored to `excellent/good/acceptable/crappy` to match `crap-metric` output expectations.
- **Integration test drift** — updated `crap-metric` and `coupling` integration tests to match post-migration output formats and messages.
- **CRAP category column width** — increased `CATEGORY` column from 12 to 15 bytes so the `△ acceptable` icon+label (14 bytes) is no longer truncated to `… acceptable`.
- **Duplicate code reduction** — refactored `ast-parse-ts` to use a new `parse_with_tree` helper, eliminating 8 repetitions of the `with_pooled_parser` + `match parser.parse` boilerplate.
- **Memory exhaustion fix** — added memory monitoring with auto-terminate to prevent OOM crashes on 16GB/32GB systems:
  - `codemetrics-cli run_batch` now runs tools sequentially (was concurrent with MAX_CONCURRENT=4)
  - Rayon ThreadPoolBuilder reduced from 2 threads to 1 in coupling, duplication, taint-scan
  - MemoryMonitor module reads `/proc/self/status` and `/proc/meminfo` to track RSS
  - Auto-terminates with exit code 137 when memory exceeds 80% of system RAM (configurable via QUALITY_MAX_MEMORY_MB, QUALITY_WARN_THRESHOLD, QUALITY_AUTO_TERMINATE)
- **Batch runner expansion** — added taint-scan and fuzz-surface to codemetrics-cli run_batch (now 9 tools total)
- **mutation-test exclusion** — removed mutation-test from batch runner because it spawns cargo test processes that bypass parent memory monitoring, causing system crashes. Run manually with explicit resource limits.
- **mutation-test production rewrite** — complete rewrite to prevent system crashes:
  - **Scratch workspace isolation**: copies entire workspace to `/tmp/mutate-<id>/` (excluding `target/` and `.git/`) so mutations never touch the real source tree
  - **Watchdog timeout**: spawns `cargo test` with `Command::spawn()` + a watchdog thread that kills the process group (`SIGKILL` on `-pid`) after configurable timeout (default 30s); watchdog exits immediately when process finishes normally (100ms poll interval)
  - **Build cache reuse**: passes `--target-dir` pointing to the host workspace's `target/` so each mutant only recompiles the changed file, not the whole workspace
  - **Workspace root detection**: walks up from crate root to find `[workspace]` Cargo.toml for correct relative path resolution in scratch copy
  - **Hard ceiling**: max_mutants capped at 50 (default 5), rejects higher values with a warning
  - **Baseline verification**: runs tests in original crate before copying (reuses full build cache, fails fast)
  - **RAII cleanup**: `ScratchCrate` implements `Drop` to remove scratch dir automatically on exit or panic
  - Removed broken `ulimit -v` bash wrapper from `CargoTestRunner`
  - Re-added to batch runner with `--max-mutants 5 --timeout 30`
- **fuzz-surface panic fix** — fixed integer underflow panic in brace depth calculation by using saturating_sub instead of -=
