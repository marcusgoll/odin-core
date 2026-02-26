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

## SASS skills (strict mode)

SASS v0.1 is the state-aware skill system used by Odin. Strict mode requires `wake_up`,
explicit end states, guarded decision branches, and least-privilege permissions.

Read the full contract and migration guide:

- `docs/skill-system.md`

Common commands:

```bash
cargo run -p odin-cli -- skill validate examples/skills/sass/v0.1/run_tests.skill.xml
cargo run -p odin-cli -- skill mermaid examples/skills/sass/v0.1/run_tests.skill.xml
bash scripts/verify/sass-skill-governance-smoke.sh
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
