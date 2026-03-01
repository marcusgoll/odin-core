# Upgrade from v0.x to v1.x Without Losing Data

This guide upgrades an existing Odin setup to `odin-core` execution while preserving skills, learnings, and runtime history.

## Before You Start

- Current date context for this guide: 2026-02-26.
- Ensure you can access your legacy repo and data root.
- Ensure you have enough disk for backup + export bundle.

Required inputs:

- `LEGACY_ROOT` (legacy scripts repo root)
- `ODIN_DIR` (legacy data root, usually `/var/odin`)
- `ODIN_DATA_ROOT` (new canonical data root; can be same root with migration layout)

## 1) Backup First

```bash
export LEGACY_ROOT=/path/to/odin-orchestrator
export ODIN_DIR=/var/odin
export SNAPSHOT_DIR=/tmp/odin-backup-$(date +%Y%m%d-%H%M%S)

mkdir -p "$SNAPSHOT_DIR"
cp -a "$ODIN_DIR" "$SNAPSHOT_DIR/odin-dir"
```

## 2) Export User Data Bundle

```bash
odin-cli migrate export \
  --source-root "$LEGACY_ROOT" \
  --odin-dir "$ODIN_DIR" \
  --out /tmp/odin-migration-bundle
```

Validate bundle:

```bash
odin-cli migrate validate --bundle /tmp/odin-migration-bundle
```

## 3) Import (Dry Run Then Apply)

```bash
export ODIN_DATA_ROOT=/var/odin

odin-cli migrate import \
  --bundle /tmp/odin-migration-bundle \
  --target "$ODIN_DATA_ROOT" \
  --dry-run

odin-cli migrate import \
  --bundle /tmp/odin-migration-bundle \
  --target "$ODIN_DATA_ROOT" \
  --apply
```

## 4) Run in Shadow Mode (Recommended)

```bash
export ODIN_ENGINE_MODE=shadow
export ODIN_CORE_BIN=/path/to/odin-cli
export ODIN_CORE_CONFIG=/path/to/config/default.yaml
export ODIN_LEGACY_ROOT="$LEGACY_ROOT"
```

In shadow mode, legacy path remains authoritative while `odin-core` outputs are compared.

## 5) Validate Parity

Run compatibility checks and your golden tasks suite.

```bash
bash /path/to/odin-core/scripts/verify/compat-regression.sh --legacy-root "$LEGACY_ROOT"
```

Only continue if parity reports are acceptable.

## 6) Cut Over to Core

```bash
export ODIN_ENGINE_MODE=core
```

Monitor health, queue behavior, and policy events for your soak window.

## Rollback (Fast Path)

If regressions appear:

```bash
export ODIN_ENGINE_MODE=legacy
```

Then restore backup snapshot and re-run legacy smoke checks.

## Common Failures and Recovery

1. Invalid bundle schema
- Symptom: `migrate validate` fails.
- Recovery: fix/export again; do not import.

2. Checksum mismatch on import
- Symptom: importer reports hash mismatch.
- Recovery: stop cutover, restore backup, regenerate bundle.

3. Skill parse failures
- Symptom: items moved to quarantine.
- Recovery: review quarantine report, correct malformed skills, re-import.

4. Resume after interrupted migration
- Symptom: migration stopped mid-run.
- Recovery: rerun importer. Import is idempotent and resumes from migration logs.

5. Runtime interruption during task execution
- Symptom: worker/task left mid-flight.
- Recovery: execute `wake_up` in resumed run; engine reads coordinates/checkpoint state and resumes safe state or restarts deterministically.

## Proof Checklist

- pre/post inventory counts captured
- bundle checksum file recorded
- golden task comparison report archived
- rollback drill completed successfully

