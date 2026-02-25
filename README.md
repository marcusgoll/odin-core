# odin-core

Vendor-agnostic, policy-driven orchestrator core with plugin-first extensibility.

## Quickstart (recommended)

```bash
cp .env.example .env
docker compose up -d
```

## Local dev

```bash
cargo run -p odin-cli -- --config config/default.yaml
```

## Watchdog task bridge (compat canary)

Process one compat `watchdog_poll` task through plugin policy/execution path:

```bash
cargo run -p odin-cli -- \
  --task-file /tmp/odin-watchdog-task.json \
  --plugins-root ./examples/private-plugins \
  --run-once
```

Use `--legacy-root /path/to/cfipros` to enqueue follow-up tasks via legacy inbox writer.

## Scope

- Core orchestration runtime
- Policy engine (capability + approval decisions)
- Plugin manager (install/verify/load)
- Secrets/session/audit interfaces
- Compatibility runtime to preserve existing private behavior during migration

## Non-Goals

- Private connectors
- Site-specific automations
- Mandatory marketplace dependency
