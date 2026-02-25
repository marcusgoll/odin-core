# Migration Checklist (Mapped to Current Files)

## Phase 0: Baseline inventory and lock

- [x] Pin baseline commit in `cfipros` for `scripts/odin`.
- [x] Snapshot behavior contracts and test baselines.
- [x] Capture current critical file map:
  - `/home/orchestrator/cfipros/scripts/odin/odin-service.sh`
  - `/home/orchestrator/cfipros/scripts/odin/keepalive.sh`
  - `/home/orchestrator/cfipros/scripts/odin/odin-inbox-write.sh`
  - `/home/orchestrator/cfipros/scripts/odin/lib/task-queue.sh`
  - `/home/orchestrator/cfipros/scripts/odin/lib/agent-lifecycle.sh`
  - `/home/orchestrator/cfipros/scripts/odin/lib/adapters/*`
  - `/home/orchestrator/cfipros/scripts/odin/lib/backend-state.sh`
  - `/home/orchestrator/cfipros/scripts/odin/lib/orchestrator-failover.sh`
  - `/home/orchestrator/cfipros/scripts/odin/lib/telegram.sh`
  - `/home/orchestrator/cfipros/scripts/odin/lib/browser-access.sh`

Baseline artifacts:
- `/home/orchestrator/odin-core/docs/baselines/cfipros-odin-baseline.md`
- `/home/orchestrator/odin-core/docs/baselines/compat-regression-matrix.md`
- `/home/orchestrator/odin-core/scripts/verify/compat-regression.sh`

## Phase 1: Interface extraction

- [x] Define plugin manifest schema (`schemas/plugin-manifest.v1.schema.json`).
- [x] Implement Rust trait contracts:
  - [x] runtime orchestrator
  - [x] policy decision engine
  - [x] plugin installer/loader
  - [x] secret/session stores
  - [x] audit sink
- [x] Add compat adapter shims in `crates/odin-compat-bash`.

## Phase 2: Private customization carve-out

- [x] Move project-specific automations from keepalive into private plugin package.
- [x] Move private approvals/risk rules into private policy pack.
- [x] Keep `runtime.mode=compat` and verify no behavior regressions.
- [x] Use `docs/phase2-keepalive-carveout.md` mapping to extract provider-specific sections:
  - [x] legacy Sentry poll/task dispatch block
  - [x] legacy PR health poll/auto-update block
- [x] Bootstrap downstream private plugin from:
  - [x] `examples/private-plugins/ops-watchdog/odin.plugin.yaml`
  - [x] `examples/private-plugins/ops-watchdog/bin/plugin`
  - [x] `examples/private-plugins/ops-watchdog/config/config.example.yaml`
- [x] Apply private policy grants from:
  - [x] `policy/private-ops-watchdog.example.yaml`

## Phase 3: OSS quickstart and demo plugin

- [x] One-command quickstart validated on fresh machine.
- [x] Demo plugin installs from local path and git ref.
- [x] Signed artifact install path tested with checksum + signature failure cases.

## Phase 4: Release readiness

- [x] CI gates active for lint/test/integration/secret scan/dependency scan.
- [x] Release artifacts include checksums and SBOM.
- [x] SemVer compatibility notes published.

## Regression matrix (must stay green)

Use current script tests as baseline references:
- `/home/orchestrator/cfipros/scripts/odin/tests/backend-state-test.sh`
- `/home/orchestrator/cfipros/scripts/odin/tests/backend-switch-events-test.sh`
- `/home/orchestrator/cfipros/scripts/odin/tests/keepalive-failover-test.sh`
- `/home/orchestrator/cfipros/scripts/odin/tests/keepalive-cooldown-test.sh`
- `/home/orchestrator/cfipros/scripts/odin/tests/keepalive-antiflap-test.sh`
- `/home/orchestrator/cfipros/scripts/odin/tests/odin-service-launcher-test.sh`
- `/home/orchestrator/cfipros/scripts/odin/tests/spend-ledger-test.sh`

## Rollback checklist

- [ ] Confirm previous release bundle includes pinned core + plugins.lock + policy.lock.
- [ ] Validate rollback command in staging.
- [ ] Validate compat fallback toggles restore prior behavior.
