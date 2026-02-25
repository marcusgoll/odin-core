# Phase 2 Carve-out: Keepalive Private Integrations

## Goal

Extract private monitoring and automation logic from legacy keepalive core loop into downstream private plugins/policy.

## Source mapping (legacy)

Legacy file:
- `/home/orchestrator/cfipros/scripts/odin/keepalive.sh`

Private integration boundaries to extract:
- Sentry unresolved issue polling and critical issue task creation:
  - `_check_sentry` block around lines 595-695
- PR merge-state polling and automated branch-update task enqueue:
  - `_check_pr_health` block around lines 700-791

Core-safe logic to keep in compat core runtime:
- process liveness checks
- disk/inbox threshold checks
- generic self-heal dispatch plumbing (without provider-specific integrations)

## Implemented feature gates in legacy keepalive

Environment flags (default behavior preserved):
- `KEEPALIVE_ENABLE_LEGACY_SENTRY=1`
- `KEEPALIVE_ENABLE_LEGACY_PR_HEALTH=1`
- `KEEPALIVE_ENABLE_WATCHDOG_SENTRY_POLL=0`
- `KEEPALIVE_ENABLE_WATCHDOG_PR_HEALTH_POLL=0`
- `KEEPALIVE_WATCHDOG_PROJECT=private`
- `KEEPALIVE_WATCHDOG_PLUGIN=private.ops-watchdog`

Plugin poll task types enqueued when legacy blocks are disabled:
- `watchdog.sentry.poll`
- `watchdog.pr_health.poll`

## New private plugin scaffold

- `examples/private-plugins/ops-watchdog/odin.plugin.yaml`
- `examples/private-plugins/ops-watchdog/bin/plugin`
- `examples/private-plugins/ops-watchdog/config/config.example.yaml`
- `policy/private-ops-watchdog.example.yaml`

## Task contract migration

Replace direct keepalive dispatch for provider-specific checks with plugin task types:
- `watchdog.sentry.poll`
- `watchdog.pr_health.poll`

Plugin emits capability/action requests and follow-up task enqueue requests via core policy path.

## Runtime bridge status (implemented)

- `crates/odin-core-runtime` now supports:
  - parsing `watchdog_poll` task envelopes from compat inbox payloads
  - dispatching `task.received` events into plugin entrypoints (out-of-process)
  - routing plugin directives:
    - `request_capability` -> policy + executor path
    - `enqueue_task` -> policy-gated ingress write path (`task.enqueue`)
    - `noop` -> audit-only
- `bin/odin-cli` now supports:
  - `--task-file <json>` to execute one watchdog task envelope
  - `--plugins-root <dir>` for plugin discovery
  - compatibility ingress routing via `BashTaskIngressAdapter` when `--legacy-root` is provided

## Rollout steps

1. Keep current keepalive behavior enabled in compat mode (defaults).
2. Deploy private plugin with policy grants and secret handles.
3. Canary: disable one legacy block and enable corresponding plugin poll flag.
4. Compare task/output parity for at least one full polling window.
5. Disable remaining legacy provider block and keep rollback env switches documented.

## Success checks

- Plugin-generated tasks match legacy volume/type for one full canary window.
- No private endpoints/tokens appear in OSS core configs.
- All plugin actions pass through policy and audit paths.
