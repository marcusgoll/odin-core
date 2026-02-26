# Odin Meta Prompt and Meta Skill Audit (SASS v0.1 Rubric)

## Scope and Evidence

Audit targets:
- `odin-orchestrator` meta prompts and meta skills
- `odin-core` meta prompts and meta skills

Primary evidence:
- `/home/orchestrator/odin-orchestrator/scripts/odin/odin-prompt.md`
- `/home/orchestrator/odin-orchestrator/hot/skill-evolution.md`
- `/home/orchestrator/odin-orchestrator/cold/skill-evolution.md`
- `/home/orchestrator/odin-orchestrator/scripts/odin/skills/.gitkeep`
- `/home/orchestrator/odin-core/.worktrees/sass-v01-state-aware-skills/docs/plans/2026-02-25-skill-plugin-governance-design.md`
- `/home/orchestrator/odin-core/.worktrees/sass-v01-state-aware-skills/docs/plans/2026-02-26-state-aware-skills-standard-design.md`

## Rubric (0-2)

- `0`: missing or not machine-checkable
- `1`: partial/procedural only
- `2`: explicit, structured, and machine-checkable

Dimensions:
- explicit start/exit
- decision splits
- guards/preconditions
- assumptions + failure transitions
- observability
- resume/wake_up capability
- idempotence
- capability discipline

## Score Table

| Target | Start/Exit | Splits | Guards | Assumptions+Failures | Observability | Resume/wake_up | Idempotence | Capability Discipline | Total / 16 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `odin-orchestrator` meta prompt (`scripts/odin/odin-prompt.md`) | 1 | 2 | 1 | 1 | 2 | 1 | 0 | 1 | 9 |
| `odin-orchestrator` meta skills surface (`hot/skill-evolution.md`, `scripts/odin/skills/.gitkeep`) | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 1 |
| `odin-core` meta prompt surface (no in-repo meta prompt artifact) | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `odin-core` meta skills surface (design-only references, no executable skill specs) | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 1 | 2 |

## Findings by Repository

### odin-orchestrator

Strengths:
- strong procedural decision logic in polling/dispatch/monitor loop
- substantial operational observability through state files and telemetry-oriented scripts

Gaps:
- no explicit state-machine artifact for meta prompt execution
- no mandatory `wake_up` state with persisted coordinates contract
- no explicit idempotence policy by state
- no executable meta-skill definitions under `scripts/odin/skills/`

### odin-core

Strengths:
- governance and SASS design intent are documented in planning docs
- capability and policy intent exists at architecture level

Gaps:
- no current in-repo meta prompt artifact
- no executable meta-skill XML/DAG/statechart baseline
- no CI gate yet enforcing `wake_up` + exits + guards for skill definitions

## Top Failures

1. Meta skills are not executable contracts in either repo.
2. `wake_up` resume checkpoint is not yet a hard requirement in active skill artifacts.
3. Guard checks and failure transitions are mostly prose/procedural, not lintable transitions.
4. Idempotence is undefined for operational states.
5. Capability discipline is not bound per state/action in executable skill specs.

## Concrete Remediation Table

| Failure | Required Remediation | Owner Repo | Artifact | Gate |
|---|---|---|---|---|
| Missing executable meta skills | Author XML DSL skills with explicit states/transitions | `odin-core` + `odin-orchestrator` | `docs/odin/skills/templates/skill.template.xml`, `docs/odin/skills/examples/*.skill.xml` | `scripts/verify/skills-contract.sh` |
| Missing mandatory wake checkpoint | Enforce `runtime start_state="wake_up"` and `state id="wake_up"` | `odin-core` | skill XML template + examples | `skills-contract.sh` hard fail |
| Missing explicit exits | Require `type="exit"`/`terminal="true"` states in every skill | `odin-core` | template + examples | `skills-contract.sh` hard fail |
| Missing guard discipline | Require guard definitions and guarded transitions from operational states | `odin-core` | example XML + template | `skills-contract.sh` hard fail |
| No CI enforcement | Add workflow job steps for workflow + skills contract checks | `odin-core` | `.github/workflows/ci.yml` | `workflow-contract.sh` |
| Orchestrator prompt-only skills | Convert top task families into SASS XML definitions | `odin-orchestrator` (consumer), `odin-core` (standard) | migration batch in plan | CI + migration tracker |

## Priority Summary

- Immediate (P0): enforce XML contract in CI (`wake_up`, exits, guards).
- Near-term (P1): convert high-frequency orchestrator skills first.
- Mid-term (P2): add state-level idempotence and capability manifests to all migrated skills.
