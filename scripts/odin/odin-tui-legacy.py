#!/usr/bin/env python3
"""Odin Agent Swarm TUI Dashboard.

Renders a two-column terminal dashboard:
  Left (60%):  Orchestrator status, agents, Kanban, inbox, activity, PRs, metrics
  Right (40%): Activity log (top) + Agent terminal viewer (bottom)

The agent terminal viewer shows cleaned real-time output from agent tmux sessions
with a tab bar for switching between agents. Tab 1 is always Odin.

Modes:
    Default:    One-shot snapshot (print and exit)
    --live/-l:  Rich Live display, refreshing every 5s
                Keys 1-9 switch agent tabs, q exits
    --json:     Raw JSON output for piping/scripting

Data sources (all read-only):
    /var/odin/heartbeat            Orchestrator heartbeat
    /var/odin/agents/*/status.json Agent statuses
    /var/odin/kanban/board.json    Kanban board state
    /var/odin/kanban/velocity.json Velocity metrics
    /var/odin/inbox/*.json         Pending tasks
    /var/odin/budgets/daily.json   Daily metrics
    /var/odin/budgets/limits.json  Budget limits
    /var/odin/outbox/*.json        Completed tasks (for today's count)
    /var/odin/logs/{date}/*.log    Activity event logs + agent terminal output
    gh pr list --json              Open GitHub PRs
"""

import argparse
import glob
import json
import os
import subprocess
import sys
import time
from collections import deque
from datetime import datetime, timedelta, timezone
from pathlib import Path
import re

try:
    from rich.console import Console, Group
    from rich.layout import Layout
    from rich.live import Live
    from rich.panel import Panel
    from rich.table import Table
    from rich.columns import Columns
    from rich.text import Text
    from rich import box
except ImportError:
    print("ERROR: Rich library required. Install: pip install rich", file=sys.stderr)
    sys.exit(1)

ODIN_DIR = os.environ.get("ODIN_DIR", "/var/odin")
SUBPROCESS_TIMEOUT = 10  # seconds — prevents blocking on slow GitHub API

# Structural dirs under agents/ that are not real sub-agents
AGENT_EXCLUDE = {"orchestrator", "self"}

# Activity log sources: (filename, display_tag, rich_color)
_LOG_SOURCES = [
    ("agents.log", "agent", "cyan"),
    ("inbox.log", "inbox", "green"),
    ("keepalive.log", "alive", "dim"),
    ("alerts.log", "alert", "yellow"),
    ("cost.log", "cost", "magenta"),
    ("ssh-dispatch.log", "ssh", "blue"),
]

# Regex: [tag] ISO-timestamp rest-of-message
_LOG_LINE_RE = re.compile(
    r"^\[([^\]]+)\]\s+"           # [tag]
    r"(\d{4}-\d{2}-\d{2}T"       # ISO date
    r"\d{2}:\d{2}:\d{2}"         # HH:MM:SS
    r"[^\s]*)\s+"                 # timezone offset
    r"(.+)$"                      # message
)

# Keepalive lines to KEEP (rest are filtered out as noise)
_KEEPALIVE_KEYWORDS = {"wake", "restart", "warn", "alert", "killed", "error", "nudge", "dead"}

# Error keywords that get highlighted red
_ERROR_KEYWORDS = {"ANTI-LOOP", "FATAL", "Force-killed", "ESCALATED", "ERROR", "FAILED"}

_ACTIVITY_MAX_LINES_PER_FILE = 200
_ACTIVITY_MAX_EVENTS = 15


# ─── Data collection ──────────────────────────────────────────────────


def _read_json(path: str) -> dict | list | None:
    """Safely read a JSON file, returning None on any error."""
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, json.JSONDecodeError):
        return None


def _file_age_seconds(path: str) -> float | None:
    """Return age of a file in seconds, or None if not found."""
    try:
        mtime = os.path.getmtime(path)
        return time.time() - mtime
    except OSError:
        return None


def _format_duration(seconds: float | None) -> str:
    """Format seconds into a human-readable duration string."""
    if seconds is None:
        return "—"
    s = int(seconds)
    if s < 60:
        return f"{s}s"
    if s < 3600:
        return f"{s // 60}m"
    hours = s // 3600
    mins = (s % 3600) // 60
    return f"{hours}h {mins:02d}m"


# Regex patterns for ANSI escape code stripping (pipe-pane logs contain raw terminal output)
_ANSI_ESCAPE_RE = re.compile(r"""
    \x1b\[[\?0-9;:]*[A-Za-z]  | # CSI sequences (colors, cursor, DEC private modes)
    \x1b\][^\x07\x1b]*(?:\x07|\x1b\\)  | # OSC sequences (BEL or ST terminated)
    \x1b\([A-Za-z]             | # Character set selection
    \x1b[>=]                   | # Keypad mode
    \x1b[78DEHM]               | # Single-char escape commands
    \x07                       | # Bell
    \x08                       | # Backspace
    [\x00-\x06\x0e-\x1a]      | # Remaining C0 control chars (except \t \n)
    \r                           # Carriage returns
""", re.VERBOSE)

# Post-strip cleanup: fragments left when byte boundary splits an escape sequence
# Requires at least one digit before the terminal letter (e.g. "42C", ";5;174m")
_ESCAPE_FRAGMENT_RE = re.compile(r"^[\?;:]*[0-9]+[;:0-9]*[A-Za-z]")


def _strip_ansi(text: str) -> str:
    """Strip ANSI escape codes and carriage returns from raw terminal output."""
    return _ANSI_ESCAPE_RE.sub("", text)


def _tmux_session_exists(session_name: str) -> bool:
    """Check if a tmux session exists."""
    try:
        result = subprocess.run(
            ["tmux", "has-session", "-t", session_name],
            capture_output=True,
            timeout=5,
        )
        return result.returncode == 0
    except (subprocess.TimeoutExpired, FileNotFoundError):
        return False


def collect_orchestrator() -> dict:
    """Collect orchestrator status data."""
    heartbeat_path = os.path.join(ODIN_DIR, "heartbeat")
    heartbeat_age = _file_age_seconds(heartbeat_path)
    tmux_alive = _tmux_session_exists("odin-orchestrator")

    if tmux_alive and heartbeat_age is not None and heartbeat_age < 120:
        status = "healthy"
    elif tmux_alive:
        status = "degraded"
    else:
        status = "dead"

    # Calculate uptime from heartbeat file creation or service start
    uptime = None
    try:
        heartbeat_content = Path(heartbeat_path).read_text().strip()
        # Try ISO format
        if heartbeat_content:
            uptime = heartbeat_age  # Approximate — real uptime from service
    except OSError:
        pass

    return {
        "status": status,
        "tmux_alive": tmux_alive,
        "heartbeat_age": heartbeat_age,
        "uptime": uptime,
    }


def _build_dispatch_map() -> dict[str, dict]:
    """Build agent→dispatch info from orchestrator state.json.

    Returns a dict mapping agent names to their active dispatch:
        {"qa": {"task_id": "...", "dispatched_at": "ISO-8601"}, ...}
    """
    state = _read_json(os.path.join(ODIN_DIR, "state.json")) or {}
    dispatch_map: dict[str, dict] = {}

    for task_id, info in (state.get("dispatched_tasks") or {}).items():
        agent = info.get("agent")
        if agent:
            dispatch_map[agent] = {
                "task_id": task_id,
                "dispatched_at": info.get("dispatched_at"),
            }

    return dispatch_map


def collect_agents() -> list[dict]:
    """Collect all agent statuses.

    Cross-references three data sources for accuracy:
        1. Per-agent status.json (role, base state)
        2. Global state.json dispatched_tasks (active task, dispatch time)
        3. tmux session liveness (ground truth for alive/dead)
    """
    agents_dir = os.path.join(ODIN_DIR, "agents")
    agents = []

    if not os.path.isdir(agents_dir):
        return agents

    dispatch_map = _build_dispatch_map()

    for agent_dir in sorted(glob.glob(os.path.join(agents_dir, "*"))):
        if not os.path.isdir(agent_dir):
            continue

        name = os.path.basename(agent_dir)

        # Skip structural directories that aren't real sub-agents
        if name in AGENT_EXCLUDE:
            continue

        status_file = os.path.join(agent_dir, "status.json")
        status_data = _read_json(status_file)

        tmux_alive = _tmux_session_exists(f"odin-{name}")
        dispatch = dispatch_map.get(name)

        if status_data is None:
            # Agent dir exists but no status.json
            if not tmux_alive and not dispatch:
                # Never been spawned — skip entirely
                continue

            # Scan agent dir for task clues: task-*.prompt or output-*.log
            fallback_task = None
            if not dispatch:
                agent_dir = os.path.join(ODIN_DIR, "agents", name)
                for pattern, prefix, suffix in [
                    ("task-*.prompt", "task-", ".prompt"),
                    ("output-*.log", "output-", ".log"),
                ]:
                    matches = glob.glob(os.path.join(agent_dir, pattern))
                    if matches:
                        latest = max(matches, key=os.path.getmtime)
                        fallback_task = os.path.basename(latest).removeprefix(prefix).removesuffix(suffix)
                        break

            has_task = dispatch or fallback_task
            state = "busy" if tmux_alive and has_task else (
                "idle" if tmux_alive else "dead"
            )
            agents.append({
                "name": name,
                "role": _derive_role("", name),
                "state": state,
                "task": dispatch["task_id"] if dispatch else fallback_task,
                "duration": _dispatch_duration(dispatch),
                "tmux": tmux_alive,
                "tasks_today": _count_agent_tasks_today(name),
            })
            continue

        # Derive role from prompt file path
        prompt_path = status_data.get("prompt", "")
        role = _derive_role(prompt_path, name)

        # Determine state: tmux liveness + dispatch map override stale status.json
        if tmux_alive and dispatch:
            state = "busy"
            task = dispatch["task_id"]
            duration = _dispatch_duration(dispatch)
        elif tmux_alive:
            state = "idle"
            task = None
            duration = None
        else:
            # tmux dead — use status.json as last known state
            file_state = status_data.get("status", "unknown")
            state = file_state if file_state == "stopped" else "dead"
            task = None
            duration = None

        # Count completed tasks today
        tasks_today = _count_agent_tasks_today(name)

        agents.append({
            "name": name,
            "role": role,
            "state": state,
            "task": task,
            "duration": duration,
            "tmux": tmux_alive,
            "tasks_today": tasks_today,
        })

    return agents


def _dispatch_duration(dispatch: dict | None) -> float | None:
    """Calculate seconds since dispatch, or None."""
    if not dispatch:
        return None
    dispatched_at = dispatch.get("dispatched_at")
    if not dispatched_at:
        return None
    try:
        dt = datetime.fromisoformat(dispatched_at)
        return (datetime.now(timezone.utc) - dt.replace(tzinfo=timezone.utc)).total_seconds()
    except (ValueError, TypeError):
        return None


def _derive_role(prompt_path: str, name: str) -> str:
    """Derive agent role from prompt file path or agent name."""
    # Management layer — check before generic "qa" match
    if "qa-lead" in prompt_path or name == "qa-lead":
        return "QA Lead"
    if "po.md" in prompt_path or name == "po":
        return "Product Owner"
    if "sm.md" in prompt_path or name == "sm":
        return "Scrum Master"
    if "tl.md" in prompt_path or name == "tl":
        return "Tech Lead"
    # Execution layer
    if "qa" in prompt_path or name == "qa":
        return "Reviewer"
    if "devops" in prompt_path or name == "devops":
        return "DevOps"
    if "security" in prompt_path or name == "security":
        return "Security"
    if "marketing" in prompt_path or name == "marketing":
        return "Marketing"
    if "sentry" in prompt_path:
        return "Sentry"
    if "worker" in prompt_path or "worker" in name:
        return "Worker"
    return "Agent"


TASK_TYPE_LABELS = {
    "issue_implement": "Impl",
    "pr_review_second": "2nd Review",
    "pr_review": "PR Review",
    "pr_fix": "PR Fix",
    "sentry_fix": "Sentry",
    "deploy_staging": "Deploy Stg",
    "deploy_prod": "Deploy Prod",
    "security_scan": "Sec Scan",
    "health_check": "Health",
    "dispatch_work": "Dispatch",
    "daily_standup": "Standup",
    "blocker_check": "Blockers",
    "backlog_groom": "Groom",
    "arch_review": "Arch Review",
    "arch_audit": "Arch Audit",
    "acceptance_test": "Accept Test",
    "test_strategy": "Test Strategy",
    "quality_gate": "Quality Gate",
    "retrospective": "Retro",
    "triage": "Triage",
    "spec_create": "Spec",
    "content_create": "Content",
    "self_heal": "Self-Heal",
    "self_improve": "Self-Improve",
}


def _pretty_task_name(task_id: str | None) -> str:
    """Convert a raw task_id into a short human-readable label.

    Examples:
        cognitive-issue_implement-1771857023-10740  →  Impl #10740
        cron-1771833600025-a826                     →  Dispatch
        n8n-1771857463875-c722                      →  PR Review
        issue-867-auth-refactor                     →  #867 auth-refactor
        self-heal-inbox_overflow-123                →  Self-Heal
    """
    if not task_id:
        return "—"

    # Try to extract task type from known patterns
    for ttype, label in TASK_TYPE_LABELS.items():
        if ttype in task_id:
            # Try to extract trailing issue/PR number
            parts = task_id.rsplit("-", 1)
            if parts[-1].isdigit():
                num = parts[-1]
                return f"{label} #{num}"
            return label

    # Pattern: issue-{number}-{slug}
    if task_id.startswith("issue-"):
        rest = task_id[6:]  # strip "issue-"
        dash = rest.find("-")
        if dash > 0 and rest[:dash].isdigit():
            num = rest[:dash]
            slug = rest[dash + 1:]
            if len(slug) > 14:
                slug = slug[:13] + "~"
            return f"#{num} {slug}"

    # Pattern: cron-{timestamp}-{hash} or n8n-{timestamp}-{hash} — already handled above
    # Fallback: truncate raw id
    if len(task_id) > 20:
        return task_id[:19] + "~"
    return task_id


def _count_agent_tasks_today(agent_name: str) -> int:
    """Count tasks handled today by an agent via output logs in its directory."""
    agent_dir = os.path.join(ODIN_DIR, "agents", agent_name)
    today = datetime.now().strftime("%Y-%m-%d")
    count = 0

    for f in glob.glob(os.path.join(agent_dir, "output-*.log")):
        try:
            mtime = os.path.getmtime(f)
            file_date = datetime.fromtimestamp(mtime).strftime("%Y-%m-%d")
            if file_date == today:
                count += 1
        except OSError:
            pass

    return count


def collect_inbox() -> list[dict]:
    """Collect pending inbox tasks."""
    inbox_dir = os.path.join(ODIN_DIR, "inbox")
    tasks = []

    if not os.path.isdir(inbox_dir):
        return tasks

    for f in sorted(glob.glob(os.path.join(inbox_dir, "*.json"))):
        if f.endswith(".tmp"):
            continue
        data = _read_json(f)
        if data is None:
            continue

        age = _file_age_seconds(f)
        tasks.append({
            "task_id": data.get("task_id", os.path.basename(f)),
            "type": data.get("type", "unknown"),
            "source": data.get("source", "unknown"),
            "age": age,
        })

    return tasks


def _build_agent_completion_map() -> dict[str, str]:
    """Build task_id→agent mapping from events.jsonl and agents.log."""
    agent_map: dict[str, str] = {}
    today = datetime.now().strftime("%Y-%m-%d")
    yesterday = datetime.fromtimestamp(time.time() - 86400).strftime("%Y-%m-%d")

    for date_str in [today, yesterday]:
        # Primary source: structured events (has agent field on dispatch/complete)
        events_path = os.path.join(ODIN_DIR, "logs", date_str, "events.jsonl")
        try:
            with open(events_path) as f:
                for line in f:
                    if '"agent"' not in line:
                        continue
                    try:
                        evt = json.loads(line)
                        tid = evt.get("task_id")
                        agent = evt.get("agent")
                        if tid and agent:
                            agent_map[tid] = agent
                    except (json.JSONDecodeError, KeyError):
                        pass
        except OSError:
            pass

        # Fallback: agents.log completion records
        log_path = os.path.join(ODIN_DIR, "logs", date_str, "agents.log")
        try:
            with open(log_path) as f:
                for line in f:
                    if "completed by agent" in line:
                        parts = line.split("'")
                        if len(parts) >= 4:
                            task_id = parts[1]
                            agent_name = parts[3]
                            agent_map[task_id] = agent_name
        except OSError:
            pass

    return agent_map


def collect_recent_activity() -> list[dict]:
    """Collect recently completed tasks from outbox with agent info."""
    outbox_dir = os.path.join(ODIN_DIR, "outbox")
    activities = []

    if not os.path.isdir(outbox_dir):
        return activities

    agent_map = _build_agent_completion_map()

    # Get outbox files sorted by modification time (newest first)
    files = sorted(
        glob.glob(os.path.join(outbox_dir, "*.json")),
        key=lambda f: os.path.getmtime(f),
        reverse=True,
    )

    for f in files[:10]:  # Last 10 completed tasks
        data = _read_json(f)
        if data is None:
            continue

        task_id = data.get("task_id", os.path.basename(f).replace(".json", ""))
        completed_at = data.get("completed_at", "")

        # Get status: top-level first (self-processed), then nested result (agent done-files)
        result = data.get("result", {})
        status = data.get("status") or ""
        if not status:
            status = result.get("status", "unknown") if isinstance(result, dict) else "unknown"

        # Get agent from completion map, then outbox data, then "odin" for self-tasks
        agent = agent_map.get(task_id) or data.get("agent") or "odin"

        # Calculate age
        age = _file_age_seconds(f)

        # Extract PR number if present (check both nesting levels)
        pr_number = None
        if isinstance(result, dict):
            pr_number = result.get("pr_number")
            inner_result = result.get("result", {})
            if not pr_number and isinstance(inner_result, dict):
                pr_number = inner_result.get("pr_number")
        if not pr_number:
            pr_number = data.get("payload", {}).get("pr_number") if isinstance(data.get("payload"), dict) else None

        activities.append({
            "task_id": task_id,
            "status": status,
            "agent": agent,
            "age": age,
            "pr_number": pr_number,
        })

    return activities


def collect_prs() -> list[dict]:
    """Collect open GitHub PRs via gh CLI."""
    try:
        result = subprocess.run(
            [
                "gh", "pr", "list", "--json",
                "number,title,statusCheckRollup,reviewDecision,latestReviews,author",
                "--limit", "10",
            ],
            capture_output=True,
            text=True,
            timeout=SUBPROCESS_TIMEOUT,
        )
        if result.returncode != 0:
            return []

        prs = json.loads(result.stdout)
        out = []
        for pr in prs:
            # Determine CI status
            checks = pr.get("statusCheckRollup", []) or []
            ci = _summarize_checks(checks)

            # Review status — reviewDecision first, fall back to latestReviews
            review = pr.get("reviewDecision", "") or ""
            if review:
                review_icon = {
                    "APPROVED": "pass",
                    "CHANGES_REQUESTED": "fail",
                    "REVIEW_REQUIRED": "pending",
                }.get(review, "none")
            else:
                # reviewDecision is null — check actual reviews
                latest = pr.get("latestReviews", []) or []
                review_icon = "none"
                for r in latest:
                    state = (r.get("state") or "").upper()
                    if state == "APPROVED":
                        review_icon = "pass"
                        break
                    elif state == "CHANGES_REQUESTED":
                        review_icon = "fail"
                        break
                    elif state in ("COMMENTED", "PENDING"):
                        review_icon = "pending"

            out.append({
                "number": pr.get("number"),
                "title": pr.get("title", ""),
                "ci": ci,
                "review": review_icon,
                "author": (pr.get("author") or {}).get("login", ""),
            })
        return out
    except (subprocess.TimeoutExpired, FileNotFoundError, json.JSONDecodeError):
        return []


def _summarize_checks(checks: list) -> str:
    """Summarize CI check status to a single state.

    Handles both CheckRun (conclusion/status) and StatusContext (state) objects
    from GitHub's statusCheckRollup. All values normalized to lowercase.
    """
    if not checks:
        return "none"

    FAIL_STATES = {"failure", "error", "action_required", "startup_failure", "timed_out", "cancelled"}
    PASS_STATES = {"success", "neutral", "skipped"}

    has_fail = False
    all_pass = True

    for c in checks:
        # CheckRun uses conclusion (when done) or status (when running)
        # StatusContext uses state
        val = (c.get("conclusion") or c.get("status") or c.get("state") or "").lower()
        if val in FAIL_STATES:
            has_fail = True
        if val not in PASS_STATES:
            all_pass = False

    if has_fail:
        return "fail"
    if all_pass:
        return "pass"
    return "pending"


def collect_metrics() -> dict:
    """Collect daily budget metrics."""
    daily = _read_json(os.path.join(ODIN_DIR, "budgets", "daily.json")) or {}
    limits = _read_json(os.path.join(ODIN_DIR, "budgets", "limits.json")) or {}

    # Disk usage
    disk_pct = None
    try:
        result = subprocess.run(
            ["df", ODIN_DIR],
            capture_output=True,
            text=True,
            timeout=5,
        )
        if result.returncode == 0:
            lines = result.stdout.strip().split("\n")
            if len(lines) >= 2:
                parts = lines[1].split()
                for p in parts:
                    if p.endswith("%"):
                        disk_pct = int(p.rstrip("%"))
                        break
    except (subprocess.TimeoutExpired, ValueError):
        pass

    # Count active agents (exclude structural dirs)
    agents_dir = os.path.join(ODIN_DIR, "agents")
    active_agents = 0
    if os.path.isdir(agents_dir):
        for d in os.listdir(agents_dir):
            if d in AGENT_EXCLUDE:
                continue
            if os.path.isdir(os.path.join(agents_dir, d)):
                if _tmux_session_exists(f"odin-{d}"):
                    active_agents += 1

    # Sentry state
    sentry_unresolved = None
    sentry_critical = None
    sentry_state_path = os.path.join(ODIN_DIR, "sentry-state.json")
    if os.path.exists(sentry_state_path):
        sentry_data = _read_json(sentry_state_path)
        if sentry_data:
            sentry_unresolved = sentry_data.get("unresolved_total", 0)
            sentry_critical = sentry_data.get("critical_count", 0)

    # PR health
    pr_health_file = os.path.join(ODIN_DIR, "pr-health.json")
    if os.path.exists(pr_health_file):
        try:
            with open(pr_health_file) as f:
                pr_data = json.load(f)
            pr_open = pr_data.get("total_open", 0)
            pr_conflicting = len(pr_data.get("conflicting", []))
            pr_behind = len(pr_data.get("behind", []))
        except (json.JSONDecodeError, OSError):
            pr_open = 0
            pr_conflicting = 0
            pr_behind = 0
    else:
        pr_open = 0
        pr_conflicting = 0
        pr_behind = 0

    return {
        "sessions_created": daily.get("sessions_created", 0),
        "max_daily_sessions": limits.get("max_daily_sessions", 100),
        "tasks_dispatched": daily.get("tasks_dispatched", 0),
        "tasks_completed": daily.get("tasks_completed", 0),
        "max_tasks": limits.get("max_daily_tasks", 200),
        "active_agents": active_agents,
        "max_agents": limits.get("max_concurrent_agents", 6),
        "disk_pct": disk_pct,
        "self_improve_count": daily.get("self_improve_count", 0),
        "sentry_unresolved": sentry_unresolved,
        "sentry_critical": sentry_critical,
        "pr_open": pr_open,
        "pr_conflicting": pr_conflicting,
        "pr_behind": pr_behind,
    }


def collect_kanban() -> dict:
    """Collect Kanban board state and velocity metrics.

    Velocity is computed live from the done column timestamps in board.json
    rather than reading the potentially-stale velocity.json file.
    """
    board = _read_json(os.path.join(ODIN_DIR, "kanban", "board.json")) or {}

    columns = board.get("columns", {})
    col_order = ["backlog", "ready", "in_progress", "in_review", "done"]

    summary = []
    for col in col_order:
        col_data = columns.get(col, {})
        items = col_data.get("items", [])
        wip_limit = col_data.get("wip_limit", 0)
        summary.append({
            "name": col,
            "count": len(items),
            "wip_limit": wip_limit,
            "items": [
                {
                    "issue_number": it.get("issue_number"),
                    "title": it.get("title", ""),
                    "priority": it.get("priority", ""),
                }
                for it in items
            ],
        })

    # Compute velocity live from done column timestamps
    done_items = columns.get("done", {}).get("items", [])
    now = datetime.now(timezone.utc)
    seven_days_ago = now - timedelta(days=7)

    recent_count = 0
    total_lead_hours = 0
    for item in done_items:
        entered = item.get("entered_column_at", "")
        if not entered:
            continue
        try:
            entered_dt = datetime.fromisoformat(entered)
        except (ValueError, TypeError):
            continue
        if entered_dt >= seven_days_ago:
            recent_count += 1
            created = item.get("created_at", "")
            if created:
                try:
                    created_dt = datetime.fromisoformat(created)
                    lead_h = (entered_dt - created_dt).total_seconds() / 3600
                    total_lead_hours += lead_h
                except (ValueError, TypeError):
                    pass

    items_per_day = round(recent_count / 7, 1) if recent_count else 0
    avg_lead = round(total_lead_hours / recent_count) if recent_count else 0

    return {
        "columns": summary,
        "velocity": {
            "items_per_day": items_per_day,
            "avg_lead_time_hours": avg_lead,
            "items_completed": recent_count,
        },
        "updated_at": board.get("updated_at", ""),
    }


def collect_activity_log() -> list[dict]:
    """Collect unified activity log from events.jsonl structured event bus.

    Reads the last 500 lines of events.jsonl, parses JSON, filters debug events,
    and returns the 15 most recent events (newest first).
    Falls back to legacy plaintext log reading if events.jsonl is empty/missing.
    """
    today = datetime.now().strftime("%Y-%m-%d")
    log_dir = os.path.join(ODIN_DIR, "logs", today)
    events_file = os.path.join(log_dir, "events.jsonl")

    events: list[dict] = []

    # Try structured events.jsonl first
    try:
        with open(events_file) as f:
            tail = deque(f, maxlen=500)
    except OSError:
        tail = []

    if tail:
        for line in tail:
            line = line.strip()
            if not line:
                continue
            try:
                ev = json.loads(line)
            except json.JSONDecodeError:
                continue

            level = ev.get("level", "info")
            if level == "debug":
                continue

            ts_str = ev.get("ts", "")
            try:
                dt = datetime.fromisoformat(ts_str)
                time_str = dt.strftime("%H:%M")
                sort_key = dt.isoformat()
            except (ValueError, TypeError):
                time_str = "??:??"
                sort_key = "0000"

            component = ev.get("component", "?")
            msg = ev.get("msg", "")

            level_colors = {
                "info": "dim",
                "warn": "yellow",
                "error": "red",
                "critical": "red bold",
            }
            color = level_colors.get(level, "dim")

            tag_map = {
                "task-queue": "task",
                "agent-lifecycle": "agent",
                "agent-supervisor": "super",
                "alert-router": "alert",
                "kanban": "kanban",
                "cognitive": "think",
                "cost-tracker": "cost",
                "self-improve": "self",
                "telegram": "tg",
                "memory-sync": "mem",
                "health-check": "health",
                "keepalive": "alive",
                "adapter-claude": "claude",
                "adapter-codex": "codex",
            }
            tag = tag_map.get(component, component[:6])

            is_error = level in ("error", "critical")

            events.append({
                "time": time_str,
                "tag": tag,
                "color": color,
                "message": msg,
                "sort_key": sort_key,
                "is_error": is_error,
            })

        events.sort(key=lambda e: e["sort_key"], reverse=True)
        return events[:_ACTIVITY_MAX_EVENTS]

    # Fallback: legacy plaintext log reading
    return _collect_activity_log_legacy()


def _collect_activity_log_legacy() -> list[dict]:
    """Legacy fallback: read from individual plaintext log files."""
    today = datetime.now().strftime("%Y-%m-%d")
    log_dir = os.path.join(ODIN_DIR, "logs", today)
    events: list[dict] = []

    for filename, tag, color in _LOG_SOURCES:
        filepath = os.path.join(log_dir, filename)
        try:
            with open(filepath) as f:
                tail = deque(f, maxlen=_ACTIVITY_MAX_LINES_PER_FILE)
        except OSError:
            continue

        for line in tail:
            line = line.rstrip("\n")
            m = _LOG_LINE_RE.match(line)
            if not m:
                continue

            _raw_tag, timestamp_str, message = m.group(1), m.group(2), m.group(3)

            if filename == "keepalive.log":
                lower_msg = message.lower()
                if not any(kw in lower_msg for kw in _KEEPALIVE_KEYWORDS):
                    continue

            if filename == "ssh-dispatch.log":
                if message.startswith("Serving "):
                    continue

            try:
                dt = datetime.fromisoformat(timestamp_str)
                time_str = dt.strftime("%H:%M")
                sort_key = dt.isoformat()
            except ValueError:
                time_str = "??:??"
                sort_key = "0000"

            is_error = any(kw in message for kw in _ERROR_KEYWORDS)

            events.append({
                "time": time_str,
                "tag": tag,
                "color": color,
                "message": message,
                "sort_key": sort_key,
                "is_error": is_error,
            })

    events.sort(key=lambda e: e["sort_key"], reverse=True)
    return events[:_ACTIVITY_MAX_EVENTS]


_TERMINAL_TAIL_BYTES = 2000
_TERMINAL_MAX_LINES = 20


def collect_agent_terminal(selected_tab: int = 1) -> dict:
    """Collect structured event stream for the selected agent tab.

    Tab 1 (Odin): orchestrator-level events (task-queue, kanban, cognitive, etc.)
    Tabs 2-9: per-agent events filtered by agent field from events.jsonl
    Falls back to raw pipe-pane output if events.jsonl is empty/missing.
    """
    today = datetime.now().strftime("%Y-%m-%d")
    log_dir = os.path.join(ODIN_DIR, "logs", today)
    events_file = os.path.join(log_dir, "events.jsonl")

    # Tab 1: Odin orchestrator (always present)
    tabs = [{"index": 1, "name": "Odin", "alive": _tmux_session_exists("odin-orchestrator")}]

    # Tabs 2-9: Active agents sorted by name
    agents_dir = os.path.join(ODIN_DIR, "agents")
    if os.path.isdir(agents_dir):
        agent_names = []
        for d in sorted(os.listdir(agents_dir)):
            if d in AGENT_EXCLUDE or not os.path.isdir(os.path.join(agents_dir, d)):
                continue
            agent_names.append(d)

        for i, name in enumerate(agent_names[:8], start=2):
            alive = _tmux_session_exists(f"odin-{name}")
            tabs.append({"index": i, "name": name, "alive": alive})

    # Clamp selected tab
    if selected_tab < 1 or selected_tab > len(tabs):
        selected_tab = 1

    tab_info = tabs[selected_tab - 1]
    raw_name = tab_info["name"]

    # Try structured events.jsonl first
    lines: list[str] = []
    try:
        with open(events_file) as f:
            tail = deque(f, maxlen=1000)
    except OSError:
        tail = []

    if tail:
        odin_components = {"task-queue", "kanban", "cognitive", "memory-sync", "keepalive", "health-check"}

        for raw_line in tail:
            raw_line = raw_line.strip()
            if not raw_line:
                continue
            try:
                ev = json.loads(raw_line)
            except json.JSONDecodeError:
                continue

            # Filter logic
            if raw_name == "Odin":
                if ev.get("component", "") not in odin_components:
                    continue
            else:
                if ev.get("agent", "") != raw_name:
                    continue

            ts_str = ev.get("ts", "")
            try:
                dt = datetime.fromisoformat(ts_str)
                time_str = dt.strftime("%H:%M:%S")
            except (ValueError, TypeError):
                time_str = "??:??:??"

            level = ev.get("level", "info").upper()
            event_name = ev.get("event", "?")
            msg = ev.get("msg", "")

            lines.append(f"{time_str} {level:<5s} {event_name}: {msg}")

        lines = lines[-_TERMINAL_MAX_LINES:]

    if not lines:
        lines = _collect_agent_terminal_legacy(raw_name, log_dir)

    return {
        "tabs": tabs,
        "selected": selected_tab,
        "lines": lines,
    }


def _collect_agent_terminal_legacy(raw_name: str, log_dir: str) -> list[str]:
    """Legacy fallback: read raw pipe-pane terminal output."""
    if raw_name == "Odin":
        log_file = os.path.join(log_dir, "odin.log")
    else:
        log_file = os.path.join(log_dir, f"{raw_name}.log")

    lines: list[str] = []
    try:
        with open(log_file, "rb") as f:
            f.seek(0, 2)
            size = f.tell()
            start = max(0, size - _TERMINAL_TAIL_BYTES)
            f.seek(start)
            raw = f.read().decode("utf-8", errors="replace")
            if start > 0:
                nl = raw.find("\n")
                if nl >= 0:
                    raw = raw[nl + 1:]

        cleaned = _strip_ansi(raw)
        for line in cleaned.split("\n"):
            stripped = line.strip()
            if not stripped:
                continue
            stripped = _ESCAPE_FRAGMENT_RE.sub("", stripped).strip()
            stripped = stripped.lstrip("⏵⏷✻✶✢✽✿·*†●○◉◎⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏↑↓←→▶▷◆◇★☆✦✧✩ ")
            if not stripped or len(stripped) <= 2:
                continue
            lower = stripped.lower()
            if any(noise in lower for noise in (
                "bypasspermission", "shift+tab", "esctointerrupt",
                "esctocancelpermission", "allowfortheentire",
                "ctrl+o to expand", "ctrl+c to cancel",
                "(shift+tab", "permission",
            )):
                continue
            stripped = stripped.rstrip("↑↓←→ ")
            lines.append(stripped)

        lines = lines[-_TERMINAL_MAX_LINES:]
    except OSError:
        lines = ["No log available"]

    return lines


def collect_all(selected_tab: int = 1) -> dict:
    """Collect all dashboard data."""
    return {
        "orchestrator": collect_orchestrator(),
        "agents": collect_agents(),
        "kanban": collect_kanban(),
        "inbox": collect_inbox(),
        "recent_activity": collect_recent_activity(),
        "prs": collect_prs(),
        "metrics": collect_metrics(),
        "activity_log": collect_activity_log(),
        "agent_terminal": collect_agent_terminal(selected_tab),
        "collected_at": datetime.now(timezone.utc).isoformat(),
    }


# ─── Rich rendering ──────────────────────────────────────────────────


STATE_COLORS = {
    "idle": "green",
    "busy": "yellow",
    "stopped": "dim",
    "dead": "red",
    "unknown": "dim red",
}

CI_ICONS = {"pass": "[green]OK[/]", "fail": "[red]FAIL[/]", "pending": "[yellow]...[/]", "none": "—"}
REVIEW_ICONS = {"pass": "[green]OK[/]", "fail": "[red]CHG[/]", "pending": "[yellow]REQ[/]", "none": "—"}


def _build_odin_blurb(data: dict) -> str:
    """Build a one-line summary of what Odin is doing from dispatched tasks."""
    state = _read_json(os.path.join(ODIN_DIR, "state.json")) or {}
    dispatched = state.get("dispatched_tasks", {})

    if not dispatched:
        inbox_count = len(data.get("inbox", []))
        if inbox_count > 0:
            return f"[yellow]Inbox: {inbox_count} pending[/yellow]"
        return "[dim]Idle — no active tasks[/dim]"

    # Group by role
    role_tasks: dict[str, list[str]] = {}
    for task_id, info in dispatched.items():
        agent = info.get("agent", "?")
        role = _derive_role("", agent.replace("worker-", "worker"))
        pretty = _pretty_task_name(task_id)
        role_tasks.setdefault(role, []).append(pretty)

    parts = []
    for role, tasks in role_tasks.items():
        if len(tasks) == 1:
            parts.append(f"{role}: {tasks[0]}")
        else:
            parts.append(f"{len(tasks)} {role}s")

    # Last cognitive scan age
    last_scan = state.get("last_cognitive_scan")
    scan_str = ""
    if last_scan:
        try:
            dt = datetime.fromisoformat(last_scan)
            age = (datetime.now(timezone.utc) - dt.replace(tzinfo=timezone.utc)).total_seconds()
            scan_str = f"  [dim]| Last scan: {_format_duration(age)} ago[/dim]"
        except (ValueError, TypeError):
            pass

    return " · ".join(parts) + scan_str


def render_header(data: dict) -> Panel:
    """Render orchestrator status header."""
    orch = data["orchestrator"]
    status = orch["status"]

    if status == "healthy":
        icon = "[green]●[/green]"
        label = "[green]Healthy[/green]"
    elif status == "degraded":
        icon = "[yellow]●[/yellow]"
        label = "[yellow]Degraded[/yellow]"
    else:
        icon = "[red]●[/red]"
        label = "[red]Dead[/red]"

    hb_age = _format_duration(orch["heartbeat_age"]) if orch["heartbeat_age"] is not None else "N/A"

    line1 = f"  Status: {icon} {label}   Heartbeat: {hb_age} ago"
    blurb = _build_odin_blurb(data)
    line2 = f"  [bold]Focus:[/bold] {blurb}"

    return Panel(
        Text.from_markup(f"{line1}\n{line2}"),
        title="[bold]Odin Agent Swarm[/bold]",
        border_style="blue",
    )


def render_agents(data: dict) -> Panel:
    """Render agents table."""
    table = Table(box=box.SIMPLE_HEAVY, expand=True, pad_edge=False)
    table.add_column("Name", style="bold", min_width=10)
    table.add_column("Role", min_width=8)
    table.add_column("State", min_width=7)
    table.add_column("Task", min_width=12, max_width=20)
    table.add_column("Duration", min_width=8, justify="right")
    table.add_column("Tmux", min_width=4, justify="center")
    table.add_column("Tasks", min_width=5, justify="right", header_style="bold yellow")

    for agent in data["agents"]:
        state = agent["state"]
        color = STATE_COLORS.get(state, "white")
        state_text = f"[{color}]{state}[/{color}]"

        tmux_icon = "[green]●[/green]" if agent["tmux"] else "[red]●[/red]"
        task = _pretty_task_name(agent["task"])

        table.add_row(
            agent["name"],
            agent["role"],
            state_text,
            task,
            _format_duration(agent["duration"]),
            tmux_icon,
            str(agent["tasks_today"]),
        )

    if not data["agents"]:
        table.add_row("—", "—", "—", "—", "—", "—", "—")

    return Panel(table, title=f"[bold]Agents ({len(data['agents'])})[/bold]", border_style="cyan")


def render_kanban(data: dict) -> Panel:
    """Render Kanban board panel with column counts and WIP status."""
    kanban = data.get("kanban", {})
    columns = kanban.get("columns", [])
    velocity = kanban.get("velocity", {})

    table = Table(box=box.SIMPLE_HEAVY, expand=True, pad_edge=False)
    table.add_column("Column", style="bold", min_width=12)
    table.add_column("Items", min_width=5, justify="right")
    table.add_column("WIP", min_width=7, justify="center")
    table.add_column("Top Items", min_width=25, max_width=50)

    col_labels = {
        "backlog": "Backlog",
        "ready": "Ready",
        "in_progress": "In Progress",
        "in_review": "In Review",
        "done": "Done",
    }

    total_items = 0
    for col in columns:
        name = col["name"]
        count = col["count"]
        wip = col["wip_limit"]
        items = col.get("items", [])
        total_items += count

        # WIP status indicator
        if wip == 0:
            wip_str = "[dim]∞[/dim]"
        elif count >= wip:
            wip_str = f"[red]{count}/{wip}[/red]"
        elif count >= wip - 1:
            wip_str = f"[yellow]{count}/{wip}[/yellow]"
        else:
            wip_str = f"[green]{count}/{wip}[/green]"

        # Show top 3 items for active columns
        item_strs = []
        for it in items[:3]:
            num = it.get("issue_number", "?")
            title = it.get("title", "")
            prio = it.get("priority", "")
            if len(title) > 20:
                title = title[:19] + "~"
            prio_color = {"P0": "red", "P1": "yellow", "P2": "white", "P3": "dim"}.get(prio, "dim")
            item_strs.append(f"[{prio_color}]{prio}[/{prio_color}] #{num} {title}")
        if count > 3:
            item_strs.append(f"[dim]+{count - 3} more[/dim]")
        top_items = ", ".join(item_strs) if item_strs else "[dim]empty[/dim]"

        table.add_row(col_labels.get(name, name), str(count), wip_str, top_items)

    # Velocity footer
    ipd = velocity.get("items_per_day", 0)
    completed = velocity.get("items_completed", 0)
    lead_time = velocity.get("avg_lead_time_hours", 0)
    footer = f"  Velocity: {ipd}/day  |  7d completed: {completed}  |  Avg lead: {lead_time}h"

    content = Group(table, Text.from_markup(footer))

    # Show when board was last updated
    updated_at = kanban.get("updated_at", "")
    updated_suffix = ""
    if updated_at:
        try:
            dt = datetime.fromisoformat(updated_at)
            age = (datetime.now(timezone.utc) - dt.replace(tzinfo=timezone.utc)).total_seconds()
            updated_suffix = f" | updated {_format_duration(age)} ago"
        except (ValueError, TypeError):
            pass

    return Panel(
        content,
        title=f"[bold]Kanban Board ({total_items} items{updated_suffix})[/bold]",
        border_style="cyan",
    )


def render_inbox(data: dict) -> Panel:
    """Render inbox tasks table."""
    table = Table(box=box.SIMPLE_HEAVY, expand=True, pad_edge=False)
    table.add_column("Task ID", style="bold", min_width=16, max_width=24)
    table.add_column("Type", min_width=12)
    table.add_column("Source", min_width=7)
    table.add_column("Age", min_width=6, justify="right")

    for task in data["inbox"]:
        task_id = task["task_id"]
        if len(task_id) > 22:
            task_id = task_id[:21] + "~"

        table.add_row(
            task_id,
            task["type"],
            task["source"],
            _format_duration(task["age"]),
        )

    if not data["inbox"]:
        table.add_row("[dim]empty[/dim]", "—", "—", "—")

    count = len(data["inbox"])
    title_color = "yellow" if count > 0 else "green"
    return Panel(
        table,
        title=f"[bold]Inbox ([{title_color}]{count} pending[/{title_color}])[/bold]",
        border_style="cyan",
    )


def render_recent_activity(data: dict) -> Panel:
    """Render recent activity table (completed tasks with agent info)."""
    table = Table(box=box.SIMPLE_HEAVY, expand=True, pad_edge=False)
    table.add_column("Task ID", style="bold", min_width=16, max_width=28)
    table.add_column("Agent", min_width=8)
    table.add_column("Status", min_width=7)
    table.add_column("PR", min_width=4, justify="right")
    table.add_column("Completed", min_width=6, justify="right")

    for item in data.get("recent_activity", []):
        task_id = _pretty_task_name(item["task_id"])

        status = item["status"]
        status_color = {
            "success": "green", "completed": "green", "resolved": "green",
            "dispatched": "cyan", "grouped": "cyan",
            "failure": "red", "partial": "yellow",
        }.get(status, "dim")
        status_text = f"[{status_color}]{status}[/{status_color}]"

        pr = f"#{item['pr_number']}" if item.get("pr_number") else "—"

        table.add_row(
            task_id,
            item.get("agent", "—"),
            status_text,
            pr,
            _format_duration(item.get("age")),
        )

    if not data.get("recent_activity"):
        table.add_row("[dim]none[/dim]", "—", "—", "—", "—")

    return Panel(
        table,
        title=f"[bold]Recent Activity ({len(data.get('recent_activity', []))})[/bold]",
        border_style="cyan",
    )


def render_prs(data: dict) -> Panel:
    """Render GitHub PRs table."""
    table = Table(box=box.SIMPLE_HEAVY, expand=True, pad_edge=False)
    table.add_column("PR", style="bold", min_width=5, justify="right")
    table.add_column("Title", min_width=20, max_width=40)
    table.add_column("CI", min_width=4, justify="center")
    table.add_column("Review", min_width=6, justify="center")
    table.add_column("Author", min_width=8)

    for pr in data["prs"]:
        title = pr["title"]
        if len(title) > 38:
            title = title[:37] + "~"

        table.add_row(
            f"#{pr['number']}",
            title,
            CI_ICONS.get(pr["ci"], "—"),
            REVIEW_ICONS.get(pr["review"], "—"),
            pr["author"],
        )

    if not data["prs"]:
        table.add_row("—", "[dim]No open PRs[/dim]", "—", "—", "—")

    return Panel(
        table,
        title=f"[bold]GitHub PRs ({len(data['prs'])} open)[/bold]",
        border_style="cyan",
    )


def render_metrics(data: dict) -> Panel:
    """Render compact daily metrics panel with AGILE-relevant stats."""
    m = data["metrics"]
    dispatched = m["tasks_dispatched"]
    completed = m["tasks_completed"]
    agents = m["active_agents"]
    max_agents = m["max_agents"]
    disk = m["disk_pct"]

    # Kanban column counts
    kanban = data.get("kanban", {})
    cols = {c["name"]: c["count"] for c in kanban.get("columns", [])}
    ready = cols.get("ready", 0)
    in_prog = cols.get("in_progress", 0)
    in_rev = cols.get("in_review", 0)

    disk_str = f"{disk}%" if disk is not None else "N/A"
    disk_color = "green" if disk is not None and disk < 80 else ("yellow" if disk is not None and disk < 90 else "red")

    sentry_u = m.get("sentry_unresolved")
    sentry_c = m.get("sentry_critical")
    if sentry_u is not None:
        sentry_color = "green" if sentry_u == 0 else ("red" if sentry_c and sentry_c > 0 else "yellow")
        sentry_str = f"[{sentry_color}]{sentry_u}[/{sentry_color}]"
        if sentry_c and sentry_c > 0:
            sentry_str += f" ([red]{sentry_c} crit[/red])"
    else:
        sentry_str = "[dim]N/A[/dim]"

    # PR health
    pr_open = m.get("pr_open", 0)
    pr_conflicting = m.get("pr_conflicting", 0)
    pr_behind = m.get("pr_behind", 0)
    if pr_conflicting > 0:
        pr_color = "red"
    elif pr_behind > 0:
        pr_color = "yellow"
    else:
        pr_color = "green"
    pr_text = f"[{pr_color}]{pr_open}[/{pr_color}]"
    if pr_conflicting > 0:
        pr_text += f" ([red]{pr_conflicting} conflict[/red])"
    if pr_behind > 0:
        pr_text += f" ([yellow]{pr_behind} behind[/yellow])"

    summary = (
        f"  Dispatched: {dispatched}  Done: {completed}  "
        f"| Agents: {agents}/{max_agents}  "
        f"| Kanban: [cyan]{ready}[/cyan]R [yellow]{in_prog}[/yellow]P [magenta]{in_rev}[/magenta]V  "
        f"| Disk: [{disk_color}]{disk_str}[/{disk_color}]  "
        f"| Sentry: {sentry_str}  "
        f"| PRs: {pr_text}"
    )

    return Panel(Text.from_markup(summary), title="[bold]Metrics[/bold]", border_style="cyan")


def render_activity_log(data: dict) -> Panel:
    """Render unified activity log panel for the right column."""
    events = data.get("activity_log", [])
    lines: list[str] = []

    for ev in events:
        tag = ev["tag"]
        color = ev["color"]
        msg = ev["message"]
        time_str = ev["time"]

        max_msg = 45
        if len(msg) > max_msg:
            msg = msg[: max_msg - 1] + "~"

        if ev.get("is_error"):
            lines.append(f"[red]{time_str}  {tag:<7s} {msg}[/red]")
        else:
            lines.append(f"[dim]{time_str}[/dim]  [{color}]{tag:<7s}[/{color}] {msg}")

    if not lines:
        lines.append("[dim]No recent activity[/dim]")

    count = len(events)
    content = Text.from_markup("\n".join(lines))

    return Panel(
        content,
        title=f"[bold]Log ({count})[/bold]",
        border_style="green",
    )


_TAB_SHORT_NAMES = {
    "Odin": "Od",
    "po": "PO",
    "sm": "SM",
    "tl": "TL",
    "qa-lead": "QA",
    "devops": "DO",
    "ops": "Op",
    "security": "Se",
    "marketing": "Mk",
}


def render_agent_terminal(data: dict) -> Panel:
    """Render tabbed agent terminal viewer panel."""
    terminal = data.get("agent_terminal", {})
    tabs = terminal.get("tabs", [])
    selected = terminal.get("selected", 1)
    lines = terminal.get("lines", [])

    # Build compact tab bar — fits on one line
    tab_parts: list[str] = []
    for tab in tabs:
        idx = tab["index"]
        name = tab["name"]
        alive = tab.get("alive", True)
        # Use 2-char abbreviation, fall back to first 2 chars
        short = _TAB_SHORT_NAMES.get(name, name.replace("worker-", "W")[:2])
        dead = "×" if not alive else ""
        label = f"{idx}·{short}{dead}"
        if idx == selected:
            tab_parts.append(f"[bold white on blue]{label}[/bold white on blue]")
        else:
            tab_parts.append(f"[dim]{label}[/dim]")
    tab_bar = " ".join(tab_parts)

    # Build terminal content
    content_lines: list[str] = []
    content_lines.append(tab_bar)

    if not lines:
        content_lines.append("[dim]No output[/dim]")
    else:
        for line in lines:
            # Skip lines that are just box-drawing, spinners, or single control chars
            stripped = line.strip("─│┌┐└┘├┤┬┴┼━ ⏵⏷●✻✶✢·*†�╭╮╰╯░▒▓")
            if not stripped or len(line) <= 2:
                continue
            # Truncate long lines (approximate panel width ~50 chars)
            if len(line) > 55:
                line = line[:54] + "~"
            # Escape Rich markup in raw terminal output
            line = line.replace("[", "\\[")
            content_lines.append(line)

    content = Text.from_markup("\n".join(content_lines))

    selected_name = "?"
    if tabs and 1 <= selected <= len(tabs):
        selected_name = tabs[selected - 1]["name"]

    return Panel(
        content,
        title=f"[bold]Terminal: {selected_name}[/bold]",
        border_style="magenta",
    )


def render_dashboard(data: dict, width: int = 80, height: int = 24) -> Layout | Group:
    """Build responsive dashboard based on terminal dimensions.

    XL     (≥65h, ≥120w): Two-column — all panels left, log+terminal right
    Large  (≥50h, ≥100w): Two-column — core panels left, log+terminal right
    Medium (≥35h, ≥80w):  Two-column — essential panels left, log+terminal right
    Small  (<35h or <80w): Single column — all panels stacked sequentially
    """
    if width >= 120 and height >= 65:
        # XL: full two-column layout — all 7 left panels
        layout = Layout()
        left = Group(
            render_header(data),
            render_agents(data),
            render_kanban(data),
            render_inbox(data),
            render_recent_activity(data),
            render_prs(data),
            render_metrics(data),
        )
        right = Group(
            render_activity_log(data),
            render_agent_terminal(data),
        )
        layout.split_row(
            Layout(left, name="left", ratio=3),
            Layout(right, name="right", ratio=2),
        )
        return layout

    if width >= 100 and height >= 50:
        # Large: two-column, 6 left panels (drop Metrics only)
        layout = Layout()
        left = Group(
            render_header(data),
            render_agents(data),
            render_kanban(data),
            render_inbox(data),
            render_recent_activity(data),
            render_prs(data),
        )
        right = Group(
            render_activity_log(data),
            render_agent_terminal(data),
        )
        layout.split_row(
            Layout(left, name="left", ratio=3),
            Layout(right, name="right", ratio=2),
        )
        return layout

    if width >= 80 and height >= 35:
        # Medium: two-column, 5 left panels (drop Inbox, Metrics)
        layout = Layout()
        left = Group(
            render_header(data),
            render_agents(data),
            render_kanban(data),
            render_recent_activity(data),
            render_prs(data),
        )
        right = Group(
            render_activity_log(data),
            render_agent_terminal(data),
        )
        layout.split_row(
            Layout(left, name="left", ratio=3),
            Layout(right, name="right", ratio=2),
        )
        return layout

    # Small: single column, everything stacked (scrollable)
    return Group(
        render_header(data),
        render_agents(data),
        render_kanban(data),
        render_inbox(data),
        render_recent_activity(data),
        render_prs(data),
        render_metrics(data),
        render_activity_log(data),
        render_agent_terminal(data),
    )


# ─── Output modes ────────────────────────────────────────────────────


def print_snapshot(console: Console) -> None:
    """Print a one-shot snapshot and exit. Layout adapts to terminal size."""
    data = collect_all(selected_tab=1)
    w, h = console.size
    console.print(render_dashboard(data, width=w, height=h))


def _poll_key(fd: int) -> str | None:
    """Non-blocking single-char read from fd using select (no tty.setraw)."""
    import select

    r, _, _ = select.select([fd], [], [], 0)
    if r:
        try:
            return os.read(fd, 1).decode("utf-8", errors="ignore")
        except OSError:
            return None
    return None


def print_live(console: Console) -> None:
    """Run a live-updating dashboard with keyboard tab switching.

    Uses termios non-canonical mode (ICANON/ECHO off, VMIN=0/VTIME=0) instead
    of tty.setraw() so Rich Live's alternate screen rendering works over SSH.
    Keys 1-9 switch agent tabs, q or Ctrl+C exits.
    """
    selected_tab = 1
    old_settings = None
    has_termios = False
    fd = sys.stdin.fileno()

    try:
        import termios

        old_settings = termios.tcgetattr(fd)
        new = termios.tcgetattr(fd)
        # Disable canonical mode and echo — leave everything else intact
        new[3] &= ~(termios.ICANON | termios.ECHO)
        new[6][termios.VMIN] = 0
        new[6][termios.VTIME] = 0
        termios.tcsetattr(fd, termios.TCSADRAIN, new)
        has_termios = True
    except (ImportError, termios.error):
        pass  # Fall back to no keyboard input (still shows dashboard)

    try:
        with Live(console=console, refresh_per_second=1, screen=True) as live:
            while True:
                w, h = console.size
                data = collect_all(selected_tab=selected_tab)
                live.update(render_dashboard(data, width=w, height=h))
                # Poll keyboard 50×100ms = 5s between data refreshes
                for _ in range(50):
                    if has_termios:
                        ch = _poll_key(fd)
                        if ch == "q" or ch == "\x03":
                            raise KeyboardInterrupt
                        if ch and ch.isdigit() and ch != "0":
                            selected_tab = int(ch)
                    time.sleep(0.1)
    except KeyboardInterrupt:
        pass
    finally:
        if old_settings is not None:
            import termios

            termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)


def print_json() -> None:
    """Print raw JSON and exit."""
    data = collect_all()
    json.dump(data, sys.stdout, indent=2, default=str)
    sys.stdout.write("\n")


# ─── CLI ──────────────────────────────────────────────────────────────


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Odin Agent Swarm TUI Dashboard",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "-l", "--live",
        action="store_true",
        help="Live mode: refresh every 5s (Ctrl+C to exit)",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Output raw JSON (for piping/scripting)",
    )
    args = parser.parse_args()

    if args.json:
        print_json()
    elif args.live:
        console = Console()
        print_live(console)
    else:
        console = Console()
        print_snapshot(console)


if __name__ == "__main__":
    main()
