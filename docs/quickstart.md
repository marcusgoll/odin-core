# Bootstrap quickstart (CLI-only)

This path is complete with only the CLI adapter. Slack and Telegram are optional adapters that can be added later.

## Prerequisites

- Run commands from the repository root.
- Bash and `grep` are installed.
- No Slack or Telegram credentials are required for this flow.

## Minimal config (safe dry-run)

```bash
export ODIN_GUARDRAILS_PATH=/tmp/odin-missing-guardrails.yaml
export ODIN_MODE_STATE_PATH=/tmp/odin-bootstrap-state.json
```

The missing guardrails path is intentional here: it keeps risky commands no-op unless `--dry-run` is present.

## Run the bootstrap command surface

```bash
scripts/odin/odin help
scripts/odin/odin connect claude oauth --dry-run
scripts/odin/odin start --dry-run
scripts/odin/odin tui --dry-run
scripts/odin/odin inbox add "bootstrap task" --dry-run
scripts/odin/odin inbox list
scripts/odin/odin gateway add cli --dry-run
scripts/odin/odin verify --dry-run
```

## Verification checks

```bash
scripts/odin/odin connect claude oauth --dry-run | grep -F "DRY-RUN connect provider=claude auth=oauth"
scripts/odin/odin gateway add cli --dry-run | grep -F "DRY-RUN gateway add source=cli"
scripts/odin/odin verify --dry-run | grep -F "DRY-RUN verify"
bash scripts/verify/docs-command-smoke.sh
```

## Common failure + smallest fix

```bash
scripts/odin/odin start
# [odin] ERROR: BLOCKED start: guardrails file not found ...

scripts/odin/odin start --dry-run
```

## Optional: run non-dry integration/mutating commands locally

```bash
cat > /tmp/odin-guardrails-local.yaml <<'YAML'
denylist: []
confirm_required:
  - integration
YAML

export ODIN_GUARDRAILS_PATH=/tmp/odin-guardrails-local.yaml
export ODIN_GUARDRAILS_ACK=yes
export ODIN_MODE_STATE_PATH=/tmp/odin-bootstrap-state-live.json

scripts/odin/odin connect claude oauth --confirm
scripts/odin/odin start
scripts/odin/odin tui
scripts/odin/odin inbox add "bootstrap task"
scripts/odin/odin verify
```
