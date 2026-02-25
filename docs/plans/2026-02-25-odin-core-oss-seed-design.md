# Odin Core OSS Seed Design (Milestones 1+2+3)

## Goal

Prepare `odin-core` for first public push with a general-purpose dashboard, locked upgrade-safety baseline against current private Odin behavior, and release/install hardening evidence.

## Context

Current `odin-core` has a committed foundation (`e11d03d`) with runtime, policy, plugin management, compatibility adapters, docs, and CI workflows. A dashboard (TUI) exists today only in `cfipros/scripts/odin` and has now been copied into `odin-core` as uncommitted work.

## Non-Negotiable Constraints

- Preserve private Odin behavior and performance.
- No secrets or private endpoints in OSS outputs.
- Keep core plugin-first and policy-enforced.
- One branch, sequential commits, one final push.
- Public destination: `github.com/marcusgoll/odin-core`.

## Approved Execution Model

### Checkpoint A: Publish-ready seed

Deliver and commit a general-purpose TUI in `odin-core`:
- `scripts/odin/odin-tui.py`
- `scripts/odin/odin-tui`
- `README.md` run instructions

Verification:
- direct snapshot/live launch
- wrapper snapshot/live launch
- `cargo test --workspace`

### Checkpoint B: Upgrade-safety lock

Deliver and commit reproducible baseline artifacts:
- pinned `cfipros` commit for Odin scripts
- documented critical-file map and regression contract
- executable compatibility regression script

Verification:
- baseline artifact references exact commit hash
- regression script exits 0 on current environment

### Checkpoint C: Install/release hardening

Deliver and commit operational hardening artifacts:
- quickstart smoke script
- plugin install matrix script (local path, git ref, signed artifact)
- release readiness checklist with evidence pointers

Verification:
- script passes on fresh run
- install matrix passes/fails as expected for positive and negative paths
- existing CI workflows map to required gates

## Final Push Protocol

1. Re-run full verification set across A/B/C.
2. Confirm public GitHub repo exists.
3. Set remote `origin` and perform one push of `main`.
4. Confirm branch and README render.

## Risks and Mitigations

- Risk: accidental behavior drift in private runtime.
  - Mitigation: no runtime changes in `cfipros`; only read + test.
- Risk: publishing incomplete hardening evidence.
  - Mitigation: checkpoint verification gates and final full matrix.
- Risk: remote creation timing mismatch.
  - Mitigation: block push until repo exists; do not rewrite commits.

## Success Criteria

- `odin-core` contains runnable TUI and operational docs.
- Baseline pin + regression matrix are reproducible.
- Quickstart/install hardening scripts are present and verified.
- One clean public push to `marcusgoll/odin-core`.
