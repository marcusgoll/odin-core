# State-Aware Skills Standard (SASS) v0.1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deliver an enforceable SASS v0.1 package in `odin-core` including a hard audit report, runnable XML skill contract/template/examples, and CI checks that block non-compliant skills.

**Architecture:** Implement docs + examples first, then add a minimal contract linter gate (`scripts/verify/skills-contract.sh`) that validates required SASS invariants (`wake_up`, exit states, guard checks, failure transitions). Keep runtime integration incremental: the contract is enforced in CI now and becomes the migration baseline for runtime-first state execution.

**Tech Stack:** Markdown docs, XML specs, Mermaid diagrams, Bash + Python stdlib validation, GitHub Actions CI.

---

Execution notes:
- Use `@test-driven-development` for every new script/lint behavior.
- Use `@verification-before-completion` before claiming success.
- Keep all new SASS artifacts under `docs/odin/skills/`.

### Task 1: Scaffold SASS docs tree and contract checker skeleton

**Files:**
- Create: `docs/odin/skills/`
- Create: `docs/odin/skills/templates/`
- Create: `docs/odin/skills/examples/`
- Create: `scripts/verify/skills-contract.sh`

**Step 1: Write the failing contract script behavior first**

Create a strict shell script skeleton:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

# fail until files are added
for required in \
  docs/odin/skills/state-aware-skills-standard.md \
  docs/odin/skills/state-aware-skills-audit.md \
  docs/odin/skills/templates/skill.template.xml \
  docs/odin/skills/examples/run-tests.skill.xml \
  docs/odin/skills/examples/run-tests.mermaid.md; do
  test -f "${required}"
done
```

**Step 2: Run check to verify it fails (RED)**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: FAIL because files do not exist yet.

**Step 3: Create directory skeleton only**

Run:

```bash
mkdir -p docs/odin/skills/templates docs/odin/skills/examples
```

**Step 4: Re-run check to confirm still RED**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: FAIL (files still missing).

**Step 5: Commit scaffold + failing gate**

```bash
git add docs/odin/skills scripts/verify/skills-contract.sh
git commit -m "chore(sass): scaffold docs tree and contract gate skeleton"
```

### Task 2: Author harsh audit report with evidence-backed scoring matrix

**Files:**
- Create: `docs/odin/skills/state-aware-skills-audit.md`

**Step 1: Define failing doc contract checks in script**

Extend `scripts/verify/skills-contract.sh` with required headings:

```bash
rg -q '^# Odin Meta Prompt and Meta Skill Audit' docs/odin/skills/state-aware-skills-audit.md
rg -q '^## Top Failures' docs/odin/skills/state-aware-skills-audit.md
rg -q '^## Score Table' docs/odin/skills/state-aware-skills-audit.md
```

**Step 2: Run gate to verify RED**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: FAIL (audit doc absent).

**Step 3: Write audit report content**

Include:
- Evidence scope and file list from `odin-orchestrator` + `odin-core`
- 0-2 rubric table for each meta prompt/meta skill family
- Remediation mapping per item
- Explicit missing-context callouts where code/docs are absent

**Step 4: Re-run gate to verify partial GREEN**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: progresses past audit-heading checks; still fails missing standard/template/example files.

**Step 5: Commit audit report**

```bash
git add docs/odin/skills/state-aware-skills-audit.md scripts/verify/skills-contract.sh
git commit -m "docs(sass): add evidence-based meta prompt and skill audit"
```

### Task 3: Write enforceable SASS v0.1 spec doc

**Files:**
- Create: `docs/odin/skills/state-aware-skills-standard.md`

**Step 1: Add failing standard-specific checks to gate**

```bash
rg -q '^## Terminology' docs/odin/skills/state-aware-skills-standard.md
rg -q '`wake_up` is mandatory' docs/odin/skills/state-aware-skills-standard.md
rg -q '^## Coordinates Snapshot' docs/odin/skills/state-aware-skills-standard.md
rg -q '^## XML Skill DSL' docs/odin/skills/state-aware-skills-standard.md
```

**Step 2: Run gate to confirm RED**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: FAIL for missing standard doc.

**Step 3: Implement spec doc sections**

Required sections:
- Terminology (`state`, `transition`, `guard`, `invariant`, `assumption`, `artifact`, `telemetry event`, `resume token`)
- Representation model (Mermaid `stateDiagram-v2` and DAG)
- XML DSL minimal schema requirements
- Mandatory `wake_up` behavior and resume algorithm
- Coordinates snapshot contract
- Failure classes and exit semantics

**Step 4: Run gate to verify partial GREEN**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: standard checks pass; still fails missing template/examples.

**Step 5: Commit spec doc**

```bash
git add docs/odin/skills/state-aware-skills-standard.md scripts/verify/skills-contract.sh
git commit -m "docs(sass): define state-aware skills standard v0.1"
```

### Task 4: Create XML template for compliant skills

**Files:**
- Create: `docs/odin/skills/templates/skill.template.xml`

**Step 1: Add failing XML structure checks**

Extend gate with Python XML validation:

```bash
python3 - <<'PY'
import xml.etree.ElementTree as ET
root = ET.parse('docs/odin/skills/templates/skill.template.xml').getroot()
assert root.tag == 'skill'
assert root.find('./states/state[@id="wake_up"]') is not None
assert root.find('./transitions') is not None
PY
```

**Step 2: Run gate to verify RED**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: FAIL because template file is missing.

**Step 3: Write template XML with required placeholders**

Use a complete template skeleton:

```xml
<skill id="skill.id" version="0.1.0" scope="project" trust_level="CAUTION">
  <capability_manifest>
    <allow capability="repo.read"/>
    <deny capability="repo.delete"/>
  </capability_manifest>
  <runtime start_state="wake_up">
    <coordinates>
      <field name="workspace_root" required="true"/>
      <field name="project_id" required="true"/>
      <field name="task_id" required="true"/>
      <field name="skill_id" required="true"/>
      <field name="state_id" required="true"/>
      <field name="attempt_count" required="true"/>
      <field name="last_artifacts" required="true"/>
      <field name="allowed_capabilities" required="true"/>
    </coordinates>
    <states>
      <state id="wake_up" type="checkpoint" idempotence="safe_repeat"/>
      <state id="exit_success" type="exit" terminal="true"/>
      <state id="exit_blocked" type="exit" terminal="true"/>
    </states>
    <transitions>
      <transition from="wake_up" to="exit_success" on="resume_or_start"/>
    </transitions>
  </runtime>
</skill>
```

**Step 4: Re-run gate to verify partial GREEN**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: template checks pass; example checks still fail.

**Step 5: Commit template**

```bash
git add docs/odin/skills/templates/skill.template.xml scripts/verify/skills-contract.sh
git commit -m "docs(sass): add XML skill template with mandatory wake_up contract"
```

### Task 5: Implement `run_tests` worked example XML with branching and failures

**Files:**
- Create: `docs/odin/skills/examples/run-tests.skill.xml`

**Step 1: Add failing behavior checks for example complexity**

Extend gate:

```bash
python3 - <<'PY'
import xml.etree.ElementTree as ET
root = ET.parse('docs/odin/skills/examples/run-tests.skill.xml').getroot()
states = {s.attrib.get('id') for s in root.findall('.//state')}
assert 'wake_up' in states
assert 'execute_tests' in states
transitions = root.findall('.//transition')
assert len(transitions) >= 8
failure_states = [s for s in root.findall('.//state') if s.attrib.get('type') == 'failure']
assert len(failure_states) >= 3
PY
```

**Step 2: Run gate to verify RED**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: FAIL (example missing).

**Step 3: Write full `run_tests` XML example**

Include states:
- `wake_up`, `detect_workspace`, `resolve_project`, `ensure_runtime`, `discover_tests`, `execute_tests`, `interpret_result`
- failures: `failure_unknown_project`, `failure_missing_runtime`, `failure_tests_not_found`, `failure_execution_error`, `failure_result_unparseable`, `failure_permission_denied`
- exits: `exit_success`, `exit_test_failures`, `exit_blocked`

Include explicit guarded transitions and retry edge from `failure_execution_error` to `execute_tests` with attempt guard.

**Step 4: Re-run gate to verify partial GREEN**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: example XML checks pass; Mermaid file checks may still fail.

**Step 5: Commit example XML**

```bash
git add docs/odin/skills/examples/run-tests.skill.xml scripts/verify/skills-contract.sh
git commit -m "docs(sass): add run_tests stateful XML example with failure paths"
```

### Task 6: Add Mermaid projection for `run_tests`

**Files:**
- Create: `docs/odin/skills/examples/run-tests.mermaid.md`

**Step 1: Add failing Mermaid checks**

```bash
rg -q '^```mermaid$' docs/odin/skills/examples/run-tests.mermaid.md
rg -q '^stateDiagram-v2' docs/odin/skills/examples/run-tests.mermaid.md
rg -q 'wake_up' docs/odin/skills/examples/run-tests.mermaid.md
rg -q 'failure_missing_runtime' docs/odin/skills/examples/run-tests.mermaid.md
```

**Step 2: Run gate to verify RED**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: FAIL (Mermaid file missing).

**Step 3: Write Mermaid diagrams (statechart + DAG)**

Include two fenced Mermaid blocks:
- `stateDiagram-v2` with branching and recovery transitions
- `flowchart TD` DAG view for operator readability

**Step 4: Re-run gate to verify GREEN for docs/examples**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: PASS for SASS doc contract checks.

**Step 5: Commit Mermaid example**

```bash
git add docs/odin/skills/examples/run-tests.mermaid.md scripts/verify/skills-contract.sh
git commit -m "docs(sass): add run_tests mermaid statechart and dag projections"
```

### Task 7: Publish migration plan and top 10 skills conversion priority

**Files:**
- Create: `docs/odin/skills/state-aware-skills-migration-plan.md`

**Step 1: Add failing migration-plan checks**

```bash
rg -q '^# SASS Migration Plan' docs/odin/skills/state-aware-skills-migration-plan.md
rg -q '^## Top 10 Skills to Convert First' docs/odin/skills/state-aware-skills-migration-plan.md
rg -q '^## CI Gates to Add' docs/odin/skills/state-aware-skills-migration-plan.md
```

**Step 2: Run gate to verify RED**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: FAIL (migration plan missing).

**Step 3: Write migration plan doc**

Must include:
- phased rollout (`observe -> warn -> enforce`)
- backward compatibility policy for legacy prompt-only skills
- top 10 skill conversion list (from current Odin runner roles/tasks)
- CI checks list and rollout policy

**Step 4: Re-run gate to verify GREEN**

Run: `bash scripts/verify/skills-contract.sh`  
Expected: PASS for migration-plan checks.

**Step 5: Commit migration plan**

```bash
git add docs/odin/skills/state-aware-skills-migration-plan.md scripts/verify/skills-contract.sh
git commit -m "docs(sass): add migration plan and conversion priority list"
```

### Task 8: Wire CI gate and run full verification

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/verify/workflow-contract.sh`

**Step 1: Add failing integration check in workflow contract**

Append to `scripts/verify/workflow-contract.sh`:

```bash
test -f scripts/verify/skills-contract.sh
bash scripts/verify/skills-contract.sh
```

**Step 2: Run workflow contract check locally (RED expected until CI wiring)**

Run: `bash scripts/verify/workflow-contract.sh`  
Expected: FAIL if dependencies are incomplete.

**Step 3: Add CI step in `contract-validation` job**

```yaml
      - name: SASS skills contract lint
        run: bash scripts/verify/skills-contract.sh
```

**Step 4: Run full local verification (GREEN target)**

Run:

```bash
bash scripts/verify/skills-contract.sh
bash scripts/verify/workflow-contract.sh
```

Expected: both PASS.

**Step 5: Commit CI integration**

```bash
git add .github/workflows/ci.yml scripts/verify/workflow-contract.sh scripts/verify/skills-contract.sh
git commit -m "ci(sass): enforce skills contract checks in workflow validation"
```

### Task 9: Final consistency pass and evidence capture

**Files:**
- Modify (if needed): `docs/odin/skills/state-aware-skills-audit.md`
- Modify (if needed): `docs/odin/skills/state-aware-skills-standard.md`

**Step 1: Verify deliverable checklist explicitly**

Checklist:
- audit summary + scoring table + remediations
- SASS spec doc
- XML template
- run-tests XML and Mermaid
- migration plan with top 10 conversions + CI gates

**Step 2: Run grep-based conformance checks**

```bash
rg -n "wake_up|coordinates|guard|failure|exit" docs/odin/skills/state-aware-skills-standard.md docs/odin/skills/examples/run-tests.skill.xml docs/odin/skills/examples/run-tests.mermaid.md
```

Expected: each deliverable contains required constructs.

**Step 3: Execute final verification commands**

```bash
bash scripts/verify/skills-contract.sh
bash scripts/verify/workflow-contract.sh
```

Expected: PASS.

**Step 4: Commit final alignment edits**

```bash
git add docs/odin/skills
if ! git diff --cached --quiet; then
  git commit -m "docs(sass): finalize audit and standard consistency"
fi
```

**Step 5: Prepare handoff summary**

Include:
- files created/modified
- verification commands and outcomes
- known limitations / missing repo context

