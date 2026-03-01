import * as readline from "node:readline";
import { loadConfig, isDomainAllowed } from "./config.js";
import { getStagehand, shutdownBrowser } from "./browser.js";
import { handleNavigate } from "./capabilities/navigate.js";
import { handleAct } from "./capabilities/act.js";
import { handleExtract } from "./capabilities/extract.js";
import { handleObserve } from "./capabilities/observe.js";
import { handleAgent } from "./capabilities/agent.js";
import type { EventEnvelope, PluginDirective } from "./protocol.js";

const config = loadConfig();

const SUPPORTED_CAPABILITIES = new Set([
  "browser.navigate",
  "browser.act",
  "browser.extract",
  "browser.observe",
  "browser.agent",
]);

function emit(directive: PluginDirective): void {
  process.stdout.write(JSON.stringify(directive) + "\n");
}

function emitNoop(): void {
  emit({ action: "noop" });
}

/**
 * Route a task.received event to the appropriate capability request.
 */
function handleTaskReceived(event: EventEnvelope): void {
  const payload = event.payload;
  const taskType = (payload.task_type as string) || "";
  const project = event.project || "default";

  if (!SUPPORTED_CAPABILITIES.has(taskType)) {
    emitNoop();
    return;
  }

  const input = (payload.input as Record<string, unknown>) || {};
  const url = (input.url as string) || "";
  if (url && !isDomainAllowed(url, config)) {
    emit({
      action: "request_capability",
      capability: { id: taskType, project },
      reason: `Domain not in allowlist: ${url}`,
      input,
      risk_tier: "destructive",
    });
    return;
  }

  emit({
    action: "request_capability",
    capability: { id: taskType, project },
    reason: `Execute ${taskType}`,
    input,
    risk_tier: "safe",
  });
}

/**
 * Handle an action that has been approved by the policy engine.
 */
async function executeCapability(
  capabilityId: string,
  input: Record<string, unknown>,
): Promise<{ status: "executed" | "failed"; detail: string; output: unknown }> {
  const stagehand = await getStagehand(config);

  const url = input.url as string | undefined;
  if (url && capabilityId !== "browser.navigate") {
    const currentUrl = stagehand.page.url();
    if (currentUrl !== url && !currentUrl.startsWith(url)) {
      await stagehand.page.goto(url, { waitUntil: "domcontentloaded" });
    }
  }

  // Verify current page is on an allowed domain
  const currentPageUrl = stagehand.page.url();
  if (currentPageUrl && currentPageUrl !== "about:blank" && !isDomainAllowed(currentPageUrl, config)) {
    return { status: "failed", detail: `Current page domain not in allowlist: ${currentPageUrl}`, output: null };
  }

  switch (capabilityId) {
    case "browser.navigate": {
      const result = await handleNavigate(stagehand, {
        url: url || stagehand.page.url(),
      });
      return { status: "executed", detail: `Navigated to ${result.url}`, output: result };
    }
    case "browser.act": {
      const result = await handleAct(stagehand, {
        instruction: input.instruction as string,
        variables: input.variables as Record<string, string> | undefined,
      });
      return { status: result.success ? "executed" : "failed", detail: result.message, output: result };
    }
    case "browser.extract": {
      const result = await handleExtract(stagehand, {
        instruction: input.instruction as string,
        schema: input.schema as Record<string, unknown> | undefined,
        selector: input.selector as string | undefined,
      });
      return { status: "executed", detail: "Extraction complete", output: result };
    }
    case "browser.observe": {
      const result = await handleObserve(stagehand, {
        instruction: input.instruction as string | undefined,
      });
      return { status: "executed", detail: "Observation complete", output: result };
    }
    case "browser.agent": {
      const result = await handleAgent(stagehand, {
        instruction: input.instruction as string,
        maxSteps: input.maxSteps as number | undefined,
      });
      return {
        status: result.success ? "executed" : "failed",
        detail: result.message,
        output: result,
      };
    }
    default:
      return { status: "failed", detail: `Unknown capability: ${capabilityId}`, output: null };
  }
}

async function handleEvent(event: EventEnvelope): Promise<void> {
  try {
    switch (event.event_type) {
      case "task.received":
        handleTaskReceived(event);
        break;

      case "action.approved": {
        const payload = event.payload;
        const capId = (payload.capability_id as string) || "";
        const input = (payload.input as Record<string, unknown>) || {};
        const result = await executeCapability(capId, input);

        emit({
          action: "enqueue_task",
          task_type: "stagehand.result",
          project: event.project,
          reason: result.detail,
          payload: { status: result.status, detail: result.detail, output: result.output },
        });
        break;
      }

      default:
        emitNoop();
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    process.stderr.write(`[stagehand] Error handling event: ${msg}\n`);
    if (event.event_type === "action.approved") {
      emit({
        action: "enqueue_task",
        task_type: "stagehand.result",
        project: event.project,
        reason: `Error: ${msg}`,
        payload: { status: "failed", detail: msg, output: null },
      });
    } else {
      emitNoop();
    }
  }
}

async function serve(): Promise<void> {
  const rl = readline.createInterface({ input: process.stdin, terminal: false });

  for await (const line of rl) {
    if (!line.trim()) continue;

    let event: EventEnvelope;
    try {
      event = JSON.parse(line) as EventEnvelope;
    } catch {
      process.stderr.write(`[stagehand] Invalid JSON: ${line.slice(0, 100)}\n`);
      emitNoop();
      continue;
    }

    await handleEvent(event);
  }

  await shutdownBrowser();
}

// --- CLI ---
const cmd = process.argv[2] || "serve";

switch (cmd) {
  case "serve":
    serve().catch((err) => {
      process.stderr.write(`[stagehand] Fatal: ${err}\n`);
      process.exit(1);
    });
    break;

  case "emit-sample":
    handleEvent({
      event_id: "test-1",
      event_type: "task.received",
      project: "cfipros",
      payload: {
        task_type: "browser.navigate",
        input: { url: "https://cfipros.com" },
      },
    });
    break;

  default:
    process.stderr.write(`Unknown command: ${cmd}\n`);
    process.exit(64);
}
