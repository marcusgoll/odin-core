# Odin Bootstrap + Operate Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deliver a first-run Odin UX that is CLI-first and verified, with explicit guardrails and confidence-based mode gating.

**Architecture:** Implement in phases. Phase 1 adds wrapper command UX + executable docs and verification scripts. Phase 2 adds persisted guardrails + confidence/mode logic. Phase 3 moves command contract into native Rust subcommands while preserving wrapper compatibility. Keep CLI-only gateway always available and optional adapters non-blocking.

**Tech Stack:** Rust (`odin-cli`), Bash wrapper/scripts, Python TUI (`rich`), Markdown tutorials, existing verify scripts.

---

### Task 1: Add minimal command contract via wrapper (`odin`)

**Files:**
- Create: `scripts/odin/odin`
- Create: `scripts/odin/lib/bootstrap.sh`
- Modify: `README.md`
- Test: `scripts/verify/bootstrap-wrapper-smoke.sh`

**Step 1: Write the failing smoke test for command surface**

Create `scripts/verify/bootstrap-wrapper-smoke.sh` with checks that fail until commands exist:
- `scripts/odin/odin help`
- `scripts/odin/odin connect claude oauth --dry-run`
- `scripts/odin/odin start --dry-run`
- `scripts/odin/odin tui --dry-run`
- `scripts/odin/odin inbox add "test task" --dry-run`
- `scripts/odin/odin verify --dry-run`

Expected initially: command/file missing.

**Step 2: Run smoke test to verify RED**

Run: `bash scripts/verify/bootstrap-wrapper-smoke.sh`  
Expected: non-zero with missing command failure.

**Step 3: Implement minimal wrapper + dispatch library**

Implement command parser in `scripts/odin/odin` and basic handlers in `scripts/odin/lib/bootstrap.sh`:
- `connect`, `start`, `tui`, `inbox add`, `inbox list`, `gateway add`, `verify`
- support `--dry-run` path for smoke tests
- keep behavior conservative if guardrails missing

**Step 4: Re-run smoke test to verify GREEN**

Run: `bash scripts/verify/bootstrap-wrapper-smoke.sh`  
Expected: pass.

**Step 5: Commit**

```bash
git add scripts/odin/odin scripts/odin/lib/bootstrap.sh scripts/verify/bootstrap-wrapper-smoke.sh README.md
git commit -m "feat(cli): add bootstrap wrapper command contract"
```

### Task 2: Add guardrails config and mandatory acknowledgement gate

**Files:**
- Create: `config/guardrails.yaml.example`
- Modify: `scripts/odin/lib/bootstrap.sh`
- Test: `scripts/verify/guardrails-gate-smoke.sh`

**Step 1: Write failing guardrail gate test**

Create `scripts/verify/guardrails-gate-smoke.sh`:
- case A: missing guardrails + risky action => blocked
- case B: guardrails present but unacknowledged => blocked
- case C: guardrails present + acknowledged => allowed

**Step 2: Run guardrail smoke test (RED)**

Run: `bash scripts/verify/guardrails-gate-smoke.sh`  
Expected: failure before gate is implemented.

**Step 3: Implement guardrails loader + gate checks**

In `bootstrap.sh`:
- load guardrails from default path or `--guardrails` flag
- require explicit ack marker before enabling non-readonly actions
- enforce denylist and confirm-required action classes

**Step 4: Run guardrail smoke test (GREEN)**

Run: `bash scripts/verify/guardrails-gate-smoke.sh`  
Expected: pass all scenarios.

**Step 5: Commit**

```bash
git add config/guardrails.yaml.example scripts/odin/lib/bootstrap.sh scripts/verify/guardrails-gate-smoke.sh
git commit -m "feat(guardrails): require acknowledgement before execution"
```

### Task 3: Implement confidence + mode state machine (BOOTSTRAP/OPERATE/RECOVERY)

**Files:**
- Create: `scripts/odin/lib/mode_state.sh`
- Modify: `scripts/odin/lib/bootstrap.sh`
- Test: `scripts/verify/mode-confidence-smoke.sh`

**Step 1: Write failing confidence transition test**

Create `scripts/verify/mode-confidence-smoke.sh` with state transitions:
- initial confidence `10`, mode `BOOTSTRAP`
- apply verified events and assert point increases
- assert `OPERATE` blocked until confidence >= 60 + guardrails + one task cycle

**Step 2: Run confidence test (RED)**

Run: `bash scripts/verify/mode-confidence-smoke.sh`  
Expected: non-zero before mode state engine exists.

**Step 3: Implement mode state persistence and transitions**

Implement:
- state file (e.g., `/var/odin/bootstrap-state.json`)
- score updates only on verified checkpoints
- gate evaluation for `OPERATE`
- fallback to `RECOVERY` on failed verify checks

**Step 4: Run confidence test (GREEN)**

Run: `bash scripts/verify/mode-confidence-smoke.sh`  
Expected: pass.

**Step 5: Commit**

```bash
git add scripts/odin/lib/mode_state.sh scripts/odin/lib/bootstrap.sh scripts/verify/mode-confidence-smoke.sh
git commit -m "feat(mode): add confidence-gated bootstrap/operate transitions"
```

### Task 4: Add executable quickstart and integration docs

**Files:**
- Create: `docs/quickstart.md`
- Create: `docs/integrations/n8n.md`
- Create: `docs/integrations/slack.md`
- Create: `docs/integrations/telegram.md`
- Modify: `README.md`
- Test: `scripts/verify/docs-command-smoke.sh`

**Step 1: Write failing docs command smoke test**

Create `scripts/verify/docs-command-smoke.sh` that extracts listed commands and executes `--dry-run`/safe checks.

**Step 2: Run docs smoke (RED)**

Run: `bash scripts/verify/docs-command-smoke.sh`  
Expected: failure before docs/commands exist.

**Step 3: Write docs with minimal copy-paste flow**

Each doc must include:
- prerequisites
- minimal config
- copy-paste commands
- verification checks
- common failure and smallest fix

**Step 4: Run docs smoke (GREEN)**

Run: `bash scripts/verify/docs-command-smoke.sh`  
Expected: pass.

**Step 5: Commit**

```bash
git add docs/quickstart.md docs/integrations README.md scripts/verify/docs-command-smoke.sh
git commit -m "docs(bootstrap): add executable quickstart and integration tutorials"
```

### Task 5: Move contract into native Rust CLI subcommands (parity layer)

**Files:**
- Modify: `bin/odin-cli/src/main.rs`
- Modify: `bin/odin-cli/Cargo.toml`
- Test: `bin/odin-cli/tests/cli_contract.rs`
- Modify: `scripts/odin/odin`

**Step 1: Write failing Rust CLI contract tests**

Add `bin/odin-cli/tests/cli_contract.rs` (using `assert_cmd`) for:
- `odin-cli connect ... --dry-run`
- `odin-cli start --dry-run`
- `odin-cli tui --dry-run`
- `odin-cli inbox add ... --dry-run`
- `odin-cli verify --dry-run`

**Step 2: Run contract tests (RED)**

Run: `cargo test -p odin-cli --test cli_contract`  
Expected: fail because subcommands do not exist.

**Step 3: Implement `clap`-based subcommands**

In `main.rs`:
- replace flag-only parser with subcommand parser
- preserve old flags behind compatibility branch where needed
- keep wrapper `scripts/odin/odin` delegating to native CLI if present

**Step 4: Run contract tests (GREEN)**

Run: `cargo test -p odin-cli --test cli_contract`  
Expected: pass.

**Step 5: Commit**

```bash
git add bin/odin-cli/src/main.rs bin/odin-cli/Cargo.toml bin/odin-cli/tests/cli_contract.rs scripts/odin/odin
git commit -m "feat(odin-cli): add native bootstrap command subcommands"
```

### Task 6: Final verification matrix and release-ready check

**Files:**
- Modify: `scripts/verify/quickstart-smoke.sh`
- Modify: `docs/release-readiness.md`

**Step 1: Add bootstrap verification to existing matrix**

Update quickstart smoke to verify:
- connect flow dry-run
- start/tui/inbox/verify contract
- first inbox item normalization fields present

**Step 2: Run full verification**

Run:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `bash scripts/verify/quickstart-smoke.sh`
- `bash scripts/verify/tui-core-smoke.sh`
- `bash scripts/verify/plugin-install-matrix.sh`

Expected: all pass.

**Step 3: Commit**

```bash
git add scripts/verify/quickstart-smoke.sh docs/release-readiness.md
git commit -m "chore(verify): enforce bootstrap command-path verification"
```

