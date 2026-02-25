# Core TUI Redesign Design

## Goal

Replace the currently ported private-style TUI with a clean, modular `odin-core` dashboard that is:
- core-default and project-agnostic,
- responsive by terminal width,
- customizable via profiles/config,
- still able to run legacy behavior as opt-in.

## User Priorities

Required primary panels:
- inbox
- kanban board
- agents
- logs
- GitHub PR visibility

Additional constraints:
- preserve current command surface,
- modular internals for different user usage patterns,
- default should no longer feel like private/custom Odin.

## Architecture

New package path:
`scripts/odin/tui_core/`

Modules:
- `app.py`: CLI, live loop, profile selection, rendering orchestration
- `profiles.py`: built-in `core` and `legacy`, user config overrides
- `layout.py`: responsive compositor by terminal width
- `models.py`: normalized `PanelData` contracts
- `collectors/*.py`: data acquisition by domain
- `panels/*.py`: visual rendering by domain

Compatibility entrypoints:
- `scripts/odin/odin-tui.py`: thin entrypoint to modular core app
- `scripts/odin/odin-tui`: existing launcher kept intact
- `scripts/odin/odin-tui-legacy.py`: previous monolithic dashboard retained for `--profile legacy`

## Profiles

- `core` (default):
  - header
  - inbox
  - kanban
  - agents
  - logs
  - github
- `legacy`:
  - delegates to preserved old implementation

Override mechanism:
- `--config <json>` supports panel enable/disable/order and refresh interval.

## Responsive Layout

Breakpoints (terminal columns):
- `<100`: single-column stack
- `100-159`: two-column
- `>=160`: operations layout with split mid-section and bottom logs

## Error/Degradation Behavior

- Collector errors are panel-local and non-fatal.
- Missing `gh` CLI or auth only degrades GitHub panel.
- Startup failures limited to invalid CLI/config parse.

## Testing

- Python unit tests for:
  - profile resolution,
  - layout mode selection.
- Smoke checks:
  - `--json` for core and legacy,
  - snapshot run for core,
  - live startup (timeout harness) for core and wrapper.

## Acceptance Criteria

- `core` is default profile.
- Inbox, kanban, agents, logs, and GitHub panels render in `core`.
- Responsive layout transitions at defined breakpoints.
- Legacy profile remains available and functional.
- Existing commands still work.
