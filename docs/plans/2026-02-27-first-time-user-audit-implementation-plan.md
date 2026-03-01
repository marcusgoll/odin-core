# First-Time User Audit Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Clone odin-core into an isolated directory and walk every documented path (cargo, Docker, TUI, verification scripts) as a first-time user, recording all failures and friction points, then fix them in the real repo.

**Architecture:** Sequential walk-through in `/tmp/odin-audit/odin-core/`. Each phase is self-contained. The audit report accumulates findings in `/tmp/odin-audit/findings.md`. After all phases complete, the report is finalized and fixes are applied to `/home/orchestrator/odin-core`.

**Tech Stack:** Rust (cargo), Docker, Python3 (rich), Bash

---

### Task 1: Clone and Orient

**Files:**
- Create: `/tmp/odin-audit/findings.md` (audit log)
- Read: `README.md` in the fresh clone

**Step 1: Create isolated audit directory and clone**

```bash
rm -rf /tmp/odin-audit
mkdir -p /tmp/odin-audit
cd /tmp/odin-audit
git clone https://github.com/marcusgoll/odin-core.git
cd odin-core
```

**Step 2: Verify clone is clean**

```bash
cd /tmp/odin-audit/odin-core
git status
```

Expected: clean working tree, on `main` branch.

**Step 3: Initialize the findings log**

Create `/tmp/odin-audit/findings.md` with this header:

```markdown
# Odin-Core First-Time User Audit — Findings

**Date:** 2026-02-27
**Clone source:** https://github.com/marcusgoll/odin-core.git
**Commit:** (fill in from git log)

| # | Phase | Severity | Finding | Detail |
|---|-------|----------|---------|--------|
```

**Step 4: Read the README top-to-bottom and record first impressions**

Read `README.md` in the fresh clone. Record findings for:
- Are prerequisites listed? (Rust version, Docker version, Python version)
- Is the quickstart clear enough for someone who has never seen the project?
- Are there any broken links or references to files that don't exist?
- Does the README explain what the project IS before jumping to setup?

Add any findings as rows to the findings table. Severity guide:
- BLOCKER: prevents proceeding
- FRICTION: works but confusing or undocumented
- COSMETIC: minor wording or formatting issues

**Step 5: Commit findings so far**

No commit needed — findings.md is in `/tmp/odin-audit/`, not the repo.

---

### Task 2: Environment Setup

**Files:**
- Read: `/tmp/odin-audit/odin-core/.env.example`
- Read: `/tmp/odin-audit/odin-core/config/default.yaml`

**Step 1: Copy .env.example to .env**

```bash
cd /tmp/odin-audit/odin-core
cp .env.example .env
cat .env
```

Expected: Should contain `ODIN_PROFILE=dev` and `RUST_LOG=info`.

**Step 2: Check if .env needs edits for first-time use**

Verify:
- Do the default values work without modification?
- Is there any documentation about what each variable does?
- Are there any missing variables that the code actually reads?

Search the codebase for env var reads:

```bash
cd /tmp/odin-audit/odin-core
grep -r 'std::env::var\|env::var\|env!\|dotenv\|dotenvy' --include="*.rs" .
grep -r 'os.environ\|os.getenv\|dotenv' --include="*.py" .
```

Record any env vars the code reads that are NOT documented in `.env.example`.

**Step 3: Check config/default.yaml works as-is**

```bash
cat config/default.yaml
```

Note: `plugins.dir: /var/odin/plugins` — does this directory need to exist? What happens if it doesn't? Record finding if this is a problem for a fresh clone.

**Step 4: Record findings**

Append any issues found to `/tmp/odin-audit/findings.md`.

---

### Task 3: Cargo Build

**Files:**
- Read: `/tmp/odin-audit/odin-core/Cargo.toml`

**Step 1: Check Rust toolchain is available**

```bash
rustc --version
cargo --version
```

Record versions in findings for reference.

**Step 2: Build the workspace**

```bash
cd /tmp/odin-audit/odin-core
cargo build 2>&1 | tee /tmp/odin-audit/cargo-build.log
echo "Exit code: $?"
```

Expected: exit code 0, no errors. Warnings are OK but should be noted.

If build fails, record the exact error as a BLOCKER finding.

**Step 3: Record findings**

Append build results to `/tmp/odin-audit/findings.md`. Note:
- Did it build successfully?
- Were there any warnings?
- How long did the build take? (not a finding, just context)

---

### Task 4: Cargo Test

**Step 1: Run the full test suite**

```bash
cd /tmp/odin-audit/odin-core
cargo test --workspace 2>&1 | tee /tmp/odin-audit/cargo-test.log
echo "Exit code: $?"
```

Expected: all 26 tests pass, exit code 0.

**Step 2: Check for test failures**

If any tests fail, record each failure as a BLOCKER finding with:
- Test name
- Crate it belongs to
- Error message
- Whether it's a real bug or a test environment issue

**Step 3: Record findings**

Append test results to `/tmp/odin-audit/findings.md`.

---

### Task 5: Cargo Run (CLI Bootstrap)

**Step 1: Run the documented quickstart command**

```bash
cd /tmp/odin-audit/odin-core
cargo run -p odin-cli -- --config config/default.yaml --run-once 2>&1 | tee /tmp/odin-audit/cargo-run.log
echo "Exit code: $?"
```

Expected: CLI starts, prints bootstrap output, exits cleanly (exit code 0).

**Step 2: Verify the output makes sense**

Check that the output includes:
- "odin-cli starting with config:"
- "bootstrap outcome:" with JSON output
- Clean exit (no panics, no unhandled errors)

**Step 3: Test the local dev command from README**

The README says:
```
cargo run -p odin-cli -- --config config/default.yaml
```

This runs WITHOUT `--run-once`, meaning it will enter the infinite loop. Verify the README mentions this behavior or if a new user would be confused. The README doesn't document `--run-once` for local dev — record if this is confusing.

Run it with a timeout to verify:

```bash
cd /tmp/odin-audit/odin-core
timeout 10 cargo run -p odin-cli -- --config config/default.yaml 2>&1 || true
```

Expected: starts, prints bootstrap, then hangs until timeout. A new user following the README would be confused by this behavior.

**Step 4: Record findings**

Append to `/tmp/odin-audit/findings.md`.

---

### Task 6: Docker Build

**Step 1: Build the Docker image**

```bash
cd /tmp/odin-audit/odin-core
docker build -t odin-core-audit . 2>&1 | tee /tmp/odin-audit/docker-build.log
echo "Exit code: $?"
```

Expected: multi-stage build completes, exit code 0.

If it fails, record as BLOCKER.

**Step 2: Record build time and image size**

```bash
docker images odin-core-audit
```

Not a finding — just context for the report.

**Step 3: Record findings**

Append to `/tmp/odin-audit/findings.md`.

---

### Task 7: Docker Compose Up

**Step 1: Create required volume directories**

The `docker-compose.yml` mounts `./state`, `./plugins`, `./policy`, `./logs`. Check if these exist:

```bash
cd /tmp/odin-audit/odin-core
ls -la state/ plugins/ policy/ logs/ 2>&1
```

If they don't exist, Docker Compose will create them as root-owned directories. Record whether the README mentions this.

**Step 2: Run docker compose up**

```bash
cd /tmp/odin-audit/odin-core
docker compose up -d 2>&1 | tee /tmp/odin-audit/docker-compose-up.log
echo "Exit code: $?"
```

Expected: container starts.

**Step 3: Check container health**

```bash
docker compose ps
docker compose logs --tail=20
```

Expected: container running (or exited if `--run-once` is not in compose command — check the compose file's `command` field).

Note: The compose file runs `["--config", "config/default.yaml"]` WITHOUT `--run-once`, so the container should stay running in the infinite `loop { sleep 60 }`. Verify this.

**Step 4: Check if the container user can access volumes**

The Dockerfile uses `USER odin` (uid 10001). The volumes mounted from the host may have permissions issues. Check:

```bash
docker compose exec odin-core ls -la /var/odin/ 2>&1 || true
docker compose logs 2>&1 | grep -i "permission\|denied\|error" || echo "no permission errors"
```

**Step 5: Shut down**

```bash
cd /tmp/odin-audit/odin-core
docker compose down
```

**Step 6: Record findings**

Append to `/tmp/odin-audit/findings.md`. Pay attention to:
- Did the container start?
- Any permission issues with mounted volumes?
- Is the behavior clear from the README? (i.e., does a new user know the container just sits idle?)

---

### Task 8: TUI Setup and Test

**Step 1: Install TUI dependency**

```bash
python3 -m pip install rich 2>&1 | tee /tmp/odin-audit/tui-install.log
```

Expected: installs cleanly.

**Step 2: Run the TUI in JSON mode (non-interactive)**

```bash
cd /tmp/odin-audit/odin-core
python3 scripts/odin/odin-tui.py --json 2>&1 | tee /tmp/odin-audit/tui-json.log
echo "Exit code: $?"
```

Expected: JSON output with keys `inbox`, `kanban`, `agents`, `logs`, `github`.

Note: The TUI script uses `from tui_core.app import main` which is a relative import. This only works if Python's working directory is `scripts/odin/` OR if the path is configured. A new user running from the repo root will likely get an import error. Check this carefully.

**Step 3: Test the TUI from repo root (as README suggests)**

The README says:
```
python3 scripts/odin/odin-tui.py --live
```

This runs from the repo root, NOT from `scripts/odin/`. The `odin-tui.py` does `from tui_core.app import main`. This import will ONLY work if `scripts/odin/` is in `sys.path`. Check if this is the case:

```bash
cd /tmp/odin-audit/odin-core
python3 scripts/odin/odin-tui.py --json 2>&1
echo "Exit code: $?"
```

If this fails with `ModuleNotFoundError: No module named 'tui_core'`, that's a BLOCKER.

**Step 4: Test the wrapper script**

```bash
cd /tmp/odin-audit/odin-core
bash scripts/odin/odin-tui --json 2>&1
echo "Exit code: $?"
```

The wrapper script resolves the script path and runs it. Check if it handles the PYTHONPATH issue.

**Step 5: Test TUI with one-shot (non-live) mode**

```bash
cd /tmp/odin-audit/odin-core
python3 scripts/odin/odin-tui.py --profile core 2>&1
echo "Exit code: $?"
```

Expected: renders a one-shot snapshot to terminal.

**Step 6: Record findings**

Append to `/tmp/odin-audit/findings.md`. Key questions:
- Does the TUI work from the repo root as the README suggests?
- Are import errors handled gracefully?
- Does the TUI gracefully handle missing data (no /var/odin directory)?

---

### Task 9: Verification Scripts

**Step 1: Run quickstart-smoke.sh**

```bash
cd /tmp/odin-audit/odin-core
bash scripts/verify/quickstart-smoke.sh 2>&1 | tee /tmp/odin-audit/quickstart-smoke.log
echo "Exit code: $?"
```

Expected: All checks pass, exits 0.

**Step 2: Run plugin-install-matrix.sh**

```bash
cd /tmp/odin-audit/odin-core
bash scripts/verify/plugin-install-matrix.sh 2>&1 | tee /tmp/odin-audit/plugin-install-matrix.log
echo "Exit code: $?"
```

Expected: All plugin install path tests pass.

**Step 3: Run workflow-contract.sh**

```bash
cd /tmp/odin-audit/odin-core
bash scripts/verify/workflow-contract.sh 2>&1 | tee /tmp/odin-audit/workflow-contract.log
echo "Exit code: $?"
```

Expected: All contract checks pass. Note: this script uses `rg` (ripgrep). If ripgrep is not installed, it will fail. Is this a documented prerequisite?

**Step 4: Run tui-core-smoke.sh**

```bash
cd /tmp/odin-audit/odin-core
bash scripts/verify/tui-core-smoke.sh 2>&1 | tee /tmp/odin-audit/tui-core-smoke.log
echo "Exit code: $?"
```

Expected: TUI smoke tests pass.

Note: This script runs `python3 -m unittest scripts.odin.tui_core.tests.test_readability` which requires running from the repo root AND the module path to be importable. Check for import issues.

**Step 5: Record findings**

Append to `/tmp/odin-audit/findings.md`. Key questions:
- Do all verification scripts pass in a fresh clone?
- Are all tool prerequisites documented (e.g., `rg`, `jq`, `python3`)?
- Are errors clear when prerequisites are missing?

---

### Task 10: Docs Cross-Check

**Step 1: Walk docs/quickstart.md**

Read `/tmp/odin-audit/odin-core/docs/quickstart.md` and verify:
- Do the listed prerequisites match what's actually needed?
- Does step 1 (`cargo run -p odin-cli -- --config config/default.yaml --run-once`) work? (Already tested in Task 5)
- Does step 2 (`python3 scripts/odin/odin-tui.py --json`) work? (Already tested in Task 8)
- Does step 3 (`bash scripts/verify/quickstart-smoke.sh`) work? (Already tested in Task 9)
- Does step 4 (`bash scripts/verify/workflow-contract.sh`) work? (Already tested in Task 9)

**Step 2: Check docs/quickstart.md against README for contradictions**

Compare the two documents:
- README quickstart: `cp .env.example .env && docker compose up -d`
- docs/quickstart.md: `cargo run -p odin-cli -- --config config/default.yaml --run-once`

These are different entry paths. Is this clear to a new user, or confusing?

**Step 3: Spot-check docs/foundation-spec.md**

Read the architecture section and verify it still matches the actual crate structure. Flag any outdated claims.

**Step 4: Record findings**

Append to `/tmp/odin-audit/findings.md`.

---

### Task 11: Compile Final Audit Report

**Step 1: Finalize /tmp/odin-audit/findings.md**

Add a summary section at the bottom:

```markdown
## Summary

- **Total findings:** N
- **BLOCKERs:** N
- **FRICTION:** N
- **COSMETIC:** N

## Recommendations (Priority Order)

1. [List BLOCKERs first with fix descriptions]
2. [Then FRICTIONs]
3. [Then COSMETICs]
```

**Step 2: Copy report to the real repo**

```bash
cp /tmp/odin-audit/findings.md /home/orchestrator/odin-core/docs/plans/2026-02-27-first-time-user-audit-report.md
```

**Step 3: Commit the report**

```bash
cd /home/orchestrator/odin-core
git add docs/plans/2026-02-27-first-time-user-audit-report.md
git commit -m "docs(audit): add first-time user audit report"
```

---

### Task 12: Fix BLOCKERs

**Files:** Depends on findings from Tasks 1-10.

This task is dynamic — the exact fixes depend on what the audit discovered. For each BLOCKER finding:

**Step 1: Identify the fix**

Read the finding, identify the root cause, and determine the minimal fix.

**Step 2: Apply the fix**

Edit the relevant file(s) in `/home/orchestrator/odin-core`.

**Step 3: Verify the fix**

Re-run the failing step from the audit in `/tmp/odin-audit/odin-core` (after pulling the fix or applying it there too) to confirm it resolves the issue.

**Step 4: Commit each fix individually**

```bash
cd /home/orchestrator/odin-core
git add <changed files>
git commit -m "fix: <description of what was fixed>"
```

---

### Task 13: Fix FRICTION Items

Same pattern as Task 12 but for FRICTION-severity findings. These are typically:
- Missing documentation of prerequisites
- Unclear README instructions
- Undocumented environment variables
- Confusing default behavior (e.g., infinite loop without `--run-once`)

**Step 1-4:** Same as Task 12 pattern — identify, fix, verify, commit individually.

---

### Task 14: Cleanup

**Step 1: Remove the audit directory**

```bash
rm -rf /tmp/odin-audit
```

**Step 2: Remove the audit Docker image**

```bash
docker rmi odin-core-audit 2>/dev/null || true
```

**Step 3: Final status**

Report the total number of findings found and fixed.
