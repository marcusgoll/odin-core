# Release Readiness (v0.1 seed)

## CI Gate Mapping

Required gates and current workflow coverage:

- Lint: `.github/workflows/ci.yml` -> `cargo fmt --check`, `cargo clippy`
- Unit/integration tests: `.github/workflows/ci.yml` -> `cargo test --workspace`, `integration-dry-run`
- Secret scanning: `.github/workflows/ci.yml` -> `gitleaks`
- Dependency scanning: `.github/workflows/ci.yml` -> `cargo audit`
- Release artifacts + checksums + SBOM: `.github/workflows/release.yml`

## Local Release Gate Commands

Run before tagging/pushing release:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/verify/compat-regression.sh --legacy-root /home/orchestrator/cfipros
bash scripts/verify/quickstart-smoke.sh
bash scripts/verify/tui-core-smoke.sh
bash scripts/verify/plugin-install-matrix.sh
bash scripts/verify/skill-plugin-governance-smoke.sh
```

## Quickstart Gate

- One-path quickstart validated via `scripts/verify/quickstart-smoke.sh`.
- Includes compose config validation (when Docker is available), wrapper command contract checks (`connect/start/tui/inbox/verify` dry-run), first inbox normalization fields (`title/raw_text/source/timestamp`), CLI bootstrap, and watchdog plugin bridge smoke.

## Plugin Install Gate

- Validates install from:
  - local path
  - git ref
  - artifact archive (checksum pinned)
- Validates signed-install policy paths:
  - required signature missing -> blocked
  - minisign signature path -> accepted (tool-dependent)
  - sigstore/cosign signature path -> accepted (tool-dependent)

## Skill + Plugin Governance Smoke Gate

- `scripts/verify/skill-plugin-governance-smoke.sh` is required for release readiness.
- Verifies:
  - untrusted skill install without ack is blocked
  - stagehand enable without domains/workspaces is blocked
  - capability missing from manifest is blocked
  - manifest-granted capability executes

## Governance Evidence Requirements

Before claiming governance flows are working, release notes/evidence must include:

1. command lines used for install/enable/delegation checks
2. JSON outputs showing blocked/allowed statuses and error codes
3. audit evidence for delegation outcomes (`governance.manifest.denied|validated`, `governance.capability.used`)

## SemVer Compatibility Promise (current)

- Core follows SemVer from `0.1.0` seed onward.
- Plugin compatibility must declare `plugin.compatibility.core_version` range.
- Breaking runtime/protocol changes require explicit compatibility-note documentation before release.
