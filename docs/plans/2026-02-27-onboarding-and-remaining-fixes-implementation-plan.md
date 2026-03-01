# Onboarding & Remaining Audit Fixes — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the four remaining audit findings (#54, #58, #18, #48) and add an LLM-friendly onboarding script (`setup.sh`).

**Architecture:** Four independent code fixes (serde_yml migration, signature test #[ignore], CI verify job, quickstart rewrite) plus a new `setup.sh` Bash script that automates first-time setup with both interactive and headless (LLM-friendly) modes. The script delegates LLM connection to the existing `scripts/odin/odin connect` subcommand.

**Tech Stack:** Rust (Cargo workspace), Bash, GitHub Actions YAML, Markdown

---

### Task 1: Migrate `serde_yaml` to `serde_yml` (Finding #18)

**Files:**
- Modify: `Cargo.toml:23` (workspace root)
- Modify: `crates/odin-plugin-manager/Cargo.toml:10`
- Modify: `crates/odin-core-runtime/Cargo.toml:10`
- Modify: `crates/odin-plugin-manager/src/lib.rs:376,479-483`
- Modify: `crates/odin-core-runtime/src/lib.rs:183`

**Step 1: Update workspace Cargo.toml**

Replace line 23:
```toml
# old
serde_yaml = "0.9"
# new
serde_yml = "0.0.12"
```

**Step 2: Update crate Cargo.toml files**

In `crates/odin-plugin-manager/Cargo.toml` line 10:
```toml
# old
serde_yaml.workspace = true
# new
serde_yml.workspace = true
```

In `crates/odin-core-runtime/Cargo.toml` line 10:
```toml
# old
serde_yaml.workspace = true
# new
serde_yml.workspace = true
```

**Step 3: Update Rust source — plugin-manager**

In `crates/odin-plugin-manager/src/lib.rs`:

Line 376 — change `serde_yaml::from_str` to `serde_yml::from_str`:
```rust
serde_yml::from_str::<PluginManifest>(&raw)
    .map_err(|e| PluginManagerError::ManifestParse(e.to_string()))
```

Lines 479-483 — change three `serde_yaml::to_string` calls to `serde_yml::to_string`:
```rust
serde_yml::to_string(method).expect("method yaml").trim(),
serde_yml::to_string(signature).expect("signature yaml").trim(),
serde_yml::to_string(certificate)
    .expect("certificate yaml")
    .trim(),
```

**Step 4: Update Rust source — core-runtime**

In `crates/odin-core-runtime/src/lib.rs` line 183:
```rust
serde_yml::from_str::<PluginManifest>(&raw)
    .map_err(|e| RuntimeError::Plugin(format!("manifest parse failed: {e}")))
```

**Step 5: Build and test**

Run: `cargo build --workspace 2>&1`
Expected: compiles cleanly (no warnings about serde_yaml)

Run: `cargo test --workspace 2>&1`
Expected: all 35 tests pass

**Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock \
  crates/odin-plugin-manager/Cargo.toml crates/odin-plugin-manager/src/lib.rs \
  crates/odin-core-runtime/Cargo.toml crates/odin-core-runtime/src/lib.rs
git commit -m "fix: migrate serde_yaml to serde_yml (finding #18)"
```

---

### Task 2: Change signature tests to `#[ignore]` (Finding #48)

**Files:**
- Modify: `crates/odin-plugin-manager/src/lib.rs:630-693`

**Step 1: Update minisign test (line 630)**

Add `#[ignore]` attribute and remove the early-return guard:
```rust
#[test]
#[ignore] // requires minisign CLI tool
fn local_install_accepts_valid_minisign_signature_when_required() {
    // Remove these lines:
    // if !command_available("minisign") {
    //     eprintln!("skipping minisign test; tool not installed");
    //     return;
    // }

    let root = temp_dir("local-minisign-ok");
    // ... rest unchanged
```

**Step 2: Update sigstore test (line 688)**

Add `#[ignore]` attribute and remove the early-return guard:
```rust
#[test]
#[ignore] // requires cosign CLI tool
fn local_install_accepts_valid_sigstore_signature_when_required() {
    // Remove these lines:
    // if !command_available("cosign") {
    //     eprintln!("skipping cosign test; tool not installed");
    //     return;
    // }

    let root = temp_dir("local-sigstore-ok");
    // ... rest unchanged
```

**Step 3: Run tests to verify**

Run: `cargo test --workspace 2>&1`
Expected: all non-ignored tests pass; 2 tests show as "ignored" in output

Run: `cargo test --workspace -- --ignored 2>&1`
Expected: the 2 signature tests attempt to run (and fail if tools aren't installed, which is expected)

**Step 4: Commit**

```bash
git add crates/odin-plugin-manager/src/lib.rs
git commit -m "fix: mark signature tests as #[ignore] instead of silent skip (finding #48)"
```

---

### Task 3: Add `verify` job to CI (Finding #58)

**Files:**
- Modify: `.github/workflows/ci.yml`

**Step 1: Add verify job**

Append after the `dependency-scan` job (after line 61):
```yaml

  verify:
    runs-on: ubuntu-latest
    needs: lint-test
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Install jq
        run: sudo apt-get update && sudo apt-get install -y jq
      - name: Install Python rich
        run: pip install rich
      - name: Build workspace
        run: cargo build --workspace
      - name: Quickstart smoke
        run: bash scripts/verify/quickstart-smoke.sh
      - name: Plugin install matrix
        run: bash scripts/verify/plugin-install-matrix.sh
      - name: TUI core smoke
        run: bash scripts/verify/tui-core-smoke.sh
      - name: Bootstrap wrapper smoke
        run: bash scripts/verify/bootstrap-wrapper-smoke.sh
      - name: Docs command smoke
        run: bash scripts/verify/docs-command-smoke.sh
```

The `needs: lint-test` ensures tests pass before running verify scripts. Scripts that need external state (`compat-regression.sh`, `guardrails-gate-smoke.sh`, `mode-confidence-smoke.sh`) are intentionally excluded per design.

**Step 2: Validate YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"`
Expected: no errors

**Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add verify job running 5 smoke scripts (finding #58)"
```

---

### Task 4: Create `setup.sh` onboarding script

**Files:**
- Create: `setup.sh` (repo root, executable)

**Step 1: Write the script**

Create `setup.sh` with the following behavior:

Flags:
- `--llm <name>` — LLM provider: `claude`, `codex` (interactive prompt if omitted)
- `--auth <method>` — Auth method: `oauth` (default), `api`
- `--api-key <key>` — API key (only with `--auth api`)
- `--docker-only` — Skip cargo build
- `--cargo-only` — Skip docker build
- `--skip-tests` — Skip test suite

Steps:
1. Check prerequisites (rust, cargo, docker, python3, jq, rich)
2. Create `.env` from `.env.example` (idempotent, never overwrites)
3. Build cargo (unless `--docker-only`)
4. Run tests (unless `--skip-tests`)
5. Build docker (unless `--cargo-only`)
6. Connect LLM via `scripts/odin/odin connect <llm> <auth>`
7. Run smoke test
8. Print success + next steps

Output format:
- Every line prefixed with `[setup]`
- Status keywords: `OK`, `MISSING`, `FAIL`, `DONE`
- Exit code 0 on success, non-zero on failure
- No color when stdout is not a TTY

LLM-friendliness:
- All prompts have flag equivalents
- Headless mode auto-detected when `--llm` flag is present
- Parseable, structured output
- Idempotent and non-destructive

```bash
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# --- Defaults ---
LLM=""
AUTH="oauth"
API_KEY=""
DOCKER_ONLY=false
CARGO_ONLY=false
SKIP_TESTS=false
HEADLESS=false

# --- Color control ---
if [ -t 1 ]; then
  BOLD="\033[1m"; GREEN="\033[32m"; RED="\033[31m"; YELLOW="\033[33m"; RESET="\033[0m"
else
  BOLD=""; GREEN=""; RED=""; YELLOW=""; RESET=""
fi

log()  { echo -e "[setup] $*"; }
ok()   { log "${GREEN}OK${RESET}   $*"; }
miss() { log "${YELLOW}MISSING${RESET} $*"; }
fail() { log "${RED}FAIL${RESET} $*"; exit 1; }
done_() { log "${GREEN}DONE${RESET} $*"; }

# --- Parse flags ---
while [[ $# -gt 0 ]]; do
  case "$1" in
    --llm)      LLM="$2"; HEADLESS=true; shift 2 ;;
    --auth)     AUTH="$2"; shift 2 ;;
    --api-key)  API_KEY="$2"; shift 2 ;;
    --docker-only) DOCKER_ONLY=true; shift ;;
    --cargo-only)  CARGO_ONLY=true; shift ;;
    --skip-tests)  SKIP_TESTS=true; shift ;;
    -h|--help)
      echo "Usage: ./setup.sh [OPTIONS]"
      echo ""
      echo "Options:"
      echo "  --llm <name>       LLM provider: claude, codex (interactive if omitted)"
      echo "  --auth <method>    Auth method: oauth (default), api"
      echo "  --api-key <key>    API key (only with --auth api)"
      echo "  --docker-only      Skip cargo build"
      echo "  --cargo-only       Skip docker build"
      echo "  --skip-tests       Skip test suite"
      echo "  -h, --help         Show this help"
      exit 0
      ;;
    *) fail "Unknown flag: $1" ;;
  esac
done

# --- Validate flags ---
if [[ "$AUTH" == "api" && -z "$API_KEY" ]]; then
  fail "--auth api requires --api-key <key>"
fi
if [[ "$DOCKER_ONLY" == true && "$CARGO_ONLY" == true ]]; then
  fail "--docker-only and --cargo-only are mutually exclusive"
fi

# --- Step 1: Check prerequisites ---
log "Checking prerequisites..."

check_cmd() {
  if command -v "$1" >/dev/null 2>&1; then
    ok "$1 found"
  else
    miss "$1 not found"
    return 1
  fi
}

prereq_fail=false

if [[ "$DOCKER_ONLY" != true ]]; then
  check_cmd rustc  || prereq_fail=true
  check_cmd cargo  || prereq_fail=true
fi
if [[ "$CARGO_ONLY" != true ]]; then
  check_cmd docker || prereq_fail=true
fi
check_cmd python3 || prereq_fail=true
check_cmd jq      || prereq_fail=true

# Check Python rich
if python3 -c "import rich" 2>/dev/null; then
  ok "python3 rich module found"
else
  miss "python3 rich module (install: pip install rich)"
  prereq_fail=true
fi

if [[ "$prereq_fail" == true ]]; then
  fail "Missing prerequisites — install them and re-run"
fi
done_ "All prerequisites satisfied"

# --- Step 2: Create .env ---
if [[ -f "$SCRIPT_DIR/.env" ]]; then
  ok ".env already exists (not overwriting)"
else
  if [[ -f "$SCRIPT_DIR/.env.example" ]]; then
    cp "$SCRIPT_DIR/.env.example" "$SCRIPT_DIR/.env"
    ok ".env created from .env.example"
  else
    miss ".env.example not found, skipping .env creation"
  fi
fi

# --- Step 3: Build cargo ---
if [[ "$DOCKER_ONLY" != true ]]; then
  log "Building workspace with cargo..."
  if (cd "$SCRIPT_DIR" && cargo build --workspace 2>&1); then
    ok "cargo build succeeded"
  else
    fail "cargo build failed"
  fi
fi

# --- Step 4: Run tests ---
if [[ "$SKIP_TESTS" != true && "$DOCKER_ONLY" != true ]]; then
  log "Running test suite..."
  if (cd "$SCRIPT_DIR" && cargo test --workspace 2>&1); then
    ok "all tests passed"
  else
    fail "tests failed"
  fi
fi

# --- Step 5: Build docker ---
if [[ "$CARGO_ONLY" != true ]]; then
  log "Building Docker image..."
  if (cd "$SCRIPT_DIR" && docker compose build 2>&1); then
    ok "docker build succeeded"
  else
    fail "docker build failed"
  fi
fi

# --- Step 6: Connect LLM ---
if [[ -z "$LLM" ]]; then
  # Interactive mode
  log "Which LLM would you like to connect?"
  echo "  1) claude (Claude Code)"
  echo "  2) codex (OpenAI Codex)"
  read -rp "[setup] Enter choice (1/2): " llm_choice
  case "$llm_choice" in
    1|claude) LLM="claude" ;;
    2|codex)  LLM="codex" ;;
    *) fail "Invalid choice: $llm_choice" ;;
  esac
fi

log "Connecting LLM: $LLM (auth: $AUTH)..."
connect_args=("$LLM" "$AUTH")
if [[ "$AUTH" == "api" ]]; then
  export ODIN_LLM_API_KEY="$API_KEY"
fi
if (cd "$SCRIPT_DIR" && bash scripts/odin/odin connect "${connect_args[@]}" --dry-run 2>&1); then
  ok "LLM connection check passed (dry-run)"
else
  fail "LLM connection check failed"
fi

# --- Step 7: Smoke test ---
log "Running smoke test..."
if (cd "$SCRIPT_DIR" && bash scripts/verify/quickstart-smoke.sh 2>&1); then
  ok "smoke test passed"
else
  fail "smoke test failed"
fi

# --- Step 8: Success ---
echo ""
done_ "odin-core is ready!"
log ""
log "Next steps:"
log "  Run the orchestrator:  cargo run -p odin-cli -- --config config/default.yaml --run-once"
log "  Start the TUI:         python3 scripts/odin/odin-tui.py --live"
log "  View help:             scripts/odin/odin help"
if [[ "$CARGO_ONLY" != true ]]; then
  log "  Docker quickstart:     docker compose up -d"
fi
```

**Step 2: Make executable**

Run: `chmod +x setup.sh`

**Step 3: Test headless mode**

Run: `bash setup.sh --llm claude --auth oauth --skip-tests --cargo-only 2>&1 | head -20`
Expected: all lines prefixed with `[setup]`, no interactive prompts

**Step 4: Commit**

```bash
git add setup.sh
git commit -m "feat: add LLM-friendly setup.sh onboarding script"
```

---

### Task 5: Rewrite `docs/quickstart.md` (Finding #54)

**Files:**
- Modify: `docs/quickstart.md`

**Step 1: Rewrite the file**

Replace entire contents with unified "Getting Started" guide:

```markdown
# Getting Started

This guide covers three ways to set up odin-core:
1. **Automated** — `./setup.sh` (recommended for most users)
2. **Docker** — container-based, no Rust toolchain needed
3. **Cargo** — build from source for development

## Automated Setup (recommended)

The setup script checks prerequisites, builds, tests, and connects your LLM in one command.

### Interactive

```bash
./setup.sh
```

### Headless (LLM-friendly)

```bash
./setup.sh --llm claude --auth oauth --skip-tests
```

All flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--llm <name>` | (interactive prompt) | LLM provider: `claude`, `codex` |
| `--auth <method>` | `oauth` | Auth method: `oauth`, `api` |
| `--api-key <key>` | — | API key (only with `--auth api`) |
| `--docker-only` | — | Skip cargo build |
| `--cargo-only` | — | Skip docker build |
| `--skip-tests` | — | Skip test suite |

## Docker Path

### Prerequisites

- Docker and Docker Compose

### Steps

```bash
cp .env.example .env
docker compose up -d
```

Verify the container is running:

```bash
docker compose ps
```

## Cargo Path

### Prerequisites

- Rust stable toolchain (1.75+) via [rustup](https://rustup.rs)
- Python 3.10+ with `rich` (`pip install rich`) for the TUI
- jq

### Steps

```bash
cp .env.example .env
cargo build --workspace
cargo test --workspace
cargo run -p odin-cli -- --config config/default.yaml --run-once
```

## Connecting an LLM

After building, connect your LLM CLI:

```bash
# Claude Code (OAuth — default)
scripts/odin/odin connect claude oauth

# Claude Code (API key)
scripts/odin/odin connect claude api

# Codex (OAuth)
scripts/odin/odin connect codex oauth
```

Use `--dry-run` to preview without making changes.

## Verification

Run the quickstart smoke test to confirm everything works:

```bash
bash scripts/verify/quickstart-smoke.sh
```

Full verification suite:

```bash
bash scripts/verify/quickstart-smoke.sh
bash scripts/verify/plugin-install-matrix.sh
bash scripts/verify/tui-core-smoke.sh
bash scripts/verify/bootstrap-wrapper-smoke.sh
bash scripts/verify/docs-command-smoke.sh
```

## Next Steps

- **TUI dashboard:** `python3 scripts/odin/odin-tui.py --live`
- **Bootstrap CLI:** `scripts/odin/odin help`
- **Integrations:** See `docs/integrations/` for Slack, Telegram, and n8n adapters
```

**Step 2: Commit**

```bash
git add docs/quickstart.md
git commit -m "docs: rewrite quickstart.md as unified Getting Started guide (finding #54)"
```

---

### Task 6: Update README to reference `setup.sh`

**Files:**
- Modify: `README.md`

**Step 1: Update Quickstart section**

Replace lines 14-21 of README.md:

```markdown
## Quickstart (recommended)

The fastest way to get started — automated setup with LLM connection:

```bash
./setup.sh
```

Or headless for LLM agents:

```bash
./setup.sh --llm claude --auth oauth --skip-tests
```

For manual setup paths (Docker or Cargo), see `docs/quickstart.md`.
```

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: update README quickstart to reference setup.sh (finding #54)"
```

---

### Task 7: Final verification and combined commit (if needed)

**Step 1: Full build and test**

Run: `cargo build --workspace && cargo test --workspace`
Expected: clean build, all non-ignored tests pass, 2 tests ignored

**Step 2: Verify setup.sh syntax**

Run: `bash -n setup.sh`
Expected: no errors

**Step 3: Verify CI YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"`
Expected: no errors

**Step 4: Verify no remaining serde_yaml references in source**

Run: `grep -r "serde_yaml" --include="*.rs" --include="*.toml" .` (excluding docs/plans, Cargo.lock)
Expected: no matches in source files
