"""Engine API client for the TUI."""
from __future__ import annotations

import os

import httpx

API_BASE = os.environ.get("ODIN_API_URL", "http://localhost:9300")
API_KEY = os.environ.get("ODIN_ENGINE_API_KEY", "")

_client: httpx.Client | None = None


def _get_client() -> httpx.Client:
    """Return a lazily initialized httpx.Client singleton."""
    global _client
    if _client is None:
        _client = httpx.Client(
            base_url=API_BASE,
            headers={"X-API-Key": API_KEY, "Content-Type": "application/json"},
            timeout=5.0,
        )
    return _client


def get(path: str) -> dict | list | None:
    try:
        r = _get_client().get(path)
        r.raise_for_status()
        return r.json()
    except (httpx.HTTPError, httpx.TimeoutException, ConnectionError):
        return None


def post(path: str, body: dict | None = None) -> dict | list | None:
    try:
        r = _get_client().post(path, json=body or {})
        r.raise_for_status()
        return r.json()
    except (httpx.HTTPError, httpx.TimeoutException, ConnectionError):
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


def cancel_task(task_id: str) -> dict | None:
    return post(f"/api/v1/tasks/{task_id}/cancel")


def restart_agent(name: str) -> dict | None:
    return post(f"/api/v1/agents/{name}/restart")


def send_command(command: str) -> dict | None:
    return post("/api/v1/commands", {"command": command})
