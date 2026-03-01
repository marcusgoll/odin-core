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

# --- Color control ---
if [ -t 1 ]; then
  BOLD="\033[1m"; GREEN="\033[32m"; RED="\033[31m"; YELLOW="\033[33m"; RESET="\033[0m"
else
  BOLD=""; GREEN=""; RED=""; YELLOW=""; RESET=""
fi

log()   { echo -e "[setup] $*"; }
ok()    { log "${GREEN}OK${RESET}   $*"; }
miss()  { log "${YELLOW}MISSING${RESET} $*"; }
fail()  { log "${RED}FAIL${RESET} $*"; exit 1; }
done_() { log "${GREEN}DONE${RESET} $*"; }

# --- Parse flags ---
while [[ $# -gt 0 ]]; do
  case "$1" in
    --llm)         LLM="$2"; shift 2 ;;
    --auth)        AUTH="$2"; shift 2 ;;
    --api-key)     API_KEY="$2"; shift 2 ;;
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
