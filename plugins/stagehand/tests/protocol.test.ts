import { describe, it, expect } from "vitest";
import type { EventEnvelope, PluginDirective } from "../src/protocol.js";

describe("Protocol types", () => {
  it("should serialize a request_capability directive", () => {
    const directive: PluginDirective = {
      action: "request_capability",
      capability: { id: "browser.navigate", project: "cfipros" },
      reason: "Navigate to homepage",
      input: { url: "https://cfipros.com" },
      risk_tier: "safe",
    };
    const json = JSON.stringify(directive);
    const parsed = JSON.parse(json) as PluginDirective;
    expect(parsed.action).toBe("request_capability");
  });

  it("should serialize a noop directive", () => {
    const directive: PluginDirective = { action: "noop" };
    const json = JSON.stringify(directive);
    expect(json).toBe('{"action":"noop"}');
  });

  it("should parse an EventEnvelope", () => {
    const envelope: EventEnvelope = {
      event_id: "evt-1",
      event_type: "task.received",
      project: "cfipros",
      payload: { task_type: "browser.navigate", input: { url: "https://cfipros.com" } },
    };
    const json = JSON.stringify(envelope);
    const parsed = JSON.parse(json) as EventEnvelope;
    expect(parsed.event_type).toBe("task.received");
    expect(parsed.payload.task_type).toBe("browser.navigate");
  });
});
