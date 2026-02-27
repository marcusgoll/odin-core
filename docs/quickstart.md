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

## Bootstrap CLI (dry-run walkthrough)

Explore the full command surface safely with `--dry-run`:

```bash
scripts/odin/odin help
scripts/odin/odin connect claude oauth --dry-run
scripts/odin/odin start --dry-run
scripts/odin/odin tui --dry-run
scripts/odin/odin inbox add "bootstrap task" --dry-run
scripts/odin/odin inbox list
scripts/odin/odin gateway add cli --dry-run
scripts/odin/odin verify --dry-run
```

Conservative default: if `config/guardrails.yaml` is missing, mutating commands are blocked unless `--dry-run` is used.

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
