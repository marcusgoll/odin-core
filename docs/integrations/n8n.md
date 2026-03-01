# n8n integration (optional)

Use n8n only as an optional command runner around the CLI-first flow. The core bootstrap path works without n8n, Slack, or Telegram.

## Prerequisites

- Complete [`docs/quickstart.md`](../quickstart.md) first.
- n8n can execute local shell commands on the same machine as this repo.

## Minimal config (safe dry-run)

```bash
export ODIN_GUARDRAILS_PATH=/tmp/odin-missing-guardrails.yaml
```

## Copy-paste command flow

```bash
scripts/odin/odin gateway add cli --dry-run
scripts/odin/odin inbox add "n8n bootstrap task" --dry-run
scripts/odin/odin inbox list
```

In n8n, map these commands into `Execute Command` steps.

## Verification checks

```bash
scripts/odin/odin gateway add cli --dry-run | grep -F "DRY-RUN gateway add source=cli"
scripts/odin/odin inbox add "n8n bootstrap task" --dry-run | grep -F "DRY-RUN inbox add"
```

## Common failure + smallest fix

```bash
scripts/odin/odin gateway add cli
# [odin] ERROR: BLOCKED gateway add: guardrails file not found ...

scripts/odin/odin gateway add cli --dry-run
```

## Optional: run non-dry with local guardrails

```bash
cat > /tmp/odin-n8n-guardrails.yaml <<'YAML'
denylist: []
confirm_required:
  - integration
YAML

export ODIN_GUARDRAILS_PATH=/tmp/odin-n8n-guardrails.yaml
export ODIN_GUARDRAILS_ACK=yes
scripts/odin/odin gateway add cli --confirm
```
