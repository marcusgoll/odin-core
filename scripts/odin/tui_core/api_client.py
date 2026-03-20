"""Engine API client for the TUI."""
from __future__ import annotations

import os

import httpx

API_BASE = os.environ.get("ODIN_API_URL", "http://localhost:9300")
API_KEY = os.environ.get("ODIN_ENGINE_API_KEY", "")


def _headers() -> dict[str, str]:
    return {"X-API-Key": API_KEY, "Content-Type": "application/json"}


def get(path: str) -> dict | list | None:
    try:
        r = httpx.get(f"{API_BASE}{path}", headers=_headers(), timeout=5.0)
        r.raise_for_status()
        return r.json()
    except Exception:
        return None


def post(path: str, body: dict | None = None) -> dict | list | None:
    try:
        r = httpx.post(
            f"{API_BASE}{path}", headers=_headers(), json=body or {}, timeout=5.0
        )
        r.raise_for_status()
        return r.json()
    except Exception:
        return None


def fetch_approvals() -> list[dict]:
    return get("/api/v1/approvals") or []


def approve_task(task_id: str) -> dict | None:
    return post(f"/api/v1/tasks/{task_id}/approve")


def reject_task(task_id: str) -> dict | None:
    return post(f"/api/v1/tasks/{task_id}/reject")


def kill_agent(name: str) -> dict | None:
    return post(f"/api/v1/agents/{name}/kill")


def requeue_task(task_id: str) -> dict | None:
    return post(f"/api/v1/tasks/{task_id}/requeue")


def send_command(command: str) -> dict | None:
    return post("/api/v1/commands", {"command": command})
