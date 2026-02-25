# Core TUI Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deliver a modular, responsive core-default TUI with legacy opt-in compatibility.

**Architecture:** Introduce `scripts/odin/tui_core` as the new modular runtime, preserve legacy monolith in a separate file, and keep command compatibility via thin entrypoint/wrapper.

**Tech Stack:** Python 3, Rich, Bash smoke checks.

---

### Task 1: Preserve legacy implementation and create modular package scaffold

**Files:**
- Create: `scripts/odin/odin-tui-legacy.py`
- Create: `scripts/odin/tui_core/__init__.py`
- Create: `scripts/odin/tui_core/models.py`
- Create: `scripts/odin/tui_core/profiles.py`
- Create: `scripts/odin/tui_core/layout.py`
- Create: `scripts/odin/tui_core/collectors/__init__.py`
- Create: `scripts/odin/tui_core/panels/__init__.py`

**Steps:**
1. Copy current monolithic `scripts/odin/odin-tui.py` to `scripts/odin/odin-tui-legacy.py`.
2. Add dataclass contracts (`PanelData`, app config helpers).
3. Add profile defaults and config loading logic.
4. Add layout-mode selection utility.

**Verification:**
- `python3 -m py_compile` over created files.

### Task 2: Implement core collectors and panel renderers

**Files:**
- Create: `scripts/odin/tui_core/collectors/{orchestrator,agents,inbox,kanban,logs,github}.py`
- Create: `scripts/odin/tui_core/panels/{header,inbox,kanban,agents,logs,github}.py`

**Steps:**
1. Implement each collector with fail-soft behavior.
2. Implement each panel renderer consuming normalized data.
3. Ensure inbox/kanban/agents/logs/github are first-class in `core`.

**Verification:**
- `python3 -m py_compile scripts/odin/tui_core/collectors/*.py scripts/odin/tui_core/panels/*.py`
- `python3 scripts/odin/odin-tui.py --json` returns required panel keys.

### Task 3: Implement new modular app entrypoint + compatibility routing

**Files:**
- Create: `scripts/odin/tui_core/app.py`
- Modify: `scripts/odin/odin-tui.py`

**Steps:**
1. Implement CLI in `app.py` with `--profile`, `--config`, `--live`, `--json`.
2. Add responsive rendering composition using layout modes.
3. Make `--profile legacy` delegate to `scripts/odin/odin-tui-legacy.py`.
4. Convert `scripts/odin/odin-tui.py` into thin bootstrap that calls `tui_core.app.main()`.

**Verification:**
- `python3 scripts/odin/odin-tui.py --json`
- `python3 scripts/odin/odin-tui.py --profile legacy --json`
- `timeout 6 python3 scripts/odin/odin-tui.py --live` (exit 124 acceptable)

### Task 4: Tests, docs, and verification script

**Files:**
- Create: `scripts/odin/tui_core/tests/test_profiles.py`
- Create: `scripts/odin/tui_core/tests/test_layout.py`
- Create: `scripts/verify/tui-core-smoke.sh`
- Modify: `README.md`

**Steps:**
1. Add profile/layout unit tests via `unittest`.
2. Add smoke script for core/legacy json+live startup checks.
3. Document new usage and profile selection in README.

**Verification:**
- `python3 -m unittest scripts.odin.tui_core.tests.test_profiles scripts.odin.tui_core.tests.test_layout`
- `bash scripts/verify/tui-core-smoke.sh`

### Task 5: Full verification and commit

**Files:**
- Modify: as needed from prior tasks

**Steps:**
1. Run full TUI + existing core checks.
2. Commit focused changes.

**Verification:**
- `cargo test --workspace`
- `bash scripts/verify/tui-core-smoke.sh`
- `python3 scripts/odin/odin-tui.py --json`
- `python3 scripts/odin/odin-tui.py --profile legacy --json`
