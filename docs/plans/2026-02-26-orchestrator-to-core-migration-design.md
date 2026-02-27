# Odin Orchestrator -> Odin Core Migration Design

**Date:** 2026-02-26  
**Authoring mode:** Brainstorming/design only (no runtime behavior changed in this document)

## Goal

Migrate a mature personal Odin runtime from legacy orchestrator behavior to `odin-core` as execution engine without losing recoverable skills, learnings, or history, while preserving a reversible rollback path and an end-user-grade upgrade flow.

Success criteria:
1. `odin-orchestrator` runs with `odin-core` as execution engine.
2. Recoverable skills/learnings/state are preserved with measurable proofs.
3. Rollback restores prior engine path and prior user data.
4. Upgrade is documented as a normal end-user path (no hidden personal hacks).

## Non-Goals

- Rewrite-all consolidation in one step.
- Migrating private plugin business logic into open-source core in this phase.
- Converting every legacy skill into state-machine XML in one release.

## Targeted Questions (Non-Blocking)

These are the only unknowns that materially affect defaults; plan below proceeds with explicit assumptions:
1. Should long-term default user-data root for end users be `~/.odin` (user install) or `/var/odin` (service install)?
2. Is `.claude/sessions` history required in v1 migration scope, or only active skills/learnings/checkpoints?
3. Is the canonical legacy source for this migration `odin-orchestrator`, `cfipros`, or both?

## Assumptions Used for This Plan

- Use `/var/odin` as canonical runtime data root for current server-style installs.
- Migrate both repo-defined and runtime-discovered skills/learnings; session history is exported as optional archival data.
- Treat both `odin-orchestrator` and live runtime paths as source-of-truth inputs.

---

## 1) Inventory Report (Current Evidence)

All inventory below is from direct repo/runtime inspection.

### 1.1 `odin-orchestrator` inventory

#### Skill locations and loading

- Auto-discovery of skills from repo-relative `.claude/skills/*/SKILL.md` is implemented in [agent-lifecycle.sh](/home/orchestrator/odin-orchestrator/scripts/odin/lib/agent-lifecycle.sh).
- Learn pipeline can create `odin-*` skills under `.claude/skills/odin-*/SKILL.md` in [learn-pipeline.sh](/home/orchestrator/odin-orchestrator/scripts/odin/lib/learn-pipeline.sh).
- `scripts/odin/skills/` in this repo currently contains only placeholder content (no active registry logic).

#### Runtime state and persistence

- Core runtime root defaults to `/var/odin` in [install.sh](/home/orchestrator/odin-orchestrator/scripts/odin/install.sh), [odin-service.sh](/home/orchestrator/odin-orchestrator/scripts/odin/odin-service.sh), [task-queue.sh](/home/orchestrator/odin-orchestrator/scripts/odin/lib/task-queue.sh), [memory-sync.sh](/home/orchestrator/odin-orchestrator/scripts/odin/lib/memory-sync.sh), and [project-registry.sh](/home/orchestrator/odin-orchestrator/scripts/odin/lib/project-registry.sh).
- Queue contracts (`inbox`, `outbox`, `rejected`) and atomic tmp+rename semantics are in [task-queue.sh](/home/orchestrator/odin-orchestrator/scripts/odin/lib/task-queue.sh) and ingress validation in [odin-inbox-write.sh](/home/orchestrator/odin-orchestrator/scripts/odin/odin-inbox-write.sh).
- Learning/state persistence:
  - hot memory: `/var/odin/memory/hot/*.md`
  - cold memory: `/var/odin/memory/cold/*`
  - tuning/routing/session state in `/var/odin/state.json`, `/var/odin/routing.json`, `/var/odin/projects.json`, `/var/odin/conversations.json`.
- Resume/checkpoint logic exists at agent level in [agent-supervisor.sh](/home/orchestrator/odin-orchestrator/scripts/odin/lib/agent-supervisor.sh) (`checkpoint_agent`, `resume_agent`), with no global skill-checkpoint schema yet.

#### Observed live runtime counts (2026-02-26)

- `cfipros/.claude/skills` directories: `33`
- `cfipros/.claude/skills/*/SKILL.md`: `28`
- `/var/odin/memory/hot/*.md`: `9`
- `/var/odin/memory/cold/*.md`: `3`
- `learnings` entries (`## LEARN:`): `28`
- `decisions` entries: `9`
- `/var/odin/agents/*/status.json`: `24`
- `/var/odin/agents/*/done-*`: `57`
- `/var/odin/outbox/*.json`: `382`
- `/var/odin/rejected/*`: `530`

### 1.2 `odin-core` inventory

#### Engine boundary and compat status

- Runtime traits (`TaskIngress`, `BackendState`, `FailoverController`) are in [odin-core-runtime lib](/home/orchestrator/odin-core/crates/odin-core-runtime/src/lib.rs).
- Legacy Bash adapters exist in [odin-compat-bash lib](/home/orchestrator/odin-core/crates/odin-compat-bash/src/lib.rs):
  - `BashTaskIngressAdapter` -> legacy `odin-inbox-write.sh`
  - `BashBackendStateAdapter` -> legacy `backend-state.sh`
  - `BashFailoverAdapter` -> legacy `orchestrator-failover.sh`
- CLI supports compat bridge parameters in [odin-cli main](/home/orchestrator/odin-core/bin/odin-cli/src/main.rs): `--legacy-root`, `--legacy-odin-dir`.
- Default runtime mode is compat in [default.yaml](/home/orchestrator/odin-core/config/default.yaml):
  - `runtime.mode: compat`
  - `queue_impl: compat`
  - `policy_impl: native`
  - `plugin_loader: native`

#### Existing migration contract docs

- Compatibility guarantees and invariants are already defined in [compat-adapter-contract.md](/home/orchestrator/odin-core/docs/compat-adapter-contract.md).
- Regression gate harness exists in [compat-regression.sh](/home/orchestrator/odin-core/scripts/verify/compat-regression.sh).
- Phase checklist exists in [migration-checklist.md](/home/orchestrator/odin-core/docs/migration-checklist.md).

### 1.3 Implicit behaviors and hidden assumptions

These must be treated as migration inputs, not ignored side effects:

- Magic root: `ODIN_DIR` defaults to `/var/odin` almost everywhere.
- Repo-relative skill discovery assumes `.claude/skills` exists under `REPO_DIR` even if actual skill corpus is project-external.
- Hidden runtime cache/state includes `/var/odin/.codex-orchestrator/*` (auth/config/sqlite/snapshots).
- Project registry in `/var/odin/projects.json` points runtime to project roots (currently default project `cfipros`).
- CLI/auth assumptions include `~/.codex/auth.json`, `~/.odin-env`, and installed `claude`/`codex` CLIs.

---

## 2) User Data Model Definition

## Decision: Introduce canonical User Data Model v1

**Rationale:** Current user data is spread across repo-relative skill folders, `/var/odin`, and hidden CLI homes. A versioned canonical model prevents silent loss during engine swaps.

**Risks:** Initial export/import complexity; false confidence if schema misses a data class.

**Rollback:** Keep original data untouched, write migration output to new root, and require explicit cutover flag.

### 2.1 Canonical layout (`user-data-model v1`)

`<ODIN_DATA_ROOT>/`

- `meta/`
  - `manifest.json` (version, timestamps, source fingerprints)
  - `checksums.sha256`
  - `migrations/` (applied migration records)
  - `backups/` (pre-import snapshots)
- `skills/`
  - `packs/<skill_pack_id>/` (skill files as-imported)
  - `index.json` (normalized skill metadata)
  - `quarantine/` (invalid/unparseable skills)
- `learnings/`
  - `hot/*.md`
  - `cold/*.md`
  - `archives/**/*.md`
  - `index.json`
- `runtime/`
  - `state.json`
  - `routing.json`
  - `projects.json`
  - `conversations.json`
  - `budgets/*.json`
- `checkpoints/`
  - `agents/<agent>/status.json`
  - `agents/<agent>/checkpoint.json`
  - `resume_tokens/*.json`
- `events/`
  - `inbox/*.json`
  - `outbox/*.json`
  - `rejected/*.json`
- `logs/`
  - `legacy/` (optional import)
- `opaque/`
  - `codex/` (optional archival)
  - `claude/` (optional archival)

### 2.2 Versioning and forward migrations

- `user_data_model_version` is integer-major (`1`, `2`, ...).
- Every importer writes `meta/migrations/<timestamp>-from-<n>-to-<n+1>.json`.
- Forward migration rule: never in-place mutate without backup snapshot.
- Idempotence rule: repeated import of same bundle hash must produce zero net changes.

---

## 3) Compatibility Strategy Selection

Selected strategy: **B) Subprocess bridge** (`odin-orchestrator` shells to `odin-core` binary over structured JSON/stdout contracts).

### Why B over A/C

- **A (adapter library)** is poor fit because orchestrator is Bash-heavy; direct Rust library embedding requires major host rewrite.
- **C (full consolidation)** violates staged safety and rollback requirements.
- **B** matches existing `odin-core` compat posture and keeps engine boundary stable while allowing gradual migration.

### Decision details

- `odin-orchestrator` becomes thin workflow/UI wrapper.
- `odin-core` handles policy, plugin execution, and task processing.
- Bridge contract is versioned JSON envelopes, invoked via `odin-cli` (initially file/stdio, then optional JSON-RPC).

**Rationale:** Lowest migration risk with immediate operational path.

**Risks:** IPC contract drift or partial parity gaps.

**Rollback:** Switch engine flag back to legacy path; preserve original scripts and data roots.

---

## 4) Migration Mechanics

## Decision: Pack-based export/import with deterministic manifests

**Rationale:** Treat skills/learnings/state as portable user data; deterministic packs provide proofs and repeatability.

**Risks:** Large bundles; mixed-quality legacy content.

**Rollback:** Export is read-only; import writes backup + transaction log + per-item status.

### 4.1 Exporter artifacts (legacy -> pack)

Create in `odin-core`:

- `bin/odin-cli` subcommands:
  - `odin-cli migrate export --source-root <legacy> --odin-dir <dir> --out <bundle-dir>`
  - `odin-cli migrate validate --bundle <bundle-dir>`
- Schema files:
  - `schemas/user-data-model.v1.schema.json`
  - `schemas/skill-pack.v1.schema.json`
  - `schemas/learning-pack.v1.schema.json`

Export output directory:

- `bundle/manifest.json`
- `bundle/skills/`
- `bundle/learnings/`
- `bundle/runtime/`
- `bundle/checkpoints/`
- `bundle/events/`
- `bundle/opaque/` (optional)
- `bundle/quarantine/`
- `bundle/checksums.sha256`

### 4.2 Importer artifacts (pack -> canonical user-data)

Create in `odin-core`:

- `odin-cli migrate import --bundle <bundle-dir> --target <odin-data-root> --dry-run`
- `odin-cli migrate import --bundle <bundle-dir> --target <odin-data-root> --apply`

Importer behaviors:

- Acquire migration lock (`meta/migration.lock`).
- Snapshot backup of target mutable dirs before write.
- Upsert by stable key (skill ID/path hash, task ID, agent name).
- Write per-item result log: `meta/import-report.json`.
- Quarantine invalid/unmapped items with reason code.

### 4.3 Mapping table (old -> new)

| Legacy artifact type | Old location(s) | New location | Action |
|---|---|---|---|
| Project skill packs (`SKILL.md`) | `<project>/.claude/skills/*/SKILL.md` | `skills/packs/project-<project>/...` + `skills/index.json` | Copy + normalize metadata |
| Auto-generated Odin skills (`odin-*`) | `<project>/.claude/skills/odin-*/SKILL.md` | `skills/packs/generated-odin/...` | Copy; tag `origin=learn-pipeline` |
| Codex local skills | `~/.codex/skills/*/SKILL.md` | `skills/packs/codex-local/...` | Copy; preserve provenance |
| Superpowers skills | `~/.codex/superpowers/skills/*/SKILL.md` | `skills/packs/superpowers/...` | Copy or mark as external reference |
| Future SASS XML skills | `**/*.skill.xml` | `skills/packs/*/*.skill.xml` | Copy + schema-validate |
| Hot learnings | `/var/odin/memory/hot/*.md` | `learnings/hot/*.md` | Copy + checksum |
| Cold learnings | `/var/odin/memory/cold/*.md` | `learnings/cold/*.md` | Copy + checksum |
| Learning archives | `/var/odin/memory/cold/learnings-archive*` | `learnings/archives/` | Copy + index |
| Runtime state | `/var/odin/state.json` | `runtime/state.json` | Copy + schema-validate |
| Routing state | `/var/odin/routing.json` | `runtime/routing.json` | Copy + schema-validate |
| Project registry | `/var/odin/projects.json` | `runtime/projects.json` | Copy + schema-validate |
| Conversations | `/var/odin/conversations.json` | `runtime/conversations.json` | Copy + schema-validate |
| Queue backlog | `/var/odin/inbox/*.json` | `events/inbox/*.json` | Copy + revalidate task schema |
| Task history | `/var/odin/outbox/*.json` | `events/outbox/*.json` | Copy |
| Rejected tasks | `/var/odin/rejected/*` | `events/rejected/*` | Copy |
| Agent statuses | `/var/odin/agents/*/status.json` | `checkpoints/agents/*/status.json` | Copy |
| Agent checkpoints | `/var/odin/agents/*/checkpoint.json` | `checkpoints/agents/*/checkpoint.json` | Copy |
| Done-file outputs | `/var/odin/agents/*/done-*` | `checkpoints/agents/*/done/*` | Copy |
| Opaque CLI/session state | `/var/odin/.codex-orchestrator`, `~/.claude/*`, `~/.codex/state*.sqlite` | `opaque/*` | Archive; do not mutate |

### 4.4 Incompatible item strategy

- **Transform:** minor schema drift (missing optional fields) -> normalized item with migration note.
- **Quarantine:** parse failure, unsafe path, checksum mismatch -> `skills/quarantine` or `events/quarantine`.
- **Manual review:** executable scripts embedded in skill packs, unknown binary blobs, or path traversal candidates.

---

## 5) Validation and Proofs

## Decision: Proof bundle as release gate

**Rationale:** Preservation claims must be testable and auditable.

**Risks:** Added release overhead.

**Rollback:** If proof gate fails, do not cut over; continue on legacy engine.

### 5.1 Required proof artifacts

- `proof/pre-export-inventory.json`
- `proof/post-import-inventory.json`
- `proof/count-diff.json`
- `proof/checksums.sha256`
- `proof/golden/compare-report.json`

### 5.2 Required count checks (before/after)

- skill files count by source class
- learning files count (hot/cold/archive)
- queue counts (inbox/outbox/rejected)
- checkpoint/status counts
- key runtime files present and parseable

### 5.3 Checksums

- SHA-256 for every exported file.
- Top-level bundle checksum recorded in `manifest.json`.
- Importer re-hashes post-write and records `hash_match=true|false`.

### 5.4 Golden task suite (old vs new)

Build deterministic golden suite from representative tasks:

- `dispatch_work`
- `incident_diagnose`
- `watchdog_poll` plugin enqueue flow
- one approval-required action path

Run modes:

1. Legacy execution path (`odin-orchestrator` current).
2. New bridge path (`odin-orchestrator` -> `odin-core`).

Compare:

- normalized event stream (`policy.decision`, `task.enqueued`, completion status)
- task output contract fields
- side effects in queue/state files

Gate:

- no schema regressions
- no missing required events
- expected behavioral deltas explicitly listed and approved

---

## 6) Cutover Plan

## Decision: staged cutover with shadow mode and hard rollback switch

**Rationale:** Avoid one-way migration risk.

**Risks:** Shadow-mode dual processing could duplicate actions if not isolated.

**Rollback:** One env/config switch + data restore from backup snapshot.

### 6.1 Modes

- `legacy`: current engine only.
- `shadow`: legacy authoritative, core runs in parallel read/compare mode.
- `core`: core authoritative, legacy path idle.

### 6.2 Engine toggle mechanism

Introduce explicit toggle in orchestrator runtime config/env:

- `ODIN_ENGINE_MODE=legacy|shadow|core`
- `ODIN_CORE_BIN=/path/to/odin-cli`
- `ODIN_CORE_CONFIG=/path/to/config.yaml`
- `ODIN_LEGACY_ROOT=/path/to/odin-orchestrator`
- `ODIN_DATA_ROOT=/var/odin`

### 6.3 Cutover sequence

1. Freeze window + preflight checks.
2. Export bundle + checksums + inventory report.
3. Import to canonical layout in dry-run, then apply.
4. Run golden suite in `shadow` mode until parity threshold met.
5. Flip to `core` mode.
6. Observe for defined soak period.

### 6.4 Rollback procedure

1. Set `ODIN_ENGINE_MODE=legacy`.
2. Stop core processing loop.
3. Restore backup snapshot from `meta/backups/<timestamp>/`.
4. Re-run legacy smoke checks.
5. Mark migration attempt failed with reason in migration log.

Rollback must be tested before production cutover.

---

## 7) End-User Upgrade Path

A user-facing guide is authored at:

- [upgrade-v0x-to-v1x.md](/home/orchestrator/odin-core/docs/upgrade-v0x-to-v1x.md)

Guide requirements met:

- step-by-step backup/export/import/validate/cutover
- engine toggle instructions
- rollback steps
- common failures and `wake_up`/resume recovery behavior

---

## Implementation Plan (Phased)

### Phase 1 - Data contract and tooling (no cutover)

Deliverables:

- migration schemas (`user-data-model`, `skill-pack`, `learning-pack`)
- `migrate export`, `migrate validate`, `migrate import --dry-run`
- inventory command and proof file generator

Verification:

- exporter/importer idempotence test (run twice; zero diff second run)
- checksum verification tests

### Phase 2 - Bridge integration

Deliverables:

- orchestrator bridge runner wrapper (`legacy|shadow|core`)
- structured JSON contract between wrapper and `odin-cli`
- config/env toggles documented

Verification:

- compat regression matrix
- bridge smoke tests for enqueue + policy + failover paths

### Phase 3 - Golden parity and shadow mode

Deliverables:

- golden tasks fixture set
- event normalizer + comparator
- shadow-mode report artifacts

Verification:

- parity threshold met on golden suite
- known deltas reviewed and accepted

### Phase 4 - Cutover + rollback rehearsal

Deliverables:

- production migration runbook
- rollback rehearsal evidence
- release notes with migration and rollback commands

Verification:

- rollback drill succeeds
- post-cutover health + queue integrity checks pass

### Phase 5 - State-aware skill progression

Deliverables:

- begin converting high-frequency skills to SASS (`wake_up` + coordinates)
- resume token persistence under canonical `checkpoints/`
- strict validation for converted skills

Verification:

- converted skills resume deterministically after interruption
- failure transitions produce auditable events

---

## Prime-User Requirement Compliance

- Uses same export/import binaries and same config switches intended for any user.
- No hidden private-only code path is required for migration success.
- Any local path overrides are explicit inputs, not hardcoded behavior.

## Hard Failure Conditions (Guardrail)

Migration must be rejected if any of these occur:

- missing pre/post inventory proof
- checksum mismatch on imported artifacts
- unresolved quarantined items above configured threshold
- no tested rollback path
- inability to run orchestrator through `odin-core` bridge mode

