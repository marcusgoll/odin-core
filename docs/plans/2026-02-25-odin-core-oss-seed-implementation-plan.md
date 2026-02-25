# Odin Core OSS Seed (Milestones 1+2+3) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Prepare `odin-core` for first public push with a general TUI, upgrade-safety baseline lock, and install/release hardening evidence.

**Architecture:** Keep runtime behavior unchanged while adding operational scripts/docs around existing core modules. Sequence work into three checkpoint commits (A/B/C) with verification at each gate, then perform one final push. Treat `cfipros` as read/test-only baseline source.

**Tech Stack:** Rust workspace (`cargo`), Bash verification scripts, Python TUI (`rich`), GitHub Actions workflows.

---

### Task 1: Checkpoint A (TUI seed) commit

**Files:**
- Modify: `README.md`
- Create: `scripts/odin/odin-tui.py`
- Create: `scripts/odin/odin-tui`

**Step 1: Verify dashboard launch paths**

Run:
`python3 scripts/odin/odin-tui.py`
`timeout 6 python3 scripts/odin/odin-tui.py --live`
`bash scripts/odin/odin-tui`
`timeout 6 bash scripts/odin/odin-tui --live`

Expected: Snapshot renders, live starts (timeout exit 124 acceptable).

**Step 2: Verify Rust workspace unaffected**

Run: `cargo test --workspace`
Expected: all pass.

**Step 3: Commit checkpoint A**

Run:
`git add README.md scripts/odin`
`git commit -m "feat(tui): add general dashboard to odin-core"`

### Task 2: Checkpoint B (upgrade-safety lock)

**Files:**
- Create: `docs/baselines/cfipros-odin-baseline.md`
- Create: `docs/baselines/compat-regression-matrix.md`
- Create: `scripts/verify/compat-regression.sh`
- Modify: `docs/migration-checklist.md`

**Step 1: Capture baseline hash and contracts**

Run: `git -C /home/orchestrator/cfipros rev-parse --short HEAD`
Expected: hash recorded in baseline doc.

**Step 2: Implement compat regression script**

Script runs keepalive syntax + baseline Odin test matrix against a provided legacy root.

**Step 3: Execute regression script**

Run: `bash scripts/verify/compat-regression.sh --legacy-root /home/orchestrator/cfipros`
Expected: all checks pass, exit 0.

**Step 4: Commit checkpoint B**

Run:
`git add docs/baselines scripts/verify docs/migration-checklist.md`
`git commit -m "docs(compat): add baseline pin and regression lock artifacts"`

### Task 3: Checkpoint C (install/release hardening)

**Files:**
- Create: `scripts/verify/quickstart-smoke.sh`
- Create: `scripts/verify/plugin-install-matrix.sh`
- Create: `docs/release-readiness.md`
- Modify: `README.md`

**Step 1: Add quickstart smoke checks**

Validate compose config and CLI boot path.

**Step 2: Add plugin install matrix checks**

Validate local path, git ref, artifact checksum and signature-required behavior.

**Step 3: Run hardening scripts**

Run:
`bash scripts/verify/quickstart-smoke.sh`
`bash scripts/verify/plugin-install-matrix.sh`
Expected: exit 0.

**Step 4: Commit checkpoint C**

Run:
`git add scripts/verify docs/release-readiness.md README.md`
`git commit -m "chore(release): add quickstart and install verification gates"`

### Task 4: Final verification and single push

**Files:**
- Modify: none (verification + git operations)

**Step 1: Run full verification matrix**

Run:
`cargo fmt --all --check`
`cargo clippy --workspace --all-targets -- -D warnings`
`cargo test --workspace`
`bash scripts/verify/compat-regression.sh --legacy-root /home/orchestrator/cfipros`
`bash scripts/verify/quickstart-smoke.sh`
`bash scripts/verify/plugin-install-matrix.sh`

Expected: all pass.

**Step 2: Configure/push remote**

Run:
`git remote add origin https://github.com/marcusgoll/odin-core.git` (or update existing)
`git push -u origin main`

Expected: one successful push.

**Step 3: Validate remote state**

Run:
`git ls-remote --heads origin main`
Expected: matching head exists remotely.
