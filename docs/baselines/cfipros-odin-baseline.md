# cfipros Odin Baseline Pin

Captured: 2026-02-25
Source repo path: /home/orchestrator/cfipros
Pinned commit full: 4b46d01e485e1e303ed40147d0513fa641ab51b5
Pinned commit short: 4b46d01e

## Scope

This baseline covers private Odin compatibility surfaces consumed by odin-core compat mode:

- scripts/odin/odin-service.sh
- scripts/odin/keepalive.sh
- scripts/odin/odin-inbox-write.sh
- scripts/odin/lib/task-queue.sh
- scripts/odin/lib/agent-lifecycle.sh
- scripts/odin/lib/adapters/*
- scripts/odin/lib/backend-state.sh
- scripts/odin/lib/orchestrator-failover.sh
- scripts/odin/lib/telegram.sh
- scripts/odin/lib/browser-access.sh

## Invariants Locked

- Keepalive failover/cooldown/anti-flap behavior.
- Backend state/routing transitions.
- Inbox write path used by compat task ingress.
- Service launcher behavior and spend ledger checks.

## Verification Entry Point

Run from odin-core:

bash scripts/verify/compat-regression.sh --legacy-root /home/orchestrator/cfipros

Expected result: all checks pass, exit code 0.
