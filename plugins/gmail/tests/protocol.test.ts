import { describe, it, expect } from "vitest";
import type { EventEnvelope, PluginDirective } from "../src/protocol.js";

describe("Protocol types", () => {
  it("should round-trip a request_capability directive", () => {
    const directive: PluginDirective = {
      action: "request_capability",
      capability: { id: "gmail.inbox.list", project: "personal" },
      reason: "List inbox messages",
      input: { max_results: 10 },
      risk_tier: "safe",
    };
    const json = JSON.stringify(directive);
    const parsed = JSON.parse(json) as PluginDirective;
    expect(parsed.action).toBe("request_capability");
    if (parsed.action === "request_capability") {
      expect(parsed.capability.id).toBe("gmail.inbox.list");
      expect(parsed.risk_tier).toBe("safe");
    }
  });

  it("should round-trip an enqueue_task directive", () => {
    const directive: PluginDirective = {
      action: "enqueue_task",
      task_type: "gmail.result",
      project: "personal",
      reason: "Labeled 3 messages",
      payload: { status: "executed", count: 3 },
    };
    const json = JSON.stringify(directive);
    const parsed = JSON.parse(json) as PluginDirective;
    expect(parsed.action).toBe("enqueue_task");
    if (parsed.action === "enqueue_task") {
      expect(parsed.task_type).toBe("gmail.result");
    }
  });

  it("should parse an EventEnvelope", () => {
    const envelope: EventEnvelope = {
      event_id: "evt-1",
      event_type: "task.received",
      project: "personal",
      payload: { task_type: "gmail.inbox.list", input: { max_results: 10 } },
    };
    const json = JSON.stringify(envelope);
    const parsed = JSON.parse(json) as EventEnvelope;
    expect(parsed.event_type).toBe("task.received");
    expect(parsed.payload.task_type).toBe("gmail.inbox.list");
  });
});
