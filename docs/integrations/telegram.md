# Telegram adapter (optional)

Telegram is an optional gateway adapter. The required baseline remains the CLI-only flow in [`docs/quickstart.md`](../quickstart.md).

## Prerequisites

- CLI quickstart has already passed.
- Telegram credentials are only needed if you later run non-dry commands.

## Minimal config (safe dry-run)

```bash
export ODIN_GUARDRAILS_PATH=/tmp/odin-missing-guardrails.yaml
```

## Copy-paste command flow

```bash
scripts/odin/odin gateway add telegram --dry-run
```

## Verification checks

```bash
scripts/odin/odin gateway add telegram --dry-run | grep -F "DRY-RUN gateway add source=telegram"
```

## Common failure + smallest fix

```bash
scripts/odin/odin gateway add telegram
# [odin] ERROR: BLOCKED gateway add: guardrails file not found ...

scripts/odin/odin gateway add telegram --dry-run
```

## Optional: run non-dry with explicit acknowledgement

```bash
cat > /tmp/odin-telegram-guardrails.yaml <<'YAML'
denylist: []
confirm_required:
  - integration
YAML

export ODIN_GUARDRAILS_PATH=/tmp/odin-telegram-guardrails.yaml
export ODIN_GUARDRAILS_ACK=yes
scripts/odin/odin gateway add telegram --confirm
```
