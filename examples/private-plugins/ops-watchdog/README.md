# private.ops-watchdog

Private plugin scaffold for extracting `keepalive.sh` external monitoring automations from core.

## Purpose

Moves these behaviors out of core and into downstream private plugin space:
- unresolved external issue polling (legacy Sentry check)
- PR merge-state polling and branch-update task enqueueing
- external notification dispatch

## Install (local path)

```bash
odin plugin install ./examples/private-plugins/ops-watchdog
```

## Configure

Copy `config/config.example.yaml` into downstream private config and provide secret handles.

## Legacy keepalive feature-gate cutover

To move polling from legacy keepalive blocks to plugin tasks:

```bash
export KEEPALIVE_ENABLE_LEGACY_SENTRY=0
export KEEPALIVE_ENABLE_WATCHDOG_SENTRY_POLL=1

export KEEPALIVE_ENABLE_LEGACY_PR_HEALTH=0
export KEEPALIVE_ENABLE_WATCHDOG_PR_HEALTH_POLL=1
```

Optional routing metadata:

```bash
export KEEPALIVE_WATCHDOG_PROJECT=private
export KEEPALIVE_WATCHDOG_PLUGIN=private.ops-watchdog
```

## Runtime contract

- Entrypoint: `./bin/plugin serve`
- Reads event envelopes from stdin
- Emits capability/action requests to stdout as JSON

## Notes

- This scaffold intentionally includes placeholders only.
- No private endpoints, tokens, or hostnames are included.
