import * as readline from "node:readline";
import * as fs from "node:fs";
import type { EventEnvelope, PluginDirective } from "./protocol.js";
import { handleTaskReceived } from "./handlers.js";
import { loadConfig } from "./config.js";
import { getGmailClient, listMessages, getMessage, applyLabel, archiveMessage, trashMessage, sendDraft, ensureLabel, getHistory } from "./gmail-client.js";
import { loadRules, evaluateRules, type MessageMeta } from "./triage.js";

const config = loadConfig();

function emit(directive: PluginDirective): void {
  process.stdout.write(JSON.stringify(directive) + "\n");
}

function emitNoop(): void {
  emit({ action: "noop" });
}

function extractHeaders(msg: { payload?: { headers?: Array<{ name?: string | null; value?: string | null }> | null } | null }): Record<string, string> {
  const headers: Record<string, string> = {};
  for (const h of msg.payload?.headers || []) {
    if (h.name && h.value) headers[h.name] = h.value;
  }
  return headers;
}

function toMessageMeta(msg: { id?: string | null; threadId?: string | null; snippet?: string | null; payload?: { headers?: Array<{ name?: string | null; value?: string | null }> | null } | null }): MessageMeta {
  const headers = extractHeaders(msg);
  return {
    id: msg.id || "",
    threadId: msg.threadId || "",
    from: headers["From"] || "",
    to: headers["To"] || "",
    subject: headers["Subject"] || "",
    headers,
    snippet: msg.snippet || "",
  };
}

async function executeTriageActions(event: EventEnvelope): Promise<void> {
  try {
    const client = getGmailClient();
    const historyId = (event.payload.input as Record<string, unknown>)?.history_id as string | undefined;
    let messageIds: string[] = [];

    if (historyId) {
      const history = await getHistory(client, historyId);
      for (const h of history) {
        for (const added of h.messagesAdded || []) {
          if (added.message?.id) messageIds.push(added.message.id);
        }
      }
    } else {
      const msgs = await listMessages(client, "is:unread in:inbox", 20);
      messageIds = msgs.map((m) => m.id!).filter(Boolean);
    }

    if (messageIds.length === 0) {
      emit({ action: "enqueue_task", task_type: "gmail.result", project: event.project, reason: "No new messages", payload: { status: "executed", count: 0 } });
      return;
    }

    const rules = loadRules(config.rulesPath);
    let actionsApplied = 0;

    for (const msgId of messageIds) {
      const rawMsg = await getMessage(client, msgId);
      const meta = toMessageMeta(rawMsg);
      const result = evaluateRules(rules, meta);

      if (!result) continue;

      for (const action of result.actions) {
        if (action.label) {
          const labelId = await ensureLabel(client, action.label);
          await applyLabel(client, msgId, labelId);
          actionsApplied++;
        }
        if (action.archive) {
          await archiveMessage(client, msgId);
          actionsApplied++;
        }
        if (action.request_trash) {
          emit({
            action: "request_capability",
            capability: { id: "gmail.message.trash", project: event.project },
            reason: `Trash spam candidate: ${meta.subject}`,
            input: { message_id: msgId, subject: meta.subject, from: meta.from },
            risk_tier: "sensitive",
          });
        }
        if (action.draft_reply) {
          emit({
            action: "request_capability",
            capability: { id: "gmail.draft.create", project: event.project },
            reason: `Draft reply to: ${meta.subject}`,
            input: { message_id: msgId, thread_id: meta.threadId, to: meta.from, subject: `Re: ${meta.subject}` },
            risk_tier: "safe",
          });
        }
      }
    }

    emit({
      action: "enqueue_task",
      task_type: "gmail.result",
      project: event.project,
      reason: `Triaged ${messageIds.length} messages, ${actionsApplied} actions applied`,
      payload: { status: "executed", messages: messageIds.length, actions: actionsApplied },
    });

    // Save last history ID for dedup
    if (historyId) {
      const stateFile = `${config.stateDir}/last_history_id`;
      fs.mkdirSync(config.stateDir, { recursive: true });
      fs.writeFileSync(stateFile, historyId);
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    process.stderr.write(`[gmail] Triage error: ${msg}\n`);
    emit({
      action: "enqueue_task",
      task_type: "gmail.result",
      project: event.project,
      reason: `Error: ${msg}`,
      payload: { status: "failed", detail: msg },
    });
  }
}

async function handleEvent(event: EventEnvelope): Promise<void> {
  try {
    switch (event.event_type) {
      case "task.received": {
        const directives = handleTaskReceived(event);
        for (const d of directives) emit(d);
        break;
      }

      case "action.approved": {
        const capId = (event.payload.capability_id as string) || "";
        const input = (event.payload.input as Record<string, unknown>) || {};

        if (capId === "gmail.inbox.list") {
          await executeTriageActions(event);
        } else if (capId === "gmail.message.trash") {
          const client = getGmailClient();
          await trashMessage(client, input.message_id as string);
          emit({ action: "enqueue_task", task_type: "gmail.result", project: event.project, reason: `Trashed: ${input.subject}`, payload: { status: "executed", capability: capId } });
        } else if (capId === "gmail.draft.send") {
          const client = getGmailClient();
          await sendDraft(client, input.draft_id as string);
          emit({ action: "enqueue_task", task_type: "gmail.result", project: event.project, reason: `Sent draft: ${input.draft_id}`, payload: { status: "executed", capability: capId } });
        } else {
          emitNoop();
        }
        break;
      }

      default:
        emitNoop();
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    process.stderr.write(`[gmail] Error handling event: ${msg}\n`);
    if (event.event_type === "action.approved") {
      emit({ action: "enqueue_task", task_type: "gmail.result", project: event.project, reason: `Error: ${msg}`, payload: { status: "failed", detail: msg } });
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
      process.stderr.write(`[gmail] Invalid JSON: ${line.slice(0, 100)}\n`);
      emitNoop();
      continue;
    }
    await handleEvent(event);
  }
}

const cmd = process.argv[2] || "serve";
switch (cmd) {
  case "serve":
    serve().catch((err) => { process.stderr.write(`[gmail] Fatal: ${err}\n`); process.exit(1); });
    break;
  default:
    process.stderr.write(`Unknown command: ${cmd}\n`);
    process.exit(64);
}
