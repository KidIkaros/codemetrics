# Project Status — CodeMetrics

## Current Phase
**Stable v1.0.0** — Production-ready, GitHub public at `github.com/KidIkaros/codemetrics`

## 21 Checks Across 3 Categories (All Green)

**Quality (16):** crap · debt · doccov · riskmap · dupfind · coupling · complexity · linelen · halstead · deadcode · cohesion · comments · propcov · typecov · fuzz · mutate

**Security (6):** secrets · taint · errhandle · vulnscan · sast · crypto

**Compliance (2):** licenses · sbom

| Check | Binary | Status |
|-------|--------|--------|
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
| Secrets | `secrets` | ✓ |
| SAST | `sast` | ✓ |
| Crypto Check | `cryptocheck` | ✓ |
| Vuln Scan | `vulnscan` | ✓ |
| Error Handling | `error-handling` | ✓ |
| Licenses | `licenses` | ✓ |
| SBOM | `sbom` | ✓ |
| Dead Code | `dead-code` | ✓ |
| Line Length | `line-length` | ✓ |
| Complexity | `complexity` | ✓ |
| Type Coverage | `type-coverage` | ✓ |

## Recent Work
- Rebrand from `quality-tools` → `codemetrics` (May 2026)
- Unified CLI under single `codemetrics` binary entry point
- Exported Hermes Agent skills into repo for AI integration
- Added `codemetrics init/check/watch/install-hooks/report` high-level commands
- MCP server (`codemetrics-server`) for GUI client compatibility (Claude Desktop, Cursor, Windsurf)
- Expanded from 10 to 21 checks: added sast, crypto, secrets, licenses, sbom, deadcode, linelen, complexity, typecov, comments, cohesion, errhandle, vulnscan, halstead
- HTML audit report with sidebar navigation, health score (A–F), SVG gauge, inline offenders
- Watch mode `--full` flag + cycle diff; `--verbose` flag on check; health score in summary box
- Self-audit clean: 21/21 checks pass (`codemetrics check .`)

## Known Limitations
| Tool | Limitation |
|------|------------|
| `mutate` | Requires tests to pass — ignores ignored tests by default |

## Roadmap
- [x] Replace `crap` icon table truncation with proper unicode width handling
- [x] Fix taint-secret detection (log-leak and Secret:: RHS now detected)
- [x] Add JSON schema validation for all tool outputs
- [x] Fix `load_config_thresholds` — all `.quality.toml` keys now parsed (was only reading 4 of 10)
- [ ] Publish crates to crates.io — guide at `docs/PUBLISHING.md` (pending API token)

## Getting Started
```bash
cargo build --release
codemetrics init                        # detect ecosystem, write .quality.toml
codemetrics check .                     # self-audit (21 checks)
bash scripts/test.sh                    # full test suite
```

## Repo Structure
```
crates/          28 crates: 21 tool engines + common + CLI + server
hermes/          Hermes Agent skills (AI integration)
docs/            Guides & integration notes
schemas/         JSON schemas for output validation
scripts/         CI/build helpers
```

---

*Last updated: 2026-05-05 | Branch: master*
