# Stagehand Safety Envelope

Stagehand is treated as high-risk browser automation and is deny-by-default.

## Default policy

Without an explicit permission envelope:

- plugin is disabled
- mode is `read/observe`
- domain allowlist is empty
- workspace allowlist is empty
- command allowlist is empty

These actions are always denied by default policy:

- login (`action_login_disallowed`)
- payment (`action_payment_disallowed`)
- PII submit (`action_pii_submit_disallowed`)
- file upload (`action_file_upload_disallowed`)

## Required envelope for enablement

Enablement requires a `PluginPermissionEnvelope` for `plugin: stagehand` with trusted/caution trust and explicit capabilities:

- `stagehand.enabled`
- `browser.observe` (or `stagehand.observe_url` / `stagehand.observe_domain`) with domain scope
- `workspace.read` (or `stagehand.workspace.read`) with workspace scope
- optional `command.run` (or `stagehand.command.run`) with explicit command scope

`untrusted` envelopes cannot enable stagehand even if `stagehand.enabled` is present.

## Operator workflow

Blocked example (missing domains/workspaces):

```bash
cargo run -p odin-cli -- governance enable-plugin --plugin stagehand --run-once
```

Expected result: `status: "blocked"`, `error_code: "policy_requirements_missing"`, reasons include `domains_required` and `workspaces_required`.

Allowed example:

```bash
cargo run -p odin-cli -- governance enable-plugin --plugin stagehand --domains example.com --workspaces /tmp --run-once
```

Expected result: `status: "ok"` with all policy checks `decision: "allow"`.

## Safety boundaries

Even when enabled, policy remains constrained:

- URL/domain access denied unless host is allowlisted
- workspace reads denied outside allowlisted paths
- command execution denied unless command is allowlisted and path arguments remain within allowlisted workspaces
- unsafe shell syntax and traversal patterns are denied fail-closed

## Evidence requirements

Before claiming stagehand is safely enabled, record:

1. enable command and JSON output
2. domain/workspace/command check decisions
3. matching policy deny/allow reasons for the tested scope

For release verification, run `bash scripts/verify/skill-plugin-governance-smoke.sh`.
