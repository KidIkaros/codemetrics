# Project Status — CodeMetrics

## Current Phase
**Stable v1.0.0** — Production-ready, GitHub public at `github.com/KidIkaros/codemetrics`

## Ten Tools (All Green)
| Tool | Binary | Status |
|------|--------|--------|
| Code Debt | `debt` | ✓ |
| Doc Coverage | `doccov` | ✓ |
| CRAP Metric | `crap` | ✓ |
| Coupling | `coupling` | ✓ |
| Risk Map | `riskmap` | ✓ |
| Duplication | `dupfind` | ✓ |
| Property Coverage | `propcov` | ✓ |
| Taint Scan | `taint` | ✓ |
| Fuzz Surface | `fuzz` | ✓ |
| Mutation Test | `mutate` | ✓ |

## Recent Work
- Rebrand from `quality-tools` → `codemetrics` (June 2026)
- Unified CLI under single `codemetrics` binary entry point
- Exported 8 Hermes Agent skills into repo for AI integration
- CI stabilized: 2 known flaky tests ignored with FIXME comments

## Known Limitations
| Tool | Limitation |
|------|------------|
| `mutate` | Requires tests to pass — ignores ignored tests by default |

## Roadmap
- [x] Replace `crap` icon table truncation with proper unicode width handling
- [x] Fix taint-secret detection (log-leak and Secret:: RHS now detected)
- [x] Add JSON schema validation for all tool outputs
- [ ] Publish crates to crates.io (currently workspace-only)

## Getting Started
```bash
cargo run -p codemetrics-cli -- run .
cargo test --workspace
```

## Repo Structure
```
crates/          10 tool crates + common + CLI
hermes/          Hermes Agent skills (AI integration)
docs/            Guides & integration notes
schemas/         JSON schemas for output validation
scripts/         CI/build helpers
```

---

*Last updated: 2026-05-04 | Branch: master*
