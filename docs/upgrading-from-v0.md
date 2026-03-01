# Upgrading from v0.x (Bash Orchestrator) to v1.x (odin-core)

This guide walks you through migrating from the legacy bash-based odin-orchestrator to the odin-core Rust engine. The process is designed to be safe, reversible, and verifiable at every step.

**Time required:** ~30 minutes (without shadow mode), ~5 hours (with shadow mode)

**Rollback time:** < 2 minutes

## Prerequisites

- odin-orchestrator is installed and has been running (data exists in `$ODIN_DIR`, default `/var/odin/`)
- odin-core is installed and `odin-cli` is available in `$PATH` or built in `target/release/`
- `jq`, `python3`, and `sha256sum` are available
- You have `sudo` access (for systemctl commands)

## Step 1: Back Up and Export

```bash
# Create a safety backup of current state
bash scripts/odin/odin-migrate.sh backup \
  --target /var/odin/backups/pre-migration-$(date +%Y%m%dT%H%M)

# Export all data into a self-contained bundle
bash scripts/odin/odin-migrate.sh export \
  --output ./odin-export-$(date +%Y%m%dT%H%M)
```

The export produces a directory containing:
- `MANIFEST.json` — schema version, file counts, SHA-256 checksums
- `skills/` — converted agent prompts + learned skills (SKILL.md format)
- `config/` — workers.yaml, routing.yaml, policy/core-policy.yaml
- `memory/` — hot and cold memory (verbatim copy)
- `state/` — state.json, kanban, budgets, autonomy contracts
- `quarantine/` — any files that couldn't be auto-converted

## Step 2: Verify the Export

```bash
bash scripts/odin/odin-migrate.sh verify \
  --bundle ./odin-export-*
```

Expected output: all categories show `MATCH`, zero verification failures. Do not proceed if verification fails.

## Step 3: Stop the Old Engine

```bash
sudo systemctl stop odin.service odin-keepalive.timer
```

Confirm it's stopped:

```bash
sudo systemctl status odin.service
tmux list-sessions | grep odin  # should be empty
```

## Step 4: Import into odin-core

```bash
odin import ./odin-export-*/
```

This will:
1. Verify all checksums in the bundle
2. Back up any existing `$ODIN_DIR` data
3. Copy skills to `$ODIN_DIR/.claude/skills/`
4. Copy config to `$ODIN_DIR/config/`
5. Copy memory to `$ODIN_DIR/memory/`
6. Copy state to `$ODIN_DIR/state/`
7. Write `$ODIN_DIR/data.version` = `1`
8. Re-verify all files against MANIFEST checksums

If post-import verification fails, the importer automatically restores from backup.

## Step 5: Shadow Mode (Optional, Recommended)

Run both engines in parallel to build confidence before switching:

```bash
# Copy data for shadow run
sudo cp -a /var/odin /var/odin-shadow

# Start old engine normally
sudo systemctl start odin.service

# Start odin-core in dry-run mode against the shadow copy
ODIN_DIR=/var/odin-shadow odin-cli --dry-run &
```

Run for 1-4 hours. Compare logs to verify odin-core starts, loads skills, polls inbox, and makes correct policy decisions.

```bash
# When satisfied, stop shadow
sudo systemctl stop odin.service
kill %1
sudo rm -rf /var/odin-shadow
```

## Step 6: Switch Engines

Set the engine toggle:

```bash
echo 'ODIN_ENGINE=core' >> ~/.odin-env
```

Start the service:

```bash
sudo systemctl start odin.service odin-keepalive.timer
```

Monitor the first few cycles:

```bash
sudo journalctl -u odin.service -f
```

## Step 7: Verify

Check that the new engine is running correctly:

```bash
# Heartbeat is being updated
cat /var/odin/heartbeat

# Skills are loaded
ls /var/odin/.claude/skills/

# Config is readable
cat /var/odin/config/runtime.yaml

# Inbox is being processed (if tasks exist)
ls /var/odin/inbox/
```

## Rollback

If anything goes wrong, rollback takes less than 2 minutes:

```bash
# 1. Switch back to legacy engine
sed -i 's/ODIN_ENGINE=core/ODIN_ENGINE=legacy/' ~/.odin-env

# 2. Stop the service
sudo systemctl stop odin.service

# 3. Restore from pre-migration backup
sudo cp -a /var/odin/backups/pre-migration-*/. /var/odin/

# 4. Restart with legacy engine
sudo systemctl start odin.service odin-keepalive.timer
```

The rollback works because:
- The backup is a complete snapshot of pre-migration state
- The engine toggle is a single env var
- Old bash modules are untouched (never modified during migration)
- The `odin/memory` git branch is immutable

**Rollback triggers** (when to rollback):
- odin-core fails to start
- No heartbeat for 3+ minutes
- Inbox not processing for 10+ minutes
- `exit_failed` on a task that previously passed

## Common Issues

### Skills not loading

Check that skills are in the correct path:

```bash
ls -la /var/odin/.claude/skills/odin-*/SKILL.md
```

Each skill should have a `---` frontmatter block with `name`, `description`, `task_types`, `backend`, and `risk_tier`.

### Inbox stalled

Verify compat mode is enabled in the runtime config:

```bash
cat /var/odin/config/runtime.yaml | grep mode
```

Should show `mode: compat`. This enables bash compat adapters for task ingress, backend state, and failover.

### Policy denying all actions

Check the core policy:

```bash
cat /var/odin/config/policy/core-policy.yaml
```

Verify `autonomy_mode` matches your expected setting (e.g., `full`). Adjust `spend_caps_usd` if actions are being blocked by budget constraints.

### Keepalive restarts

Check that the heartbeat file is writable:

```bash
ls -la /var/odin/heartbeat
touch /var/odin/heartbeat
```

If permissions are wrong, fix with:

```bash
sudo chown orchestrator:orchestrator /var/odin/heartbeat
```

### Learning pipeline not running

Verify the bash compat adapter path in runtime.yaml points to the correct location:

```bash
grep legacy_root /var/odin/config/runtime.yaml
```

Should point to the odin-orchestrator `scripts/odin/` directory where `lib/learn-pipeline.sh` and `lib/memory-sync.sh` live.

### Wake-up after crash

Check `state.json` reflects the last complete cycle:

```bash
jq '.orchestrator_started_at, .tasks_this_session' /var/odin/state/state.json
```

If state seems stale, the engine will re-initialize on next start. No manual intervention needed.
