# Odin-Core First-Time User Audit Report

**Date:** 2026-02-27
**Auditor:** Claude Opus 4.6 (automated)
**Clone source:** https://github.com/marcusgoll/odin-core.git
**Commit:** 6c6da87
**Environment:** rustc 1.93.1, cargo 1.93.1, Python 3.12, Docker 27.x

## Executive Summary

The odin-core project **builds, tests, and runs successfully** from a fresh clone. All 35 tests pass, the Docker image builds (116MB), and all 4 verification scripts pass. The core engineering is solid.

However, a first-time user would encounter **significant documentation friction**: no prerequisites listed, no project description, confusing dual quickstart paths, undocumented env vars, and Docker permission issues. None of these are blockers today, but they would frustrate anyone discovering the repo on GitHub.

## Scorecard

| Category | Result |
|----------|--------|
| `cargo build` | PASS (zero warnings) |
| `cargo test --workspace` | PASS (35/35) |
| `cargo run -p odin-cli -- --run-once` | PASS |
| `docker build` | PASS (116MB image) |
| `docker compose up -d` | PASS (container runs) |
| TUI `--json` | PASS |
| TUI `--live` | PASS |
| `quickstart-smoke.sh` | PASS |
| `plugin-install-matrix.sh` | PASS |
| `workflow-contract.sh` | PASS |
| `tui-core-smoke.sh` | PASS |

## Findings Summary

| Severity | Count |
|----------|-------|
| BLOCKER | 0 |
| FRICTION | 19 |
| COSMETIC | 14 |
| OK (confirmations) | 17 |
| **Total** | **50** |

## FRICTION Findings (Priority Order)

| # | Phase | Finding | Fix |
|---|-------|---------|-----|
| 1 | README | No project description before setup | Add 2-3 sentence "What is Odin?" section |
| 2 | README | No prerequisites listed | Add Prerequisites section: Rust stable, Docker, Python 3.12+ |
| 54 | Docs | README quickstart vs docs/quickstart.md are different paths with no cross-reference | Add cross-references and clarify audiences |
| 39 | Docker | No `.dockerignore` file (488MB build context) | Add `.dockerignore` excluding `target/`, `.git/`, `state/`, `logs/` |
| 40 | Docker | Mounted volumes non-writable by container user (uid 10001) | Add `RUN mkdir -p /var/odin && chown odin:odin /var/odin` to Dockerfile |
| 7 | Env | `ODIN_DIR` env var not documented | Add to `.env.example` with comment |
| 8 | Env | `ODIN_GUARDRAILS_ACK` env var not documented | Add to `.env.example` with comment |
| 9 | Env | `ODIN_TUI_PROFILE` env var not documented | Add to `.env.example` with comment |
| 10 | Config | `plugins.dir: /var/odin/plugins` requires privileged directory | Add `config/dev.yaml` override or document in config |
| 3 | README | Docker quickstart creates root-owned host directories | Document in README or add `.gitkeep` files |
| 15 | Docker | `state/` and `logs/` missing from repo | Add `.gitkeep` files |
| 41 | Docker | `state/` and `logs/` auto-created as root-owned on host | Same fix as #15 |
| 5 | README | Bootstrap wrapper not explained | Add brief explanation of `scripts/odin/odin` |
| 32 | TUI | No Python dependency declaration for TUI | Add `scripts/odin/requirements.txt` |
| 48 | Verify | Signature verification tests silently skip | Tests should mark themselves as `#[ignore]` or CI should install tools |
| 53 | Docs | `docs/quickstart.md` prerequisites incomplete | Update prerequisites list |
| 57 | README | Verification Gates section lists only 3 of 10 scripts | List all verification scripts |
| 58 | CI | CI does not run most verification scripts | Add verification scripts to CI |

## COSMETIC Findings

| # | Phase | Finding |
|---|-------|---------|
| 4 | README | Two guardrails example files with inconsistent naming |
| 6 | README | Verification gates section omits `workflow-contract.sh` |
| 11 | Config | `config/default.yaml` `plugins.dir` unused by CLI |
| 16 | README | Local dev section has no `--run-once` hint |
| 18 | Build | Deprecated `serde_yaml` dependency |
| 22 | Build | No `rust-version` MSRV in `Cargo.toml` |
| 35 | TUI | `datetime.utcnow()` deprecated in Python 3.12+ |
| 38 | Docker | Dockerfile `as` keyword casing (`as` → `AS`) |
| 42 | Docker | Overlapping volume mounts create shadow directories |
| 45 | Docker | Dockerfile runtime stage missing `ca-certificates` |
| 50 | Verify | `workflow-contract.sh` does NOT use `rg` (initial concern unfounded) |
| 52 | Verify | `tui-core-smoke.sh` unittest works without `__init__.py` (fragile) |
| 56 | Docs | Foundation spec does not mention `odin-cli` binary |
| 59 | CI | CI `contract-validation` uses `jq` without installing it |
| 60 | Verify | `quickstart-smoke.sh` output order is confusing |

## Recommendations

### Immediate (this session)
1. Add `.dockerignore` (finding #39)
2. Fix Dockerfile: `AS` casing, create `/var/odin` with correct ownership, add `ca-certificates` (findings #38, #40, #45)
3. Add `.gitkeep` to `state/` and `logs/` (finding #15)
4. Expand `.env.example` with all env vars (findings #7, #8, #9)
5. Improve README: add description, prerequisites, `--run-once` hint (findings #1, #2, #16)
6. Add `scripts/odin/requirements.txt` (finding #32)
7. Fix `datetime.utcnow()` deprecation (finding #35)

### Follow-up (separate PRs)
8. Reconcile README quickstart with `docs/quickstart.md` (finding #54)
9. Add verification scripts to CI (finding #58)
10. Address `serde_yaml` deprecation (finding #18)
