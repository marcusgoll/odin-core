# Huginn Plugin Replacement Design

**Date:** 2026-03-10

## Goal

Replace the remaining Stagehand-specific surface in `odin-core` with Huginn so core policy, runtime wiring, plugin packaging, and docs match the browser system that Odin actually runs in production.

## Current State

- `odin-orchestrator` already routes live browser work through Huginn wrappers and `odin-huginn-server.js`.
- `odin-core` still models browser automation as a Stagehand plugin:
  - plugin package and manifest live under `plugins/stagehand`
  - governance policy is keyed to `plugin: "stagehand"`
  - runtime capability checks interpret `stagehand.*` actions
  - CLI enablement and docs instruct operators to enable Stagehand
- The current Stagehand plugin exposes AI-native capabilities (`extract`, `observe`, `agent`) that do not map cleanly to Huginn’s ref/snapshot HTTP API.

## Approaches Considered

### 1. Keep Stagehand names, swap only backend

Pros:
- minimal rename churn
- lowest short-term migration cost

Cons:
- leaves core lying about what backend it is using
- preserves Stagehand-specific capability names and docs
- keeps operator and governance semantics misaligned with live Odin

### 2. Full rename plus direct Huginn HTTP wrapper

Pros:
- aligns `odin-core` with live Odin architecture
- removes Stagehand dependency and terminology from core
- keeps browser automation backend explicit and inspectable

Cons:
- requires contract changes for capabilities that were previously AI-native
- touches runtime, governance, docs, tests, and plugin packaging

### 3. Shell out from `odin-core` into orchestrator bash wrappers

Pros:
- reuses existing Huginn wrapper behavior

Cons:
- couples `odin-core` to orchestrator shell libraries and path layout
- harder to test deterministically inside `odin-core`
- poor fit for the current external-process plugin model

## Decision

Choose approach 2.

`odin-core` will replace the Stagehand plugin with a Huginn plugin that talks to Huginn’s HTTP server directly. The plugin name, governance surface, docs, and tests will move from `stagehand` to `huginn`.

Generic `browser.*` capability ids remain where they still make sense as cross-plugin browser primitives. Huginn-specific capability ids become `huginn.*`.

## Capability Contract

### Keep

- `browser.navigate`
- `browser.observe`
- `workspace.read`
- `command.run`

These remain policy-governed primitives and map naturally onto Huginn behavior.

### Rename

- `stagehand.observe_url` -> `huginn.observe_url`
- `stagehand.observe_domain` -> `huginn.observe_domain`
- `stagehand.workspace.read` -> `huginn.workspace.read`
- `stagehand.command.run` -> `huginn.command.run`
- `stagehand.login` -> `huginn.login`
- `stagehand.payment` -> `huginn.payment`
- `stagehand.pii_submit` -> `huginn.pii_submit`
- `stagehand.file_upload` -> `huginn.file_upload`
- `stagehand.enabled` -> `huginn.enabled`

### Remove From The Plugin Surface

- `browser.extract`
- `browser.agent`

Reason: these are Stagehand AI behaviors, not Huginn primitives. Keeping them would imply semantics Huginn does not provide.

### Add Huginn-Native Actions

- `huginn.snapshot`
- `huginn.click`
- `huginn.type`
- `huginn.press`
- `huginn.hover`
- `huginn.back`
- `huginn.forward`
- `huginn.reload`
- `huginn.screenshot`
- `huginn.cookies.get`
- `huginn.cookies.set`

## Plugin Architecture

Create a Huginn plugin under `plugins/huginn`:

- external Node process, same stdin/stdout plugin protocol
- config-driven connection to Huginn HTTP server
- supports:
  - connect to existing Huginn server via base URL + bearer token
  - optional auto-launch of a local browser session through Huginn `/launch`
- capability execution becomes HTTP calls to Huginn endpoints instead of direct Stagehand SDK calls

The plugin should not shell out to orchestrator bash libraries.

## Input Model

Because Huginn is ref-driven, browser actions need explicit input shapes:

- navigate: `{ url }`
- observe/snapshot: `{ interactive?, compact? }`
- click/hover: `{ ref }`
- type: `{ ref, text, submit? }`
- press: `{ key }`
- screenshot: `{ ref?, fullPage?, outputPath? }`
- cookie set: `{ cookies }` or `{ cookieFile }`

The plugin will treat missing required fields as execution failures, not try to infer AI behavior.

## Governance Model

Rename the Stagehand policy model to Huginn policy:

- registry key becomes `huginn`
- enable command becomes `governance enable-plugin --plugin huginn`
- default mode remains deny-by-default
- destructive actions remain denied by default:
  - login
  - payment
  - PII submit
  - file upload

Domain, workspace, and command boundaries stay intact. The safety model carries over; only the plugin identity and capability ids change.

## Backward Compatibility

This replacement is intentionally not fully backward compatible.

- Old `stagehand.*` capability ids will be removed from primary code paths.
- Generic `browser.navigate` and `browser.observe` remain.
- Existing docs/tests/config using `stagehand` must be migrated.

This is acceptable because the stated goal is replacement, not alias preservation.

## Testing Strategy

### Rust

- update governance and runtime unit tests from `stagehand` to `huginn`
- verify capability manifest enforcement for the renamed plugin/capabilities
- verify CLI `enable-plugin --plugin huginn`

### Node plugin

- update protocol/config tests for the new Huginn plugin package
- add unit tests for Huginn HTTP client request shaping
- keep tests deterministic by mocking HTTP rather than requiring a live browser

### End-to-end

- build the Huginn plugin package
- run `cargo test`
- run targeted plugin tests in `plugins/huginn`

## Risks

- Capability semantics narrow when moving away from Stagehand AI flows.
- Docs/tests may still carry Stagehand terminology after code compiles.
- If any external manifests rely on `plugin: "stagehand"`, they will need migration after this change lands.

## Out of Scope

- migrating orchestrator browser wrappers; they already use Huginn
- adding new AI extraction semantics on top of Huginn snapshots
- preserving Stagehand package compatibility aliases forever
