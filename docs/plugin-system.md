# Plugin System Contract (v1)

## Install sources

- local path
- git ref (`<repo>#<ref>`)
- artifact URL/file (`.tar.gz`/`.tgz` or unpacked directory)

## Verification pipeline

1. Resolve source
2. Verify SHA256 checksum (required)
3. Verify signature if required by policy or marketplace mode
4. Validate manifest against `schemas/plugin-manifest.v1.schema.json`
5. Validate `plugin.compatibility.core_version` against running core
6. Persist audit event and register plugin

## Signature methods

- `none`: never valid when signature verification is required
- `minisign`: verifies detached signature against manifest using `minisign`
- `sigstore`: verifies detached signature using `cosign verify-blob`

When `InstallRequest.require_signature=true` or manifest `signing.required=true`, missing/invalid signature is a hard install failure.

## Permission model

- Default deny
- Capabilities must be declared in manifest and granted by policy
- Risk tiers: `safe`, `sensitive`, `destructive`
- Destructive actions always require explicit approval

## Runtime isolation

- Plugins run out-of-process
- Requests are capability-token scoped per action
- No direct secrets, only handle references

## Governance overlays

- Skill installs and plugin enablement are governed by scoped registries and trust levels (`global`, `project`, `user`).
- Untrusted/script-bearing skill installs are blocked until explicit acknowledgement is supplied.
- Stagehand plugin enablement is blocked without explicit domain and workspace allowlists.

See:

- `docs/skill-system.md` for trust and install gating workflows
- `docs/stagehand-safety.md` for Stagehand-specific safety defaults

## Delegation capability manifests

- Delegated runtime actions must include a capability manifest (`schemas/capability-manifest.v1.schema.json`).
- Runtime fails closed when a requested capability/scope is not granted by manifest.
- Manifest validation and usage decisions are auditable via governance events:
  - `governance.manifest.denied`
  - `governance.manifest.validated`
  - `governance.capability.used`
