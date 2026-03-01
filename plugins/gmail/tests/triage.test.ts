import { describe, it, expect } from "vitest";
import { loadRules, evaluateRules, type TriageRule, type MessageMeta } from "../src/triage.js";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

describe("loadRules", () => {
  it("should parse the default rules file", () => {
    const rulesPath = path.join(__dirname, "../config/gmail-rules.yaml");
    const rules = loadRules(rulesPath);
    expect(rules.length).toBeGreaterThan(0);
    expect(rules[0].name).toBe("receipts");
  });
});

describe("evaluateRules", () => {
  const rules: TriageRule[] = [
    {
      name: "receipts",
      match: { from_pattern: "(receipt|invoice)@" },
      actions: [{ label: "Receipts" }, { archive: true }],
    },
    {
      name: "newsletters",
      match: { headers: { "List-Unsubscribe": "present" } },
      actions: [{ label: "Newsletters" }, { archive: true }],
    },
    {
      name: "spam_candidates",
      match: { subject_pattern: "(urgent|act now|winner)" },
      actions: [{ label: "Spam/Review" }, { request_trash: true }],
    },
    {
      name: "uncategorized",
      match: { no_rule_matched: true },
      actions: [{ label: "Triage/Review" }],
    },
  ];

  it("should match receipts by from address", () => {
    const msg: MessageMeta = {
      id: "msg-1",
      threadId: "t-1",
      from: "receipt@example.com",
      to: "me@gmail.com",
      subject: "Your order",
      headers: {},
      snippet: "",
    };
    const result = evaluateRules(rules, msg);
    expect(result?.name).toBe("receipts");
    expect(result?.actions).toContainEqual({ label: "Receipts" });
  });

  it("should match newsletters by List-Unsubscribe header", () => {
    const msg: MessageMeta = {
      id: "msg-2",
      threadId: "t-2",
      from: "news@example.com",
      to: "me@gmail.com",
      subject: "Weekly digest",
      headers: { "List-Unsubscribe": "<mailto:unsub@example.com>" },
      snippet: "",
    };
    const result = evaluateRules(rules, msg);
    expect(result?.name).toBe("newsletters");
  });

  it("should match spam by subject pattern", () => {
    const msg: MessageMeta = {
      id: "msg-3",
      threadId: "t-3",
      from: "unknown@spam.com",
      to: "me@gmail.com",
      subject: "URGENT: You are a winner!",
      headers: {},
      snippet: "",
    };
    const result = evaluateRules(rules, msg);
    expect(result?.name).toBe("spam_candidates");
  });

  it("should fall through to uncategorized", () => {
    const msg: MessageMeta = {
      id: "msg-4",
      threadId: "t-4",
      from: "friend@example.com",
      to: "me@gmail.com",
      subject: "Lunch tomorrow?",
      headers: {},
      snippet: "",
    };
    const result = evaluateRules(rules, msg);
    expect(result?.name).toBe("uncategorized");
  });

  it("should return first match (receipts before spam)", () => {
    const msg: MessageMeta = {
      id: "msg-5",
      threadId: "t-5",
      from: "receipt@urgent-store.com",
      to: "me@gmail.com",
      subject: "URGENT receipt",
      headers: {},
      snippet: "",
    };
    const result = evaluateRules(rules, msg);
    expect(result?.name).toBe("receipts");
  });
});
