# Odin Core Foundation Spec (v0.1 draft)

## Objective

Deliver a hardened, plugin-first `odin-core` that is simple to install, secure by default, and safe to upgrade while preserving existing private behavior through compatibility runtime mode.

## Architecture

Core modules:
- `odin-core-runtime`
- `odin-policy-engine`
- `odin-plugin-manager`
- `odin-plugin-protocol`
- `odin-secrets`
- `odin-audit`
- `odin-compat-bash`

Out-of-scope for core:
- private connectors
- private automations
- private policy defaults

## Plugin system

- Manifest schema: `schemas/plugin-manifest.v1.schema.json`
- Default deny capability model
- Risk tiers: safe/sensitive/destructive
- Install sources: local path, git ref, artifact
- Verification: checksum required; signature/provenance policy-dependent
- Runtime: out-of-process only

## Security baseline

- Secrets/session interfaces return handles, not plaintext values.
- Destructive actions require explicit approvals.
- Audit stream captures policy decisions and action outcomes.
- CI includes secret and dependency scanning.

## Install model

- One-path quickstart using Docker Compose.
- Config layering: default -> profile -> env -> CLI.
- Profiles: dev/homelab/vps.
- Marketplace is optional and disabled by default.

## Upgrade model

- Upstream core + downstream private plugins/policy pack.
- SemVer with compatibility checks at plugin load/install.
- Canary-first rollout and deterministic rollback bundles.
- Dual runtime switches allow per-capability migration and rollback.

## Migration execution

See `docs/migration-checklist.md` and `docs/compat-adapter-contract.md`.

## Acceptance criteria

- Fresh install path runs in under 15 minutes.
- No secrets in logs or manifest payloads.
- Private behavior preserved in compat mode on pinned baseline.
- Destructive plugin actions blocked without approval.
- Plugin install works for local path, git ref, and signed artifact option.
- CI green on pull requests and release tags.
