import type { EventEnvelope, PluginDirective, RiskTier } from "./protocol.js";

const SUPPORTED_TASKS = new Set([
  "gmail_triage",
  "gmail.inbox.list",
  "gmail.message.read",
  "gmail.label.apply",
  "gmail.thread.archive",
  "gmail.thread.mark_read",
  "gmail.draft.create",
  "gmail.draft.send",
  "gmail.unsubscribe",
  "gmail.message.trash",
  "gmail.message.delete",
]);

const SENSITIVE_CAPABILITIES = new Set([
  "gmail.draft.send",
  "gmail.unsubscribe",
  "gmail.message.trash",
]);

const DESTRUCTIVE_CAPABILITIES = new Set([
  "gmail.message.delete",
]);

function riskTierFor(capabilityId: string): RiskTier {
  if (DESTRUCTIVE_CAPABILITIES.has(capabilityId)) return "destructive";
  if (SENSITIVE_CAPABILITIES.has(capabilityId)) return "sensitive";
  return "safe";
}

export function handleTaskReceived(event: EventEnvelope): PluginDirective[] {
  const taskType = (event.payload.task_type as string) || "";
  const project = event.project || "personal";

  if (!SUPPORTED_TASKS.has(taskType)) {
    return [{ action: "noop" }];
  }

  // gmail_triage maps to inbox.list as the first step
  const capabilityId = taskType === "gmail_triage" ? "gmail.inbox.list" : taskType;
  const input = (event.payload.input as Record<string, unknown>) || {};
  if (taskType === "gmail_triage") {
    (input as Record<string, unknown>).history_id = event.payload.history_id;
  }

  return [{
    action: "request_capability",
    capability: { id: capabilityId, project },
    reason: `Execute ${taskType}`,
    input,
    risk_tier: riskTierFor(capabilityId),
  }];
}

export function handleActionApproved(event: EventEnvelope): PluginDirective[] {
  const capId = (event.payload.capability_id as string) || "";
  return [{
    action: "enqueue_task",
    task_type: "gmail.result",
    project: event.project,
    reason: `Executed ${capId}`,
    payload: { status: "executed", capability: capId },
  }];
}
