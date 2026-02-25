# TUI Stabilization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Lock in TUI readability and profile compatibility by adding deterministic agent-task selection coverage and enforcing stabilization checks.

**Architecture:** Use test-first changes in `scripts/odin/tui_core/tests/test_readability.py`, then apply a minimal collector fix in `agents.py` only to satisfy the new regression contract. Finish by tightening the smoke gate so CI/local verification always executes readability + core/legacy JSON checks together.

**Tech Stack:** Python 3, `unittest`, Rich TUI modules, Bash smoke script.

---

### Task 1: Add failing regression tests for agent dispatch selection

**Files:**
- Modify: `scripts/odin/tui_core/tests/test_readability.py`
- Test: `scripts/odin/tui_core/tests/test_readability.py`

**Step 1: Write the failing test**

Add these tests under `CollectorReadabilityTests`:

```python
    def test_agents_collect_prefers_newest_created_at_task_for_agent(self):
        with tempfile.TemporaryDirectory() as tmp:
            odin_dir = Path(tmp)
            agent_dir = odin_dir / "agents" / "sm"
            agent_dir.mkdir(parents=True)
            (agent_dir / "status.json").write_text(json.dumps({"role": "sm"}))
            (odin_dir / "state.json").write_text(
                json.dumps(
                    {
                        "dispatched_tasks": {
                            "new-task": {
                                "agent": "sm",
                                "created_at": "2026-02-25T12:00:00+00:00",
                            },
                            "old-task": {
                                "agent": "sm",
                                "created_at": "2026-02-25T10:00:00+00:00",
                            },
                        }
                    }
                )
            )
            data = collect_agents(odin_dir)
            self.assertEqual(data.items[0]["task"], "new-task")
            self.assertEqual(data.items[0]["state"], "busy")

    def test_agents_collect_falls_back_to_unknown_without_dispatch_or_state(self):
        with tempfile.TemporaryDirectory() as tmp:
            odin_dir = Path(tmp)
            agent_dir = odin_dir / "agents" / "sm"
            agent_dir.mkdir(parents=True)
            (agent_dir / "status.json").write_text(json.dumps({"role": "sm"}))
            (odin_dir / "state.json").write_text(json.dumps({"dispatched_tasks": {}}))
            data = collect_agents(odin_dir)
            self.assertEqual(data.items[0]["task"], "-")
            self.assertEqual(data.items[0]["state"], "unknown")
```

**Step 2: Run test to verify it fails**

Run:
`python3 -m unittest scripts.odin.tui_core.tests.test_readability.CollectorReadabilityTests.test_agents_collect_prefers_newest_created_at_task_for_agent -v`

Expected:
`FAIL` showing selected task is `old-task` instead of `new-task`.

**Step 3: Commit failing test**

```bash
git add scripts/odin/tui_core/tests/test_readability.py
git commit -m "test(tui): add regression coverage for agent dispatch ordering"
```

### Task 2: Implement deterministic task selection in agents collector

**Files:**
- Modify: `scripts/odin/tui_core/collectors/agents.py`
- Modify: `scripts/odin/tui_core/tests/test_readability.py`

**Step 1: Write minimal implementation**

Update dispatch mapping logic to select newest task by `created_at` per agent (fall back to task id ordering when timestamps are missing):

```python
def _dispatch_sort_key(task_id: str, info: dict) -> tuple[str, str]:
    created_at = str((info or {}).get("created_at") or "")
    return (created_at, task_id)


dispatch_by_agent = {}
for task_id, info in sorted(dispatch.items(), key=lambda item: _dispatch_sort_key(item[0], item[1])):
    agent = (info or {}).get("agent")
    if agent:
        dispatch_by_agent[agent] = task_id
```

**Step 2: Run test to verify it passes**

Run:
`python3 -m unittest scripts.odin.tui_core.tests.test_readability.CollectorReadabilityTests.test_agents_collect_prefers_newest_created_at_task_for_agent -v`

Expected:
`OK`.

**Step 3: Run focused readability suite**

Run:
`python3 -m unittest scripts.odin.tui_core.tests.test_readability -v`

Expected:
All tests pass.

**Step 4: Commit implementation**

```bash
git add scripts/odin/tui_core/collectors/agents.py scripts/odin/tui_core/tests/test_readability.py
git commit -m "fix(tui): make dispatched task selection deterministic per agent"
```

### Task 3: Enforce stabilization checks in smoke gate

**Files:**
- Modify: `scripts/verify/tui-core-smoke.sh`

**Step 1: Write failing gate check**

Add readability tests to the smoke script near the top:

```bash
echo "[tui-smoke] RUN readability tests"
python3 -m unittest scripts.odin.tui_core.tests.test_readability
```

**Step 2: Run smoke script to verify behavior**

Run:
`bash scripts/verify/tui-core-smoke.sh`

Expected:
If readability regresses, script fails before JSON/live checks. Otherwise passes end-to-end.

**Step 3: Commit smoke gate change**

```bash
git add scripts/verify/tui-core-smoke.sh
git commit -m "chore(verify): include tui readability tests in smoke gate"
```

### Task 4: Final verification and completion commit

**Files:**
- Modify: as needed from prior tasks

**Step 1: Run full stabilization verification**

Run:
- `python3 -m unittest scripts.odin.tui_core.tests.test_readability`
- `python3 scripts/odin/odin-tui.py --json`
- `python3 scripts/odin/odin-tui.py --profile legacy --json`
- `bash scripts/verify/tui-core-smoke.sh`

Expected:
All commands succeed.

**Step 2: Create completion commit**

```bash
git add -A
git commit -m "chore(tui): complete stabilization verification lane"
```

**Step 3: Capture completion evidence**

Record command outputs in PR/summary notes:
- test pass counts,
- smoke script completion marker,
- confirmation that both `core` and `legacy` JSON commands succeed.
