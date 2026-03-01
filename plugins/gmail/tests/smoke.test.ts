import { describe, it, expect } from "vitest";
import { execSync } from "node:child_process";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const pluginDir = path.join(__dirname, "..");

describe("Plugin smoke test", () => {
  it("should emit request_capability for gmail_triage task", () => {
    const event = JSON.stringify({
      event_id: "smoke-1",
      event_type: "task.received",
      project: "personal",
      payload: { task_type: "gmail_triage", history_id: "99999" },
    });

    const result = execSync(`echo '${event}' | node dist/index.js serve`, {
      cwd: pluginDir,
      encoding: "utf-8",
      timeout: 10_000,
    }).trim();

    const directive = JSON.parse(result);
    expect(directive.action).toBe("request_capability");
    expect(directive.capability.id).toBe("gmail.inbox.list");
    expect(directive.risk_tier).toBe("safe");
  });

  it("should emit noop for unknown task type", () => {
    const event = JSON.stringify({
      event_id: "smoke-2",
      event_type: "task.received",
      payload: { task_type: "something_else" },
    });

    const result = execSync(`echo '${event}' | node dist/index.js serve`, {
      cwd: pluginDir,
      encoding: "utf-8",
      timeout: 10_000,
    }).trim();

    const directive = JSON.parse(result);
    expect(directive.action).toBe("noop");
  });

  it("should emit noop for unknown event type", () => {
    const event = JSON.stringify({
      event_id: "smoke-3",
      event_type: "unknown.event",
      payload: {},
    });

    const result = execSync(`echo '${event}' | node dist/index.js serve`, {
      cwd: pluginDir,
      encoding: "utf-8",
      timeout: 10_000,
    }).trim();

    const directive = JSON.parse(result);
    expect(directive.action).toBe("noop");
  });

  it("should request sensitive capability for gmail.draft.send", () => {
    const event = JSON.stringify({
      event_id: "smoke-4",
      event_type: "task.received",
      project: "personal",
      payload: { task_type: "gmail.draft.send", input: { draft_id: "d-1" } },
    });

    const result = execSync(`echo '${event}' | node dist/index.js serve`, {
      cwd: pluginDir,
      encoding: "utf-8",
      timeout: 10_000,
    }).trim();

    const directive = JSON.parse(result);
    expect(directive.action).toBe("request_capability");
    expect(directive.risk_tier).toBe("sensitive");
  });
});
