# Slack adapter (optional)

Slack is an optional gateway adapter. Keep the CLI-only path from [`docs/quickstart.md`](../quickstart.md) as the required baseline.

## Prerequisites

- CLI quickstart has already passed.
- Slack credentials are only needed if you later run non-dry commands.

## Minimal config (safe dry-run)

```bash
export ODIN_GUARDRAILS_PATH=/tmp/odin-missing-guardrails.yaml
```

## Copy-paste command flow

```bash
scripts/odin/odin gateway add slack --dry-run
```

## Verification checks

```bash
scripts/odin/odin gateway add slack --dry-run | grep -F "DRY-RUN gateway add source=slack"
```

## Common failure + smallest fix

```bash
scripts/odin/odin gateway add slack
# [odin] ERROR: BLOCKED gateway add: guardrails file not found ...

scripts/odin/odin gateway add slack --dry-run
```

## Optional: run non-dry with explicit acknowledgement

```bash
cat > /tmp/odin-slack-guardrails.yaml <<'YAML'
denylist: []
confirm_required:
  - integration
YAML

export ODIN_GUARDRAILS_PATH=/tmp/odin-slack-guardrails.yaml
export ODIN_GUARDRAILS_ACK=yes
scripts/odin/odin gateway add slack --confirm
```
