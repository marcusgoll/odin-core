import * as readline from "node:readline";
import type { EventEnvelope, PluginDirective } from "./protocol.js";

function emit(directive: PluginDirective): void {
  process.stdout.write(JSON.stringify(directive) + "\n");
}

function emitNoop(): void {
  emit({ action: "noop" });
}

async function handleEvent(event: EventEnvelope): Promise<void> {
  switch (event.event_type) {
    case "task.received":
      // TODO: route to capability request
      emitNoop();
      break;

    case "action.approved":
      // TODO: execute approved capability
      emitNoop();
      break;

    default:
      emitNoop();
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
    serve().catch((err) => {
      process.stderr.write(`[gmail] Fatal: ${err}\n`);
      process.exit(1);
    });
    break;

  default:
    process.stderr.write(`Unknown command: ${cmd}\n`);
    process.exit(64);
}
