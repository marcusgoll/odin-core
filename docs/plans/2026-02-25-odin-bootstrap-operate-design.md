# Odin Bootstrap + Operate UX Design

## Goal

Make Odin install/run path stupid-simple for first-time users, then progressively more assertive only after verified wins and explicit guardrails.

## Current State Summary (Evidence-Based)

- `README.md` has a short quickstart but no low-cognitive-load command contract (`ostart`, `otui`, `oin`).
- `odin-cli` currently supports argument flags only (`--config`, `--task-file`, `--run-once`, etc.) and no user-facing subcommands such as `connect/start/tui/inbox/verify`.
- `docs/quickstart.md` and `docs/integrations/{n8n,slack,telegram}.md` are missing.
- TUI exists and is usable as the cockpit.
- Verification scripts exist (`scripts/verify/*.sh`) but are optimized for maintainer workflows, not first-run user guidance.

## Constraints

- LLM/tool agnostic (Claude Code and Codex via OAuth or API key).
- CLI + TUI are primary UX.
- CLI-only path must always work without Slack/Telegram/n8n.
- No setup completion claims without verification checks.
- Guardrails must be explicit before assertive execution/delegation.

## Approaches Considered

### Approach A: Wrapper-first UX

Add a thin shell wrapper exposing `odin connect/start/tui/inbox/verify` semantics while reusing current runtime flags.

Pros:
- fastest path to usable commands
- low risk to core runtime

Cons:
- logic duplicated between wrapper and Rust CLI
- harder long-term maintainability

### Approach B: Native Rust CLI-first UX

Add first-class `odin` subcommands in `bin/odin-cli` immediately.

Pros:
- single command source of truth
- strongest maintainability

Cons:
- slower path to first user value
- larger initial surface change

### Approach C (Recommended): Phased Hybrid

Phase 1 delivers wrapper + docs + aliases quickly.  
Phase 2 moves command contract into Rust CLI with parity tests.  
Phase 3 adds confidence engine and mode gating (`BOOTSTRAP`, `OPERATE`, `RECOVERY`).

Pros:
- immediate user value with bounded change
- preserves long-term maintainability target
- enables stepwise verification

Cons:
- temporary dual path must be kept in sync

## Approved Design

### 1) Mode and confidence engine

Introduce explicit runtime mode state:
- `BOOTSTRAP`: conservative discovery/setup only
- `OPERATE`: assertive task movement/delegation
- `RECOVERY`: error-handling fallback

Confidence score starts at `10` and increases only on verified checkpoints:
- provider connected (+10)
- TUI opens (+10)
- first inbox item via gateway (+10)
- guardrails acknowledged (+10)
- one accepted item split to atomic DoD tasks (+10)
- one delegated task finished with verification (+10)

`OPERATE` gate:
- confidence >= 60
- guardrails present
- one verified end-to-end task cycle

### 2) Guardrails-first execution

Add a short persisted config (e.g., `config/guardrails.yaml`) that includes:
- workspace roots
- command allowlist + denylist
- network policy
- confirm-required action types
- project routing rules
- gateway source permissions

If guardrails are not acknowledged, Odin stays in `BOOTSTRAP` and limits to read-only planning/setup guidance.

### 3) Command contract (stable, minimal)

User-visible command contract:
- `odin connect <provider> (oauth|api)`
- `odin start`
- `odin tui`
- `odin inbox add "<task>" [--meta key=val]`
- `odin inbox list`
- `odin gateway add <cli|slack|telegram>`
- `odin verify`

Shell aliases:
- `ostart` -> `odin start`
- `otui` -> `odin tui`
- `oin "<task>"` -> `odin inbox add "<task>"`

### 4) Gateway model

All gateway payloads normalize into:
- `title`
- `raw_text`
- `source`
- `timestamp`
- `tags/meta` (optional)

CLI gateway is always available and never blocked by optional adapters.

### 5) Docs model (small + executable)

Add:
- `docs/quickstart.md`
- `docs/integrations/n8n.md`
- `docs/integrations/slack.md` (optional path)
- `docs/integrations/telegram.md` (optional path)

Each doc includes:
- prerequisites
- copy-paste steps
- verification checks
- common failure + smallest fix

## Failure Handling / Self-heal

- If provider OAuth fails, propose API path immediately (and inverse if API fails).
- If gateway adapter fails, fall back to `odin inbox add`.
- If state is inconsistent, move to `RECOVERY`, run verification checks, then return to `BOOTSTRAP`.

## Verification Strategy

A first-run user must be able to reach:
1. provider connected
2. Odin started
3. TUI opened
4. first inbox item created
5. health check passed

with no hidden dependencies on Slack/Telegram/n8n.

## Acceptance Criteria

- New user can run first inbox item flow with <= 10 copy-paste commands.
- Guardrails are captured before risky execution.
- No assertive/delegated flow when confidence < 60.
- CLI-only path fully functional independent of optional adapters.

