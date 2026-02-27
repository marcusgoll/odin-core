# odin-core

Vendor-agnostic, policy-driven orchestrator core with plugin-first extensibility.

Odin is a self-hosted orchestration engine that routes tasks through a policy engine, executes them via plugins, and logs every decision for audit. It replaces ad-hoc shell scripts with a structured runtime that enforces capability-based security, supports both native Rust plugins and legacy Bash compatibility, and exposes a TUI dashboard for monitoring.

## Prerequisites

- **Rust** stable toolchain (1.75+) via [rustup](https://rustup.rs)
- **Docker** and Docker Compose (for containerized deployment)
- **Python 3.10+** with `rich` (`pip install rich`) for the TUI dashboard
- **jq** (used by verification scripts)

## Quickstart (recommended)

```bash
cp .env.example .env
docker compose up -d
```

For a CLI-only walkthrough without Docker, see `docs/quickstart.md`.

## Local dev

```bash
cargo run -p odin-cli -- --config config/default.yaml --run-once
```

Omit `--run-once` to keep the runtime running (enters a 60-second poll loop).

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
bash scripts/verify/quickstart-smoke.sh
bash scripts/verify/plugin-install-matrix.sh
bash scripts/verify/workflow-contract.sh
bash scripts/verify/tui-core-smoke.sh
bash scripts/verify/bootstrap-wrapper-smoke.sh
bash scripts/verify/guardrails-gate-smoke.sh
bash scripts/verify/mode-confidence-smoke.sh
bash scripts/verify/skills-contract.sh
bash scripts/verify/docs-command-smoke.sh
bash scripts/verify/compat-regression.sh --legacy-root /path/to/legacy-repo
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
