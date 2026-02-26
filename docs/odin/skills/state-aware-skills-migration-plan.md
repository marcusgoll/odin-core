# SASS Migration Plan (v0.1)

## Goal

Move Odin from prompt/procedural skill behavior to executable, state-aware skills with mandatory `wake_up`, explicit guards, explicit failures, and terminal exits.

## Migration Phases

1. Baseline and lint: publish standard/template/examples and enforce CI checks.
2. Top-10 conversion batch: convert highest-volume/highest-risk orchestrator skills first.
3. Dual-run period: keep prompt behavior as compatibility fallback while logging SASS parity.
4. Enforcement: require SASS-compliant skill definitions for all new skills.

## Top 10 Skills to Convert First

Selection criteria:
- high execution frequency
- high blast radius on failure
- direct dependency on routing/verification/capabilities

1. `triage`
2. `dispatch_work`
3. `pr_review`
4. `acceptance_test`
5. `issue_implement`
6. `pr_fix`
7. `sentry_fix`
8. `deploy_staging`
9. `deploy_prod`
10. `security_scan`

## Conversion Order Rationale

- `triage` and `dispatch_work` shape all downstream work; converting first increases determinism.
- review/testing skills (`pr_review`, `acceptance_test`) protect quality gates.
- worker execution skills (`issue_implement`, `pr_fix`, `sentry_fix`) need explicit retries/failures.
- deploy/security skills need strict guard and capability discipline from day one.

## CI Gates to Add

Required immediately:
- `scripts/verify/skills-contract.sh`:
  - fail if skill XML is missing `wake_up`
  - fail if skill XML has no exit states
  - fail if example XML transitions are unguarded
- `scripts/verify/workflow-contract.sh`:
  - fail if CI omits workflow-contract gate
  - fail if CI omits skills-contract gate

Recommended next:
- XML schema validation against SASS XSD/RELAX NG (when schema is published)
- per-skill semantic tests (`resume`, `retry`, `blocked`, `success`)
- migration coverage gate (warn on prompt-only legacy skills)

## Backward Compatibility

- Legacy prompt-only skills remain runnable during dual-run.
- CI remains strict for any new SASS XML artifact.
- Migrate by feature flag: enable SASS runtime per skill family after parity evidence.

## Definition of Done

- Top-10 skills each have:
  - SASS XML definitions
  - generated Mermaid state and DAG views
  - passing contract lint and workflow lint
  - explicit `wake_up`, guards, failures, and exits
