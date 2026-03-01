# Onboarding & Remaining Audit Fixes — Design

**Date:** 2026-02-27
**Status:** Approved

## Goal

Fix the four remaining audit findings (#54, #58, #18, #48) and add an LLM-friendly onboarding script (`setup.sh`) that guides both human users and LLM agents through first-time setup.

## Part 1: Remaining Audit Fixes

### Finding #54 — README vs docs/quickstart.md confusion
Rewrite `docs/quickstart.md` to be a unified "Getting Started" guide covering both Docker and Cargo paths. Points to `./setup.sh` as the primary automated path. README quickstart stays as the 2-line Docker shortcut with cross-references.

### Finding #58 — CI doesn't run most verification scripts
Add a `verify` job to `.github/workflows/ci.yml` running:
- `quickstart-smoke.sh`
- `plugin-install-matrix.sh`
- `tui-core-smoke.sh`
- `bootstrap-wrapper-smoke.sh`
- `docs-command-smoke.sh`

Skip scripts that need external state (`compat-regression.sh`, `guardrails-gate-smoke.sh`, `mode-confidence-smoke.sh`).

### Finding #18 — `serde_yaml` deprecation
Replace `serde_yaml = "0.9"` with `serde_yml = "0.0.12"` in workspace Cargo.toml. Update all `use serde_yaml` to `use serde_yml`. API is nearly identical.

### Finding #48 — Signature tests silently skip
Change `local_install_accepts_valid_minisign_signature_when_required` and `local_install_accepts_valid_sigstore_signature_when_required` from early-return to `#[ignore]`. CI reports them as ignored (visible) rather than false-positive passes.

## Part 2: Onboarding Script (`setup.sh`)

### Location
`/setup.sh` (repo root, executable)

### Two Modes
1. **Human interactive** — shows prompts, guides through setup
2. **LLM headless** — all choices via flags, parseable output

### Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--llm <name>` | (interactive prompt) | LLM provider: `claude`, `codex` |
| `--auth <method>` | `oauth` | Auth method: `oauth`, `api` |
| `--api-key <key>` | — | API key (only with `--auth api`) |
| `--docker-only` | — | Skip cargo build |
| `--cargo-only` | — | Skip docker build |
| `--skip-tests` | — | Skip test suite |

### Steps
1. Check prerequisites (rust, cargo, docker, python3, jq, rich)
2. Create `.env` from `.env.example` (idempotent, never overwrites)
3. Build cargo (unless `--docker-only`)
4. Run tests (unless `--skip-tests`)
5. Build docker (unless `--cargo-only`)
6. Connect LLM via `scripts/odin/odin connect <llm> <auth>`
7. Run smoke test
8. Print success + next steps

### Output Format
- Every line prefixed with `[setup]`
- Status keywords: `OK`, `MISSING`, `FAIL`, `DONE`
- Exit code 0 on success, non-zero on failure
- No color when stdout is not a TTY

### LLM-Friendliness
- All prompts have flag equivalents
- Headless mode auto-detected when all required flags are present
- Parseable, structured output
- Idempotent and non-destructive

## Part 3: `docs/quickstart.md` Rewrite

Rewrite to be a unified "Getting Started" doc:
1. Points to `./setup.sh` as primary automated path
2. Documents manual steps for Docker path
3. Documents manual steps for Cargo path
4. Covers LLM connection
5. Ends with verification

## Deliverables
1. `setup.sh` — new file, executable
2. `docs/quickstart.md` — rewritten
3. `README.md` — minor updates to point to `setup.sh`
4. `.github/workflows/ci.yml` — new `verify` job
5. `Cargo.toml` + crate sources — `serde_yaml` → `serde_yml`
6. `crates/odin-plugin-manager/src/lib.rs` — `#[ignore]` on signature tests
