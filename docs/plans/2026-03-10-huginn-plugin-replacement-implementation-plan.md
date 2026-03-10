# Huginn Plugin Replacement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace Stagehand-specific browser automation surfaces in `odin-core` with a Huginn plugin and Huginn-named governance/runtime contracts.

**Architecture:** Move the plugin package from `plugins/stagehand` to `plugins/huginn`, replace Stagehand SDK usage with a Huginn HTTP client, and rename runtime/governance/CLI/docs/test references from `stagehand` to `huginn` while keeping generic `browser.navigate` and `browser.observe` as stable cross-plugin primitives.

**Tech Stack:** Rust, TypeScript, Node.js, Vitest, Huginn HTTP API, Odin plugin protocol

---

### Task 1: Create the failing Rust rename expectations

**Files:**
- Modify: `bin/odin-cli/tests/governance_cli.rs`
- Modify: `crates/odin-core-runtime/tests/capability_manifest_enforcement.rs`
- Modify: `crates/odin-governance/tests/stagehand_policy.rs`

**Step 1: Rename Stagehand-focused test names and fixtures to Huginn**

Update the tests to expect:
- `--plugin huginn`
- `plugin: "huginn"`
- `huginn.enabled`
- `huginn.observe_url`, `huginn.observe_domain`, `huginn.workspace.read`, `huginn.command.run`

**Step 2: Run targeted tests to verify they fail before implementation**

Run: `cargo test governance_enable_plugin_huginn -- --nocapture`

Expected: failures referencing unsupported plugin name or old stagehand values.

**Step 3: Commit test-only red state if split commits are useful**

If kept separate:

```bash
git add bin/odin-cli/tests/governance_cli.rs crates/odin-core-runtime/tests/capability_manifest_enforcement.rs crates/odin-governance/tests/stagehand_policy.rs
git commit -m "test(huginn): rename governance expectations"
```

### Task 2: Rename governance and runtime policy code

**Files:**
- Modify: `crates/odin-governance/src/plugins.rs`
- Modify: `crates/odin-core-runtime/src/lib.rs`
- Modify: `bin/odin-cli/src/main.rs`

**Step 1: Rename policy types/functions from Stagehand to Huginn**

Change:
- `StagehandMode` -> `HuginnMode`
- `StagehandPolicy` -> `HuginnPolicy`
- `stagehand_default_policy` -> `huginn_default_policy`
- `stagehand_policy_from_envelope` -> `huginn_policy_from_envelope`

**Step 2: Re-key the plugin registry and CLI to `huginn`**

Make the CLI `enable-plugin` path accept only `huginn` and emit `"plugin": "huginn"` in JSON responses.

**Step 3: Rename runtime capability parsing**

Update runtime helpers so:
- plugin id must be `huginn`
- `huginn.*` capability ids are understood
- `browser.observe`, `workspace.read`, and `command.run` remain recognized generic aliases

**Step 4: Run targeted Rust tests**

Run: `cargo test governance_enable_plugin_huginn`

Expected: targeted governance tests pass.

**Step 5: Commit**

```bash
git add crates/odin-governance/src/plugins.rs crates/odin-core-runtime/src/lib.rs bin/odin-cli/src/main.rs bin/odin-cli/tests/governance_cli.rs crates/odin-core-runtime/tests/capability_manifest_enforcement.rs crates/odin-governance/tests/stagehand_policy.rs
git commit -m "refactor(huginn): rename browser governance surface"
```

### Task 3: Replace the plugin package with Huginn

**Files:**
- Move/replace: `plugins/stagehand` -> `plugins/huginn`
- Modify: `plugins/huginn/package.json`
- Modify: `plugins/huginn/odin.plugin.yaml`
- Modify: `plugins/huginn/src/config.ts`
- Modify: `plugins/huginn/src/index.ts`
- Add/modify: `plugins/huginn/src/huginn-client.ts`
- Remove Stagehand-specific files/helpers not needed

**Step 1: Rename the package and manifest**

Change package/manifest identity to:
- package name: `odin-plugin-huginn`
- plugin name: `odin.huginn`
- capabilities updated to Huginn/browser set

**Step 2: Replace Stagehand SDK dependency**

Remove `@browserbasehq/stagehand`.
Use plain HTTP calls to Huginn via Node’s fetch API or a minimal HTTP wrapper.

**Step 3: Implement a Huginn client**

Support:
- `/health`
- `/launch`
- `/navigate`
- `/snapshot`
- `/act`
- `/screenshot`
- `/cookies`
- `/cookies/set`

**Step 4: Rework execution paths**

Map capability ids to Huginn calls:
- `browser.navigate` -> `/navigate`
- `browser.observe` / `huginn.snapshot` / `huginn.observe_*` -> `/snapshot`
- `huginn.click` / `huginn.type` / `huginn.press` / `huginn.hover` -> `/act`
- `huginn.back` / `huginn.forward` / `huginn.reload` -> `/navigate` with action payload
- `huginn.screenshot` -> `/screenshot`
- `huginn.cookies.get` / `huginn.cookies.set` -> cookie endpoints

**Step 5: Drop unsupported Stagehand-only capabilities**

Remove `browser.extract` and `browser.agent` from supported capability routing and manifest declarations.

**Step 6: Run plugin tests/build**

Run: `npm run build`

Expected: build succeeds in `plugins/huginn`.

**Step 7: Commit**

```bash
git add plugins/huginn
git commit -m "feat(huginn): replace stagehand plugin with huginn client"
```

### Task 4: Rewrite plugin tests around Huginn semantics

**Files:**
- Modify: `plugins/huginn/tests/protocol.test.ts`
- Modify: `plugins/huginn/tests/config.test.ts`
- Add: `plugins/huginn/tests/huginn-client.test.ts`
- Remove/update Stagehand QA tests as needed

**Step 1: Keep deterministic unit coverage**

Test config loading, protocol routing, and HTTP request shaping with mocked fetch.

**Step 2: Remove or quarantine Stagehand-specific QA tests**

Tests that require `Stagehand` instances or AI extract/agent flows should be removed or rewritten to Huginn snapshot/action semantics.

**Step 3: Run targeted plugin tests**

Run: `npm test`

Expected: plugin test suite passes without requiring real AI credentials.

**Step 4: Commit**

```bash
git add plugins/huginn/tests
git commit -m "test(huginn): cover HTTP client and routing"
```

### Task 5: Update docs and safety guidance

**Files:**
- Move/replace: `docs/stagehand-safety.md` -> `docs/huginn-safety.md`
- Modify: `docs/release-readiness.md`
- Modify: stagehand design/plan docs if they are part of current navigational docs

**Step 1: Rewrite operator guidance from Stagehand to Huginn**

Update plugin identity, enablement commands, capability ids, and safety examples.

**Step 2: Update references that claim Stagehand is the active browser plugin**

Keep history docs intact where they are explicitly historical, but current-facing docs should point to Huginn.

**Step 3: Verify docs paths referenced by tests/scripts still exist or are updated**

Run any relevant smoke checks if they depend on the doc path.

**Step 4: Commit**

```bash
git add docs/huginn-safety.md docs/release-readiness.md
git commit -m "docs(huginn): replace stagehand operator guidance"
```

### Task 6: Full verification

**Files:**
- Verify the full worktree state

**Step 1: Run Rust verification**

Run: `cargo test`

Expected: full Rust suite passes.

**Step 2: Run plugin verification**

Run: `npm run build`
Run: `npm test`

Expected: Huginn plugin package builds and tests pass.

**Step 3: Run patch hygiene**

Run: `git diff --check`

Expected: no whitespace or patch formatting errors.

**Step 4: Push and open PR**

```bash
git push -u origin huginn-replace-stagehand
gh pr create --base main --head huginn-replace-stagehand --title "feat(huginn): replace stagehand plugin surface" --body-file /tmp/huginn-pr.md
```

Plan complete and saved to `docs/plans/2026-03-10-huginn-plugin-replacement-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?
