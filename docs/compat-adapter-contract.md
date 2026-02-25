# Compatibility Adapter Contract (Rust core <-> legacy Bash runtime)

## Goal

Preserve current private Odin behavior while progressively migrating capabilities into native Rust modules.

## Runtime modes

- `compat`: Rust delegates selected capabilities to legacy scripts.
- `native`: Rust uses native implementations.

Mode is set globally and can be overridden per capability.

## Adapter boundaries mapped to current code

### 1. Service lifecycle adapter

Legacy files:
- `/home/orchestrator/cfipros/scripts/odin/odin-service.sh`
- `/home/orchestrator/cfipros/scripts/odin/odin.service`

Contract:
- Start/stop/status operations remain behaviorally identical in `compat` mode.
- Existing state files under `/var/odin` remain canonical unless explicit migration enabled.

### 2. Task ingress and queue adapter

Legacy files:
- `/home/orchestrator/cfipros/scripts/odin/odin-inbox-write.sh`
- `/home/orchestrator/cfipros/scripts/odin/lib/task-queue.sh`
- `/home/orchestrator/cfipros/scripts/odin/odin-ssh-dispatch.sh`

Contract:
- Keep task payload validation rules and id format unchanged.
- Preserve atomic write + lock semantics.
- Preserve inbox/outbox/rejected directory contracts.

### 3. Agent lifecycle and backend routing adapter

Legacy files:
- `/home/orchestrator/cfipros/scripts/odin/lib/agent-lifecycle.sh`
- `/home/orchestrator/cfipros/scripts/odin/lib/adapters/resolve.sh`
- `/home/orchestrator/cfipros/scripts/odin/lib/adapters/claude.sh`
- `/home/orchestrator/cfipros/scripts/odin/lib/adapters/codex.sh`

Contract:
- Keep backend selection precedence and failover behavior in compat mode.
- Preserve session naming and task dispatch behavior.
- Rust normalizes events but must not alter legacy routing outcomes in compat mode.

### 4. Failover adapter

Legacy files:
- `/home/orchestrator/cfipros/scripts/odin/lib/backend-state.sh`
- `/home/orchestrator/cfipros/scripts/odin/lib/orchestrator-failover.sh`
- `/home/orchestrator/cfipros/scripts/odin/keepalive.sh`

Contract:
- Keep anti-flap/cooldown behavior unchanged.
- Preserve backend switch state transitions and audit events.

### 5. Approval/session adapter

Legacy files:
- `/home/orchestrator/cfipros/scripts/odin/lib/telegram.sh`
- `/home/orchestrator/cfipros/scripts/odin/lib/browser-access.sh`

Contract:
- Preserve nonce validation/replay protections.
- Preserve approval wait semantics in compat mode.
- Wrap session access behind Rust `SessionVault` interface; legacy file behavior remains available under compat flag.

### 6. Guardrail adapter

Legacy files:
- `/home/orchestrator/cfipros/scripts/odin/hooks/enforce-delegation.sh`
- `/home/orchestrator/cfipros/scripts/odin/hooks/enforce-delegation-bash.sh`

Contract:
- Hooks remain optional UX guardrails.
- Authoritative enforcement moves to Rust policy engine.
- Any policy decision conflict resolves in favor of Rust deny/approval requirement.

## Data contract invariants

Compat mode must preserve:
- `/var/odin/state.json` readable/writable contract
- inbox/outbox payload schema expectations
- routing and failover state semantics
- approval nonce semantics
- collector schema normalization for runtime files:
  - resolve agent lifecycle from `status`, fallback `state`
  - if both are missing, infer `busy` when dispatch/current task exists; otherwise `unknown`
  - any `/var/odin/*` collector key mapping change must include a fixture test for alias handling

## Observability contract

- Legacy logs continue to be written in compat mode.
- Rust audit stream receives normalized mirror events with correlation IDs.
- Secret redaction is enforced at Rust sink boundary.

## Exit criteria per adapter

Adapter can move from compat->native only when:
1. Golden regression tests pass against baseline behavior.
2. Audit event parity checks pass.
3. Rollback switch exists and is tested.
