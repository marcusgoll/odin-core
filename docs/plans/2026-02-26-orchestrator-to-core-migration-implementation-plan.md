# Odin Orchestrator -> Odin Core Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship a staged, reversible migration where `odin-orchestrator` runs via `odin-core` engine, preserving recoverable skills/learnings/state with measurable proof artifacts.

**Architecture:** Keep `odin-core` as stable engine boundary and `odin-orchestrator` as thin wrapper. Implement a canonical user-data model with deterministic export/import tooling, then cut over through `legacy -> shadow -> core` modes with parity gates and tested rollback.

**Tech Stack:** Rust (`odin-cli`, `odin-core-runtime`, new migration crate), Bash bridge scripts in `odin-orchestrator`, JSON schema files, existing compat adapters, shell verification scripts.

---

## Scope and Sequence

- Build migration data tooling first (`export/validate/import`) with proofs.
- Add engine-mode bridge toggles in orchestrator without removing legacy behavior.
- Add golden parity/shadow checks.
- Add rollback automation and end-user docs.
- Gate release on migration rehearsal.

## Major Decision Checkpoints

### Checkpoint A: Canonical user-data model and pack format
- Rationale: portable, versioned, testable migration artifact.
- Risks: under-modeled legacy variants.
- Rollback: no cutover until dry-run + checksum + quarantine thresholds pass.

### Checkpoint B: Subprocess bridge (wrapper -> core binary)
- Rationale: lowest-risk path from Bash orchestrator to Rust engine.
- Risks: contract drift between wrapper and CLI.
- Rollback: flip `ODIN_ENGINE_MODE=legacy`.

### Checkpoint C: Shadow-mode before core authority
- Rationale: prove behavioral parity before control-plane switch.
- Risks: duplicate side effects if shadow isolation is incomplete.
- Rollback: keep legacy authoritative; core shadow disabled.

---

### Task 1: Add Migration CLI Surface and Wiring

**Files:**
- Modify: `bin/odin-cli/src/main.rs`
- Modify: `bin/odin-cli/Cargo.toml`
- Create: `bin/odin-cli/tests/migrate_cli_help.rs`
- Create: `crates/odin-migration/Cargo.toml`
- Create: `crates/odin-migration/src/lib.rs`

**Step 1: Write failing CLI tests for migrate command surface**

Add tests for:
- `odin-cli migrate --help`
- `odin-cli migrate export --help`
- `odin-cli migrate validate --help`
- `odin-cli migrate import --help`

**Step 2: Run test to verify it fails**

Run: `cargo test -p odin-cli migrate_cli_help -- --nocapture`  
Expected: FAIL (subcommands missing).

**Step 3: Add minimal subcommand parsing + crate wiring**

Implement `migrate` command tree and delegate handlers to `odin-migration` crate stubs.

**Step 4: Run test to verify it passes**

Run: `cargo test -p odin-cli migrate_cli_help -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add bin/odin-cli/src/main.rs bin/odin-cli/Cargo.toml bin/odin-cli/tests/migrate_cli_help.rs crates/odin-migration/Cargo.toml crates/odin-migration/src/lib.rs
git commit -m "feat(cli): add migrate command surface and crate wiring"
```

### Task 2: Define User Data Model v1 Contracts

**Files:**
- Create: `schemas/user-data-model.v1.schema.json`
- Create: `schemas/skill-pack.v1.schema.json`
- Create: `schemas/learning-pack.v1.schema.json`
- Create: `crates/odin-migration/src/model.rs`
- Create: `crates/odin-migration/src/validate.rs`
- Create: `crates/odin-migration/tests/model_validation.rs`

**Step 1: Write failing validation tests (good + bad fixtures)**

Add fixture-based tests for required top-level sections and schema version checks.

**Step 2: Run test to verify it fails**

Run: `cargo test -p odin-migration model_validation -- --nocapture`  
Expected: FAIL (validator not implemented).

**Step 3: Implement serde models + schema validation helpers**

Implement strict required fields and version checks for:
- `manifest.json`
- skill pack metadata
- learning pack metadata

**Step 4: Run test to verify it passes**

Run: `cargo test -p odin-migration model_validation -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add schemas/user-data-model.v1.schema.json schemas/skill-pack.v1.schema.json schemas/learning-pack.v1.schema.json crates/odin-migration/src/model.rs crates/odin-migration/src/validate.rs crates/odin-migration/tests/model_validation.rs
git commit -m "feat(migration): define user-data model v1 schemas and validators"
```

### Task 3: Implement Inventory and Proof Snapshot Command

**Files:**
- Create: `crates/odin-migration/src/inventory.rs`
- Modify: `crates/odin-migration/src/lib.rs`
- Modify: `bin/odin-cli/src/main.rs`
- Create: `crates/odin-migration/tests/inventory_snapshot.rs`

**Step 1: Write failing test for deterministic inventory output**

Test should assert stable JSON keys and expected counts for fixture directory.

**Step 2: Run test to verify it fails**

Run: `cargo test -p odin-migration inventory_snapshot -- --nocapture`  
Expected: FAIL.

**Step 3: Implement `migrate inventory` logic**

Collect counts for skills/learnings/checkpoints/events and write JSON snapshot to output path.

**Step 4: Run test to verify it passes**

Run: `cargo test -p odin-migration inventory_snapshot -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/odin-migration/src/inventory.rs crates/odin-migration/src/lib.rs bin/odin-cli/src/main.rs crates/odin-migration/tests/inventory_snapshot.rs
git commit -m "feat(migration): add deterministic inventory snapshot command"
```

### Task 4: Build Exporter (`legacy -> bundle`)

**Files:**
- Create: `crates/odin-migration/src/export.rs`
- Create: `crates/odin-migration/src/checksum.rs`
- Modify: `crates/odin-migration/src/lib.rs`
- Modify: `bin/odin-cli/src/main.rs`
- Create: `crates/odin-migration/tests/export_bundle.rs`

**Step 1: Write failing export tests**

Cover:
- bundle folder structure creation
- manifest emission
- checksum file presence
- deterministic file ordering

**Step 2: Run test to verify it fails**

Run: `cargo test -p odin-migration export_bundle -- --nocapture`  
Expected: FAIL.

**Step 3: Implement exporter with mapping table from design doc**

Include sources:
- repo/project `.claude/skills`
- `/var/odin/memory/*`
- `/var/odin/state|routing|projects|conversations`
- `/var/odin/inbox|outbox|rejected`
- `/var/odin/agents/*/(status|checkpoint|done-*)`

**Step 4: Run test to verify it passes**

Run: `cargo test -p odin-migration export_bundle -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/odin-migration/src/export.rs crates/odin-migration/src/checksum.rs crates/odin-migration/src/lib.rs bin/odin-cli/src/main.rs crates/odin-migration/tests/export_bundle.rs
git commit -m "feat(migration): implement export bundle with manifest and checksums"
```

### Task 5: Build Bundle Validator

**Files:**
- Create: `crates/odin-migration/src/verify.rs`
- Modify: `crates/odin-migration/src/lib.rs`
- Modify: `bin/odin-cli/src/main.rs`
- Create: `crates/odin-migration/tests/verify_bundle.rs`

**Step 1: Write failing checksum-tamper test**

Test mutates exported file and expects validation failure.

**Step 2: Run test to verify it fails**

Run: `cargo test -p odin-migration verify_bundle -- --nocapture`  
Expected: FAIL.

**Step 3: Implement checksum + schema + structural validation**

Return non-zero with actionable error report.

**Step 4: Run test to verify it passes**

Run: `cargo test -p odin-migration verify_bundle -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/odin-migration/src/verify.rs crates/odin-migration/src/lib.rs bin/odin-cli/src/main.rs crates/odin-migration/tests/verify_bundle.rs
git commit -m "feat(migration): add bundle validate command with checksum enforcement"
```

### Task 6: Build Importer (`bundle -> canonical`) with Idempotence

**Files:**
- Create: `crates/odin-migration/src/import.rs`
- Create: `crates/odin-migration/src/lock.rs`
- Create: `crates/odin-migration/src/backup.rs`
- Modify: `crates/odin-migration/src/lib.rs`
- Modify: `bin/odin-cli/src/main.rs`
- Create: `crates/odin-migration/tests/import_idempotent.rs`

**Step 1: Write failing dry-run/apply/idempotence tests**

Required assertions:
- `--dry-run` writes no mutable target files
- `--apply` writes expected files + report
- second `--apply` with same bundle yields zero diff

**Step 2: Run test to verify it fails**

Run: `cargo test -p odin-migration import_idempotent -- --nocapture`  
Expected: FAIL.

**Step 3: Implement importer transaction flow**

- Acquire migration lock
- Create backup snapshot
- Upsert files by stable key/path
- Write `meta/import-report.json`
- Quarantine invalid entries

**Step 4: Run test to verify it passes**

Run: `cargo test -p odin-migration import_idempotent -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/odin-migration/src/import.rs crates/odin-migration/src/lock.rs crates/odin-migration/src/backup.rs crates/odin-migration/src/lib.rs bin/odin-cli/src/main.rs crates/odin-migration/tests/import_idempotent.rs
git commit -m "feat(migration): implement import with lock, backup, quarantine, and idempotence"
```

### Task 7: Add Orchestrator Engine Mode Wrapper (`legacy|shadow|core`)

**Files:**
- Create: `odin-orchestrator/scripts/odin/lib/engine-mode.sh`
- Create: `odin-orchestrator/scripts/odin/lib/engine-bridge.sh`
- Modify: `odin-orchestrator/scripts/odin/odin-service.sh`
- Modify: `odin-orchestrator/scripts/odin/odin-inbox-processor.sh`
- Create: `odin-orchestrator/scripts/odin/tests/engine-mode-test.sh`

**Step 1: Write failing shell tests for mode selection and fallback**

Cover:
- default `legacy`
- explicit `shadow` does not replace legacy side effects
- explicit `core` routes through bridge
- failure in `core` mode triggers configured fallback behavior

**Step 2: Run test to verify it fails**

Run: `bash odin-orchestrator/scripts/odin/tests/engine-mode-test.sh`  
Expected: FAIL.

**Step 3: Implement engine mode resolver + bridge invocation**

Use env/config keys:
- `ODIN_ENGINE_MODE`
- `ODIN_CORE_BIN`
- `ODIN_CORE_CONFIG`
- `ODIN_LEGACY_ROOT`

**Step 4: Run test to verify it passes**

Run: `bash odin-orchestrator/scripts/odin/tests/engine-mode-test.sh`  
Expected: PASS.

**Step 5: Commit**

```bash
git -C odin-orchestrator add scripts/odin/lib/engine-mode.sh scripts/odin/lib/engine-bridge.sh scripts/odin/odin-service.sh scripts/odin/odin-inbox-processor.sh scripts/odin/tests/engine-mode-test.sh
git -C odin-orchestrator commit -m "feat(engine): add legacy-shadow-core wrapper and bridge routing"
```

### Task 8: Add Golden Task Fixtures and Parity Comparator

**Files:**
- Create: `odin-core/tests/fixtures/migration-golden/dispatch_work.json`
- Create: `odin-core/tests/fixtures/migration-golden/incident_diagnose.json`
- Create: `odin-core/tests/fixtures/migration-golden/watchdog_poll.json`
- Create: `odin-core/scripts/verify/migration-golden.sh`
- Create: `odin-core/scripts/verify/lib/normalize-events.sh`
- Create: `odin-core/docs/baselines/migration-golden-matrix.md`

**Step 1: Write failing parity script test harness**

Expected to fail until comparator and fixtures are in place.

**Step 2: Run test to verify it fails**

Run: `bash odin-core/scripts/verify/migration-golden.sh --dry-run`  
Expected: FAIL.

**Step 3: Implement fixture execution + event normalization + compare report**

Output:
- `proof/golden/compare-report.json`

**Step 4: Run test to verify it passes**

Run: `bash odin-core/scripts/verify/migration-golden.sh --legacy-root /home/orchestrator/cfipros`  
Expected: PASS (or PASS with approved deltas file).

**Step 5: Commit**

```bash
git -C odin-core add tests/fixtures/migration-golden scripts/verify/migration-golden.sh scripts/verify/lib/normalize-events.sh docs/baselines/migration-golden-matrix.md
git -C odin-core commit -m "test(migration): add golden parity suite and comparator"
```

### Task 9: Add Rollback Automation

**Files:**
- Create: `odin-core/scripts/migrate/rollback.sh`
- Create: `odin-core/scripts/verify/rollback-rehearsal.sh`
- Modify: `odin-core/docs/upgrade-v0x-to-v1x.md`
- Create: `odin-core/docs/baselines/rollback-checklist.md`

**Step 1: Write failing rollback rehearsal script**

Script should fail until backups, restore flow, and mode flip checks are implemented.

**Step 2: Run test to verify it fails**

Run: `bash odin-core/scripts/verify/rollback-rehearsal.sh`  
Expected: FAIL.

**Step 3: Implement rollback script + rehearsal checks**

Must verify:
- engine mode reset to `legacy`
- restore from `meta/backups/<timestamp>`
- legacy smoke check run

**Step 4: Run test to verify it passes**

Run: `bash odin-core/scripts/verify/rollback-rehearsal.sh`  
Expected: PASS.

**Step 5: Commit**

```bash
git -C odin-core add scripts/migrate/rollback.sh scripts/verify/rollback-rehearsal.sh docs/upgrade-v0x-to-v1x.md docs/baselines/rollback-checklist.md
git -C odin-core commit -m "feat(migration): add automated rollback and rehearsal verification"
```

### Task 10: Wire Migration Gates Into CI

**Files:**
- Modify: `odin-core/.github/workflows/ci.yml`
- Create: `odin-core/scripts/verify/migration-smoke.sh`
- Modify: `odin-core/docs/release-readiness.md`

**Step 1: Write failing workflow contract check**

Ensure CI contract requires migration smoke + golden + rollback rehearsal on protected branches.

**Step 2: Run test to verify it fails**

Run: `bash odin-core/scripts/verify/workflow-contract.sh`  
Expected: FAIL.

**Step 3: Add migration verification steps to CI + smoke wrapper**

Include:
- exporter/importer dry-run smoke
- bundle validate
- golden parity dry-run

**Step 4: Run test to verify it passes**

Run: `bash odin-core/scripts/verify/workflow-contract.sh`  
Expected: PASS.

**Step 5: Commit**

```bash
git -C odin-core add .github/workflows/ci.yml scripts/verify/migration-smoke.sh docs/release-readiness.md
git -C odin-core commit -m "ci(migration): enforce export-import and parity smoke gates"
```

### Task 11: End-User Upgrade Guide Finalization

**Files:**
- Modify: `odin-core/docs/upgrade-v0x-to-v1x.md`
- Modify: `odin-core/README.md`
- Create: `odin-core/docs/integrations/migration-troubleshooting.md`

**Step 1: Write failing docs contract check**

Check guide includes:
- backup
- export/import
- shadow mode
- rollback
- wake_up/resume recovery

**Step 2: Run test to verify it fails**

Run: `bash odin-core/scripts/verify/workflow-contract.sh`  
Expected: FAIL on missing docs anchors.

**Step 3: Add final operator-facing docs and troubleshooting table**

Keep all steps end-user runnable, no private-only command path.

**Step 4: Run test to verify it passes**

Run: `bash odin-core/scripts/verify/workflow-contract.sh`  
Expected: PASS.

**Step 5: Commit**

```bash
git -C odin-core add docs/upgrade-v0x-to-v1x.md README.md docs/integrations/migration-troubleshooting.md
git -C odin-core commit -m "docs(migration): publish end-user upgrade and troubleshooting guide"
```

### Task 12: Full Rehearsal and Proof Artifact Bundle

**Files:**
- Create: `odin-core/scripts/verify/migration-rehearsal.sh`
- Create: `odin-core/docs/baselines/migration-proof-template.md`
- Create: `odin-core/proof/.gitkeep`

**Step 1: Write failing rehearsal script for end-to-end dry run**

Script should require all proof files and fail if any are missing.

**Step 2: Run test to verify it fails**

Run: `bash odin-core/scripts/verify/migration-rehearsal.sh --legacy-root /home/orchestrator/cfipros --odin-dir /var/odin`  
Expected: FAIL.

**Step 3: Implement full rehearsal pipeline**

Sequence:
- inventory pre snapshot
- export bundle
- validate bundle
- import dry-run
- golden parity
- rollback rehearsal
- inventory post snapshot + count diff

**Step 4: Run test to verify it passes**

Run: `bash odin-core/scripts/verify/migration-rehearsal.sh --legacy-root /home/orchestrator/cfipros --odin-dir /var/odin`  
Expected: PASS and `proof/` populated.

**Step 5: Commit**

```bash
git -C odin-core add scripts/verify/migration-rehearsal.sh docs/baselines/migration-proof-template.md proof/.gitkeep
git -C odin-core commit -m "chore(migration): add end-to-end rehearsal and proof bundle template"
```

---

## Global Verification Checklist (Run Before Any Merge/Cutover)

- `cargo test -p odin-migration`
- `cargo test -p odin-cli`
- `bash scripts/verify/compat-regression.sh --legacy-root /home/orchestrator/cfipros`
- `bash scripts/verify/migration-smoke.sh`
- `bash scripts/verify/migration-golden.sh --legacy-root /home/orchestrator/cfipros`
- `bash scripts/verify/rollback-rehearsal.sh`
- `bash scripts/verify/migration-rehearsal.sh --legacy-root /home/orchestrator/cfipros --odin-dir /var/odin`

Expected: all pass, with proof artifacts generated and no unapproved parity deltas.

## Release Gate

Do not enable `ODIN_ENGINE_MODE=core` by default until:
- proof bundle complete
- rollback rehearsal verified
- golden parity threshold met
- quarantined items reviewed and accepted

