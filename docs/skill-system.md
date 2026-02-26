# Skill System Governance

This document defines operator workflows for skill discovery, install, trust review, delegation manifests, and evidence collection.

## Scoped registries and precedence

Skill registries are scoped and resolved in this order:

1. user (`config/skills.user.yaml`)
2. project (`config/skills.project.yaml`)
3. global (`config/skills.global.yaml`)

Each file must match `schemas/skill-registry.v1.schema.json`.

Use CLI discovery per scope:

```bash
cargo run -p odin-cli -- governance discover --scope project --registry config/skills.project.yaml --run-once
```

Trust values are:

- `trusted`: local/reviewed/pinned source
- `caution`: partially trusted, manual invocation preferred
- `untrusted`: install requires explicit acknowledgment

## Import risk scan and acknowledgement gate

`governance install` evaluates trust and risk findings before allowing install. The risk scanner checks script/readme content for shell/network/secret/delete indicators.

Blocked example (no acknowledgement):

```bash
cargo run -p odin-cli -- governance install --name suspicious-skill --trust-level untrusted --run-once
```

Expected result: JSON with `status: "blocked"` and `error_code: "ack_required"`.

Proceed only with explicit acceptance:

```bash
cargo run -p odin-cli -- governance install --name suspicious-skill --trust-level untrusted --ack --run-once
```

If scripts are present or secret-touching findings are detected, acknowledgement is also required.

## Capability manifest requirements for delegation

Delegated actions must include `capability-manifest.v1` (`schema_version: 1`) with:

- `plugin`
- `capabilities[]` entries with `id` and optional `scope[]`

Runtime enforcement is fail-closed:

- capability missing from manifest -> `manifest_capability_not_granted`
- requested scope not granted -> `manifest_scope_not_granted`
- plugin mismatch -> `manifest_plugin_mismatch`
- unsupported schema version -> `manifest_schema_version_unsupported`
- stagehand capability misuse/unknown -> blocked

Successful delegated execution emits governance audit events:

- `governance.manifest.validated`
- `governance.capability.used`

Blocked execution emits:

- `governance.manifest.denied`

## Audit evidence required before "working" claims

Do not claim install/enable/delegation is working without command evidence.

Required evidence bundle:

1. exact command used
2. JSON summary output (`status`, `error_code`, checks)
3. for delegated runtime actions, audit event evidence from sink/logs

Minimum claim mapping:

- "install blocked without ack" -> JSON shows `command: install`, `status: blocked`, `error_code: ack_required`
- "stagehand enable blocked" -> JSON shows `command: enable-plugin`, `status: blocked`
- "manifest denied" -> runtime result `ActionStatus::Blocked` and `governance.manifest.denied`
- "manifest allowed and executed" -> runtime result `ActionStatus::Executed` and both validation/usage events

Use `bash scripts/verify/skill-plugin-governance-smoke.sh` as the release smoke gate for these invariants.
