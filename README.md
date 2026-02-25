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

## Bootstrap wrapper contract (minimal)

```bash
scripts/odin/odin help
scripts/odin/odin connect claude oauth --dry-run
scripts/odin/odin start --dry-run
scripts/odin/odin tui --dry-run
scripts/odin/odin inbox add "test task" --dry-run
scripts/odin/odin inbox list
scripts/odin/odin gateway add cli --dry-run
scripts/odin/odin verify --dry-run
```

Conservative default: if `config/guardrails.yaml` is missing, mutating commands are blocked unless `--dry-run` is used.

## Bootstrap docs (executable)

- CLI quickstart: `docs/quickstart.md`
- n8n adapter (optional): `docs/integrations/n8n.md`
- Slack adapter (optional): `docs/integrations/slack.md`
- Telegram adapter (optional): `docs/integrations/telegram.md`

Smoke check for documented commands:

```bash
bash scripts/verify/docs-command-smoke.sh
```

## TUI Dashboard

Install dependency once:

```bash
python3 -m pip install rich
```

Run from core repo:

```bash
python3 scripts/odin/odin-tui.py --live
```

Wrapper command:

```bash
bash scripts/odin/odin-tui --live
```

Profile selection:

```bash
# core profile is default
python3 scripts/odin/odin-tui.py --profile core --live

# legacy profile keeps previous monolithic dashboard behavior
python3 scripts/odin/odin-tui.py --profile legacy --live
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

## Verification Gates

```bash
bash scripts/verify/compat-regression.sh --legacy-root /home/orchestrator/cfipros
bash scripts/verify/quickstart-smoke.sh
bash scripts/verify/plugin-install-matrix.sh
```

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
