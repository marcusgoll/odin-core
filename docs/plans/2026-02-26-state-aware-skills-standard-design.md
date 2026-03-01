# State-Aware Skills Standard (SASS) v0.1 Design

## Goal

Define and adopt a runtime-first skill system where Odin skills are explicitly executable as state machines, resumable through a mandatory `wake_up` checkpoint, and auditable through structured telemetry and capability enforcement.

## Scope

- **Authoritative spec and templates:** `odin-core`
- **Audit target surface:** `odin-orchestrator` runner behavior and prompts + `odin-core` governance/policy contracts
- **Out of scope for v0.1:** full marketplace/distribution UX, cross-org trust federation, non-deterministic prompt-only skills

## Constraints

- Runtime-first execution (state engine is source of truth, not prose prompts)
- Least privilege capability model must be preserved
- Existing orchestrator behavior must remain operable during migration
- Resume must be deterministic and safe under partial progress
- No success claims without verifiable artifacts/events

## Repo Evidence (Current Baseline)

- Orchestrator lifecycle and wake-cycle behavior live in `odin-orchestrator/scripts/odin/odin-prompt.md` and `scripts/odin/lib/*.sh`.
- Resume/checkpoint logic exists for agents in `scripts/odin/lib/agent-supervisor.sh` (`checkpoint_agent`, `resume_agent`), but not as a first-class skill state machine contract.
- Verification gate exists in `scripts/odin/lib/verification-gate.sh`, but checks are command-list based and not state-scoped.
- Task dispatch state and trust tagging exist in `scripts/odin/lib/task-queue.sh` and `state.json` metadata.
- Capability/policy and audit foundations exist in `odin-core` (`odin-policy-engine`, `odin-audit`, policy docs), but there is no executable in-repo skill state DSL yet.

## Approaches Considered

### 1) Docs-only standard

Pros:
- quick to publish
- low implementation cost

Cons:
- no enforcement
- high drift risk
- does not satisfy explicit executability

### 2) Runtime-first state engine (selected)

Pros:
- aligns with requirement: "do it right"
- execution semantics become explicit and testable
- enables deterministic wake/resume and auditable transitions

Cons:
- highest implementation scope
- requires phased migration and compatibility shims

### 3) Contract-first without runtime

Pros:
- improves consistency quickly
- easier incremental rollout

Cons:
- still leaves runtime behavior implicit
- enforcement limited until engine adoption

## Selected Design: Runtime-First with Contract Guardrails

### Architecture Components

1. **Skill Definition Loader**
- Loads XML skill specs from scoped registries (`user > project > global`).
- Validates schema, trust level, and capability manifest before run.

2. **State Engine (authoritative runtime)**
- Executes explicit states/transitions.
- Supports branch transitions, retry/failure edges, checkpoints, and terminal exits.
- Enforces per-state idempotence policy.

3. **Guard Evaluator**
- Runs state guard checks before state action execution.
- Emits structured pass/fail outcomes and routes guard failures via explicit failure edges.

4. **Execution Adapter Layer**
- Binds state actions to concrete executors (shell/tool/delegation/policy check).
- Performs capability checks before execution.

5. **State Store + Resume Tokens**
- Persists pacman coordinates after each transition.
- Powers deterministic `wake_up` resume vs restart behavior.

6. **Telemetry + Audit Sink**
- Emits transition-level events and links policy decisions to skill execution via correlation IDs.

7. **Projection Layer**
- Compiles XML to Mermaid `stateDiagram-v2` and Mermaid DAG flowchart.
- Mermaid is generated output; XML stays canonical.

## SASS v0.1 Minimal Enforceable Contract

Each skill spec must include:

- `skill_id`, `version`, `scope`, `trust_level`
- `capability_manifest` (allowed and denied capabilities)
- `start_state` and explicit exit states
- explicit states and transitions
- explicit guards on operational states
- assumption-to-failure mapping
- per-state idempotence mode
- telemetry event requirements
- resume coordinates schema

### Mandatory `wake_up`

- `wake_up` is required and must be `start_state`.
- No operational state may run before `wake_up` completes.
- `wake_up` resolves context, validates permissions, inspects checkpoints, and chooses `resume|restart|abort` transition.

### Mandatory Runtime Coordinates

Persist and reload:

- `workspace_root`
- `project_id`
- `task_id` or `ticket_id`
- `skill_id`
- `state_id`
- `attempt_count`
- `last_artifacts`
- `allowed_capabilities`

### Idempotence Modes

- `safe_repeat`
- `requires_checkpoint`
- `non_repeatable`

`non_repeatable` states require checkpoint evidence before re-entry.

### Required Telemetry Event Types

- `state_enter`
- `guard_eval`
- `action_exec`
- `transition`
- `state_exit`
- `state_fail`
- `run_exit`

## Representation Model

### A) XML Skill DSL (source of truth)

Executable representation used by runtime and linting.

### B) Mermaid `stateDiagram-v2` (generated)

For branch/recovery/resume-heavy skills.

### C) Mermaid DAG flowchart (generated)

For mostly-linear operator readability.

### Compilation Guarantees

Compilation fails if:

- `wake_up` missing
- no terminal exit state
- missing transition target
- unguarded operational state

## `wake_up` Runtime Semantics (Pacman Coordinates)

### Deterministic Resume Algorithm

1. Load previous coordinates by `(skill_id, task_id)`.
2. Validate coordinate integrity and policy/capability constraints.
3. Re-evaluate guards for candidate resume state.
4. If valid -> resume.
5. If invalid but recoverable -> route recovery state.
6. If invalid and unsafe -> controlled restart or permission/block exit.

### Safety Invariants

- Atomic coordinate persistence per transition
- Guard re-check on resumed state entry
- Permission narrowing cannot be bypassed by resume

## Audit Rubric (0-2 per Criterion)

1. Explicit start/exit states
2. Explicit decision splits
3. Checkable guards/preconditions
4. Assumptions mapped to failure transitions
5. Observability and artifacts
6. Resume capability via `wake_up` + coordinates
7. Idempotence by state
8. Capability discipline (least privilege)

### Weighted Compliance

Critical dimensions (`resume`, `guards`, `failure handling`, `capability discipline`) carry higher weight.

- `L0`: <40%
- `L1`: 40-64%
- `L2`: 65-84%
- `L3`: 85%+ with no critical zeroes

## Worked Example Behavior: `run_tests`

State flow (high level):

`wake_up -> detect_workspace -> resolve_project -> ensure_runtime -> discover_tests -> execute_tests -> interpret_result -> exit`

Failure paths include:

- unknown project
- missing runtime
- tests not found
- execution error (retry path)
- unparseable result
- permission denied

This satisfies the requirement for explicit starts, branching, guards, and failure handling.

## Migration Strategy

1. Build state engine and DSL parser in `odin-core`.
2. Introduce linter/CI checks (block non-compliant skills).
3. Convert highest-value/high-frequency skills first.
4. Run compatibility mode where legacy prompt-only skills remain allowed but flagged.
5. Gradually enforce strict mode (wake_up + guards + exits mandatory).

## Success Criteria

- All new skills require `wake_up` and explicit exits.
- `run_tests` example demonstrates branching and >=3 failure transitions.
- CI blocks skill specs missing required state-machine invariants.
- Audit output for legacy prompts/skills is reproducible and evidence-backed.
