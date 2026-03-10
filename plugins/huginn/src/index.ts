import * as readline from "node:readline";
import { pathToFileURL } from "node:url";

import {
  isDomainAllowed,
  loadConfig,
  normalizeObserveTarget,
} from "./config.js";
import { HuginnClient } from "./huginn-client.js";
import type { EventEnvelope, PluginDirective } from "./protocol.js";

const config = loadConfig();

const SUPPORTED_CAPABILITIES = new Set([
  "browser.observe",
  "huginn.observe_url",
  "huginn.observe_domain",
]);

type ExecutionResult = {
  status: "executed" | "failed";
  detail: string;
  output: unknown;
};

function emit(directive: PluginDirective): void {
  process.stdout.write(`${JSON.stringify(directive)}\n`);
}

function emitNoop(): void {
  emit({ action: "noop" });
}

export function isSupportedCapability(capabilityId: string): boolean {
  return SUPPORTED_CAPABILITIES.has(capabilityId);
}

export function capabilityForTaskType(taskType: string): string | undefined {
  return isSupportedCapability(taskType) ? taskType : undefined;
}

function handleTaskReceived(event: EventEnvelope): void {
  const payload = event.payload;
  const taskType = (payload.task_type as string) || "";
  const capabilityId = capabilityForTaskType(taskType);
  if (!capabilityId) {
    emitNoop();
    return;
  }

  emit({
    action: "request_capability",
    capability: {
      id: capabilityId,
      project: event.project || "default",
    },
    reason: `Execute ${capabilityId} via Huginn`,
    input: (payload.input as Record<string, unknown>) || {},
    risk_tier: "safe",
  });
}

async function executeCapability(
  capabilityId: string,
  input: Record<string, unknown>,
): Promise<ExecutionResult> {
  if (!isSupportedCapability(capabilityId)) {
    return {
      status: "failed",
      detail: `Unknown capability: ${capabilityId}`,
      output: null,
    };
  }

  const targetUrl = normalizeObserveTarget(input);
  if (targetUrl && !isDomainAllowed(targetUrl, config)) {
    return {
      status: "failed",
      detail: `Target domain not allowlisted: ${targetUrl}`,
      output: null,
    };
  }

  const client = new HuginnClient(config);
  const result = await client.observe(targetUrl);
  return {
    status: "executed",
    detail: targetUrl
      ? `Observed ${result.url || targetUrl}`
      : "Observed active Huginn page",
    output: result,
  };
}

export async function handleEvent(event: EventEnvelope): Promise<void> {
  try {
    switch (event.event_type) {
      case "task.received":
        handleTaskReceived(event);
        break;
      case "action.approved": {
        const payload = event.payload;
        const capabilityId = (payload.capability_id as string) || "";
        const input = (payload.input as Record<string, unknown>) || {};
        const result = await executeCapability(capabilityId, input);
        emit({
          action: "enqueue_task",
          task_type: "huginn.result",
          project: event.project,
          reason: result.detail,
          payload: {
            status: result.status,
            detail: result.detail,
            output: result.output,
          },
        });
        break;
      }
      default:
        emitNoop();
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    process.stderr.write(`[huginn] Error handling event: ${message}\n`);
    if (event.event_type === "action.approved") {
      emit({
        action: "enqueue_task",
        task_type: "huginn.result",
        project: event.project,
        reason: `Error: ${message}`,
        payload: {
          status: "failed",
          detail: message,
          output: null,
        },
      });
      return;
    }
    emitNoop();
  }
}

async function serve(): Promise<void> {
  const rl = readline.createInterface({
    input: process.stdin,
    terminal: false,
  });

  for await (const line of rl) {
    if (!line.trim()) {
      continue;
    }

    let event: EventEnvelope;
    try {
      event = JSON.parse(line) as EventEnvelope;
    } catch {
      process.stderr.write(`[huginn] Invalid JSON: ${line.slice(0, 100)}\n`);
      emitNoop();
      continue;
    }

    await handleEvent(event);
  }
}

async function main(): Promise<void> {
  const cmd = process.argv[2] || "serve";
  switch (cmd) {
    case "serve":
      await serve();
      break;
    case "emit-sample":
      await handleEvent({
        event_id: "test-1",
        event_type: "task.received",
        project: "cfipros",
        payload: {
          task_type: "huginn.observe_url",
          input: { url: "https://cfipros.com" },
        },
      });
      break;
    default:
      process.stderr.write(`Unknown command: ${cmd}\n`);
      process.exit(64);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((err) => {
    process.stderr.write(`[huginn] Fatal: ${err}\n`);
    process.exit(1);
  });
}
