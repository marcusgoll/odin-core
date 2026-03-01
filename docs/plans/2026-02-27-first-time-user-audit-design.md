# First-Time User Audit — Design

**Date:** 2026-02-27
**Status:** Approved
**Approach:** Sequential walk-through (Approach A)

## Goal

Audit odin-core as a first-time user who discovers the repo on GitHub. Clone into an isolated directory, follow the README and docs step-by-step, and record every failure, friction point, or undocumented assumption.

## Audit Environment

- **Location:** `/tmp/odin-audit/odin-core/` (fresh `git clone`)
- **Source:** `https://github.com/marcusgoll/odin-core.git`
- **Persona:** Developer with Rust, Docker, and Python3 installed. Never seen the codebase. Only guide is README and docs.

## Severity Categories

| Severity | Definition |
|----------|-----------|
| BLOCKER | Cannot proceed without fixing |
| FRICTION | Works but confusing or undocumented |
| COSMETIC | Minor wording or formatting issues |

## Walk-Through Sequence

### Phase 1 — Clone & Orient
1. `git clone` into `/tmp/odin-audit/odin-core`
2. Read README — note first impressions
3. Check if prerequisites are stated (Rust version, Docker version, etc.)

### Phase 2 — Environment Setup
4. `cp .env.example .env`
5. Check if `.env` defaults work without edits

### Phase 3 — Local Build & Test (cargo path)
6. `cargo build`
7. `cargo test --workspace`
8. `cargo run -p odin-cli -- --config config/default.yaml --run-once`

### Phase 4 — Docker Path
9. `docker build -t odin-core-audit .`
10. `docker compose up -d`
11. Check logs, verify running
12. `docker compose down`

### Phase 5 — TUI Path
13. `python3 -m pip install rich`
14. `python3 scripts/odin/odin-tui.py --profile core`

### Phase 6 — Verification Gates
15. `bash scripts/verify/quickstart-smoke.sh`
16. `bash scripts/verify/plugin-install-matrix.sh`
17. `bash scripts/verify/workflow-contract.sh`
18. `bash scripts/verify/tui-core-smoke.sh`

### Phase 7 — Docs Cross-Check
19. Walk `docs/quickstart.md` — compare with README
20. Check `docs/foundation-spec.md` for accuracy

### Phase 8 — Report & Fix
21. Compile audit report
22. Fix all BLOCKERs and FRICTIONs in the real repo
23. Commit fixes

## Deliverables

1. **Audit Report** — findings table with severity ratings, summary statistics
2. **Fixes Applied** — direct commits to `/home/orchestrator/odin-core` fixing all BLOCKERs and FRICTIONs
3. **No new features** — strictly audit and fix scope
