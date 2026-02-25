# TUI Stabilization Design

## Goal

Close the current TUI stabilization lane by locking in verified behavior for readability and profile compatibility without widening scope into a refactor.

## Problem

The TUI redesign work has moved quickly and requires a bounded closure step to prevent regressions in:
- readability-focused collector/panel output, and
- command compatibility between `core` and `legacy` profiles.

Without an explicit stabilization pass, future edits can silently break operator-facing output quality or profile routing behavior.

## Outcome

A single stabilization ticket confirms and preserves current behavior through targeted, repeatable checks:
- readability tests pass,
- core JSON rendering works,
- legacy JSON rendering works.

## Constraints

- No broad collector/panel refactor.
- Preserve current command surface and profile semantics.
- Favor verification and small corrective fixes only if checks fail.

## Design

### Architecture

Use existing modules and treat this as a verification gate:
- `scripts/odin/tui_core/tests/test_readability.py`
- `scripts/odin/odin-tui.py` (`core` default path)
- `scripts/odin/odin-tui.py --profile legacy` (compat path)

No new runtime components are introduced.

### Components in Scope

- Readability test coverage for formatting and panel rendering contracts.
- Core profile JSON command path.
- Legacy profile JSON command path.

### Data Flow

1. Run readability suite to validate formatting, collector normalization, and panel readability expectations.
2. Run core profile JSON command to verify modular dashboard collection path.
3. Run legacy profile JSON command to verify compatibility routing path.
4. If any check fails, apply the minimal code fix and rerun all checks.

### Error Handling

- Fail fast on first broken check to isolate defect cause.
- Apply least-destructive correction to the failing component.
- Re-verify full stabilization check set before claiming completion.

### Testing Strategy

- `python3 -m unittest scripts.odin.tui_core.tests.test_readability`
- `python3 scripts/odin/odin-tui.py --json`
- `python3 scripts/odin/odin-tui.py --profile legacy --json`

## Acceptance Criteria

- Readability test suite passes.
- Core profile JSON output command exits successfully.
- Legacy profile JSON output command exits successfully.

## Definition of Done

- Stabilization checks are executed and passing.
- Any required fixes are minimal and limited to failing behavior.
- Results are documented in implementation execution notes.
