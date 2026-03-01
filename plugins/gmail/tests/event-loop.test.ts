import { describe, it, expect } from "vitest";
import { handleTaskReceived, handleActionApproved } from "../src/handlers.js";
import type { EventEnvelope, PluginDirective } from "../src/protocol.js";

describe("handleTaskReceived", () => {
  it("should request gmail.inbox.list capability for gmail_triage task", () => {
    const event: EventEnvelope = {
      event_id: "evt-1",
      event_type: "task.received",
      project: "personal",
      payload: { task_type: "gmail_triage", history_id: "12345" },
    };
    const directives = handleTaskReceived(event);
    expect(directives.length).toBe(1);
    expect(directives[0].action).toBe("request_capability");
    if (directives[0].action === "request_capability") {
      expect(directives[0].capability.id).toBe("gmail.inbox.list");
      expect(directives[0].risk_tier).toBe("safe");
    }
  });

  it("should emit noop for unknown task types", () => {
    const event: EventEnvelope = {
      event_id: "evt-2",
      event_type: "task.received",
      payload: { task_type: "unknown_task" },
    };
    const directives = handleTaskReceived(event);
    expect(directives.length).toBe(1);
    expect(directives[0].action).toBe("noop");
  });

  it("should request sensitive capability for gmail.draft.send", () => {
    const event: EventEnvelope = {
      event_id: "evt-3",
      event_type: "task.received",
      project: "personal",
      payload: { task_type: "gmail.draft.send", input: { draft_id: "d-1" } },
    };
    const directives = handleTaskReceived(event);
    expect(directives.length).toBe(1);
    if (directives[0].action === "request_capability") {
      expect(directives[0].risk_tier).toBe("sensitive");
    }
  });
});
