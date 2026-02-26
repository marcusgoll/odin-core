#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

REQUIRED_FILES=(
  "docs/odin/skills/state-aware-skills-audit.md"
  "docs/odin/skills/state-aware-skills-standard.md"
  "docs/odin/skills/state-aware-skills-migration-plan.md"
  "docs/odin/skills/templates/skill.template.xml"
  "docs/odin/skills/examples/run-tests.skill.xml"
  "docs/odin/skills/examples/run-tests.mermaid.md"
)

for required in "${REQUIRED_FILES[@]}"; do
  if [[ ! -f "${required}" ]]; then
    echo "[skills-contract] ERROR missing file: ${required}" >&2
    exit 1
  fi
done

echo "[skills-contract] PASS required files"

grep -Eq '^# Odin Meta Prompt and Meta Skill Audit' docs/odin/skills/state-aware-skills-audit.md
grep -Eq '^## Top Failures' docs/odin/skills/state-aware-skills-audit.md
grep -Eq '^## Concrete Remediation Table' docs/odin/skills/state-aware-skills-audit.md
echo "[skills-contract] PASS audit structure"

grep -Eq '^## Terminology' docs/odin/skills/state-aware-skills-standard.md
grep -Eq '^### Required `wake_up` State' docs/odin/skills/state-aware-skills-standard.md
grep -Eq '^## Coordinates Snapshot' docs/odin/skills/state-aware-skills-standard.md
grep -Eq '^## XML Skill DSL \(Minimal\)' docs/odin/skills/state-aware-skills-standard.md
echo "[skills-contract] PASS standard structure"

grep -Eq '^## Top 10 Skills to Convert First' docs/odin/skills/state-aware-skills-migration-plan.md
grep -Eq '^## CI Gates to Add' docs/odin/skills/state-aware-skills-migration-plan.md
echo "[skills-contract] PASS migration plan structure"

python3 - <<'PY'
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

errors = []

def add_error(message: str) -> None:
    errors.append(message)

def parse_skill(path: Path):
    try:
        root = ET.parse(path).getroot()
    except ET.ParseError as exc:
        add_error(f"{path}: invalid XML ({exc})")
        return None
    if root.tag != "skill":
        add_error(f"{path}: root element must be <skill>")
        return None
    runtime = root.find("./runtime")
    if runtime is None:
        add_error(f"{path}: missing <runtime>")
        return None
    return root, runtime

def check_wake_up_and_exits(path: Path, runtime: ET.Element):
    start_state = runtime.attrib.get("start_state")
    if start_state != "wake_up":
        add_error(f"{path}: runtime start_state must be wake_up")

    states = runtime.findall("./states/state")
    state_ids = {s.attrib.get("id") for s in states if s.attrib.get("id")}
    if "wake_up" not in state_ids:
        add_error(f"{path}: missing wake_up state")

    strict_exit_states = [
        s for s in states
        if s.attrib.get("type") == "exit" and s.attrib.get("terminal") == "true"
    ]
    if not strict_exit_states:
        add_error(f"{path}: must include at least one exit state with type='exit' and terminal='true'")

def check_transitions(path: Path, runtime: ET.Element):
    transitions = runtime.findall("./transitions/transition")
    if not transitions:
        add_error(f"{path}: missing transitions")
        return

    states = runtime.findall("./states/state")
    state_types = {s.attrib.get("id"): s.attrib.get("type", "") for s in states if s.attrib.get("id")}
    guard_defs = {g.attrib.get("id") for g in runtime.findall("./guards/guard") if g.attrib.get("id")}
    state_ids = set(state_types.keys())

    for transition in transitions:
        frm = transition.attrib.get("from")
        to = transition.attrib.get("to")
        if not frm or not to:
            add_error(f"{path}: transition missing from/to attributes")
            continue
        if frm not in state_ids:
            add_error(f"{path}: transition source '{frm}' does not exist in <states>")
        if to not in state_ids:
            add_error(f"{path}: transition target '{to}' does not exist in <states>")
        if state_types.get(frm) == "exit":
            add_error(f"{path}: exit state '{frm}' must not have outgoing transitions")

    # Required contract: skill XMLs must include guard checks on operational transitions.
    if not guard_defs:
        add_error(f"{path}: missing <guards> definitions")
        return
    for transition in transitions:
        frm = transition.attrib.get("from", "")
        frm_type = state_types.get(frm, "")
        if frm_type in ("failure", "exit"):
            continue
        guard_ref = transition.attrib.get("guard_ref")
        if not guard_ref:
            add_error(f"{path}: missing guard_ref on operational transition from '{frm}'")
            continue
        if guard_ref not in guard_defs:
            add_error(f"{path}: guard_ref '{guard_ref}' not declared in <guards>")

template_path = Path("docs/odin/skills/templates/skill.template.xml")
example_paths = sorted(Path("docs/odin/skills/examples").glob("*.skill.xml"))
if not example_paths:
    add_error("docs/odin/skills/examples: must contain at least one *.skill.xml example")

for path in [template_path, *example_paths]:
    parsed = parse_skill(path)
    if not parsed:
        continue
    _, runtime = parsed
    check_wake_up_and_exits(path, runtime)
    check_transitions(path, runtime)

if errors:
    for error in errors:
        print(f"[skills-contract] ERROR {error}", file=sys.stderr)
    sys.exit(1)

print("[skills-contract] PASS XML contract checks")
PY

grep -Eq '^```mermaid$' docs/odin/skills/examples/run-tests.mermaid.md
grep -Eq '^stateDiagram-v2$' docs/odin/skills/examples/run-tests.mermaid.md
grep -Eq '^flowchart TD$' docs/odin/skills/examples/run-tests.mermaid.md
grep -Eq 'wake_up' docs/odin/skills/examples/run-tests.mermaid.md
grep -Eq 'failure_execution_error' docs/odin/skills/examples/run-tests.mermaid.md
echo "[skills-contract] PASS mermaid projections"

echo "[skills-contract] COMPLETE"
