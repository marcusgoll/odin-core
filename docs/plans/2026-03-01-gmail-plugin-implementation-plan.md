# Gmail Plugin Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Gmail triage plugin for Odin that auto-labels, archives, deletes spam, unsubscribes, and drafts replies — following the stagehand plugin pattern exactly.

**Architecture:** TypeScript external-process plugin in `odin-core/plugins/gmail/`. Receives EventEnvelope JSON on stdin, emits PluginDirective JSON on stdout. Gmail API via `googleapis` npm package. OAuth2 tokens stored via Odin secret bindings. Triage rules in `/var/odin/config/gmail-rules.yaml`.

**Tech Stack:** TypeScript, Node.js, googleapis, vitest, Odin plugin protocol

**Design Doc:** `docs/plans/2026-03-01-gmail-plugin-design.md`

---

### Task 1: Scaffold Plugin Project

**Files:**
- Create: `plugins/gmail/package.json`
- Create: `plugins/gmail/tsconfig.json`
- Create: `plugins/gmail/vitest.config.ts`
- Create: `plugins/gmail/.gitignore`
- Create: `plugins/gmail/odin.plugin.yaml`

**Step 1: Create package.json**

```json
{
  "name": "odin-plugin-gmail",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "main": "dist/index.js",
  "scripts": {
    "build": "tsc",
    "dev": "tsc --watch",
    "test": "vitest run",
    "test:watch": "vitest",
    "serve": "node dist/index.js serve"
  },
  "dependencies": {
    "googleapis": "^144.0.0",
    "yaml": "^2.7.0"
  },
  "devDependencies": {
    "@types/node": "^22",
    "typescript": "^5.7",
    "vitest": "^3"
  }
}
```

**Step 2: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "outDir": "dist",
    "rootDir": "src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "declaration": true,
    "sourceMap": true
  },
  "include": ["src"],
  "exclude": ["node_modules", "dist", "tests"]
}
```

**Step 3: Create vitest.config.ts**

```typescript
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    globals: true,
    testTimeout: 30_000,
  },
});
```

**Step 4: Create .gitignore**

```
node_modules/
dist/
.env
```

**Step 5: Create odin.plugin.yaml**

```yaml
schema_version: 1
plugin:
  name: odin.gmail
  version: 0.1.0
  description: Gmail inbox triage — label, archive, unsubscribe, draft replies
  runtime: external-process
  entrypoint:
    command: node
    args: ["dist/index.js", "serve"]
  compatibility:
    core_version: ">=0.1.0 <1.0.0"
  hooks:
    - event: task.received
      handler: on_task_received
    - event: action.approved
      handler: on_action_approved
  capabilities:
    - id: gmail.inbox.list
      scope: [project]
    - id: gmail.message.read
      scope: [project]
    - id: gmail.label.list
      scope: [project]
    - id: gmail.label.apply
      scope: [project]
    - id: gmail.thread.archive
      scope: [project]
    - id: gmail.thread.mark_read
      scope: [project]
    - id: gmail.draft.create
      scope: [project]
    - id: gmail.unsubscribe
      scope: [project]
    - id: gmail.draft.send
      scope: [project]
    - id: gmail.message.trash
      scope: [project]
    - id: gmail.message.delete
      scope: [project]
distribution:
  source:
    type: local-path
    ref: .
  integrity:
    checksum_sha256: "0000000000000000000000000000000000000000000000000000000000000000"
signing:
  required: false
  method: none
```

**Step 6: Install dependencies**

Run: `cd plugins/gmail && npm install`

**Step 7: Commit**

```bash
git add plugins/gmail/
git commit -m "feat(gmail): scaffold plugin project with manifest and build config"
```

---

### Task 2: Protocol Types & Event Loop Skeleton

**Files:**
- Create: `plugins/gmail/src/protocol.ts`
- Create: `plugins/gmail/src/index.ts`
- Create: `plugins/gmail/tests/protocol.test.ts`

**Step 1: Write the failing test**

Create `plugins/gmail/tests/protocol.test.ts`:

```typescript
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
```

**Step 2: Run test to verify it fails**

Run: `cd plugins/gmail && npx vitest run tests/protocol.test.ts`
Expected: FAIL — cannot resolve `../src/protocol.js`

**Step 3: Create protocol.ts**

Create `plugins/gmail/src/protocol.ts`:

```typescript
export type RiskTier = "safe" | "sensitive" | "destructive";

export interface EventEnvelope {
  event_id: string;
  event_type: string;
  task_id?: string;
  request_id?: string;
  project?: string;
  payload: Record<string, unknown>;
}

export interface RequestCapabilityDirective {
  action: "request_capability";
  capability: { id: string; project?: string };
  reason?: string;
  input?: unknown;
  risk_tier?: RiskTier;
}

export interface EnqueueTaskDirective {
  action: "enqueue_task";
  task_type: string;
  project?: string;
  reason?: string;
  payload?: unknown;
}

export interface NoopDirective {
  action: "noop";
}

export type PluginDirective =
  | RequestCapabilityDirective
  | EnqueueTaskDirective
  | NoopDirective;
```

**Step 4: Run test to verify it passes**

Run: `cd plugins/gmail && npx vitest run tests/protocol.test.ts`
Expected: PASS — 3 tests

**Step 5: Create the event loop skeleton (index.ts)**

Create `plugins/gmail/src/index.ts`:

```typescript
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
```

**Step 6: Build and verify**

Run: `cd plugins/gmail && npm run build`
Expected: Compiles without errors, `dist/index.js` and `dist/protocol.js` created

**Step 7: Verify event loop works**

Run: `echo '{"event_id":"test","event_type":"task.received","payload":{}}' | node dist/index.js serve`
Expected: `{"action":"noop"}` on stdout

**Step 8: Commit**

```bash
git add plugins/gmail/src/ plugins/gmail/tests/
git commit -m "feat(gmail): add protocol types and stdin/stdout event loop skeleton"
```

---

### Task 3: Config & Gmail Client

**Files:**
- Create: `plugins/gmail/src/config.ts`
- Create: `plugins/gmail/src/gmail-client.ts`
- Create: `plugins/gmail/tests/config.test.ts`

**Step 1: Write the failing test**

Create `plugins/gmail/tests/config.test.ts`:

```typescript
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { loadConfig } from "../src/config.js";

describe("loadConfig", () => {
  const origEnv = { ...process.env };

  afterEach(() => {
    process.env = { ...origEnv };
  });

  it("should return defaults when no env vars set", () => {
    delete process.env.GMAIL_RULES_PATH;
    delete process.env.GMAIL_STATE_DIR;
    const cfg = loadConfig();
    expect(cfg.rulesPath).toBe("/var/odin/config/gmail-rules.yaml");
    expect(cfg.stateDir).toBe("/var/odin/state/gmail");
    expect(cfg.account).toBe("personal");
  });

  it("should read env overrides", () => {
    process.env.GMAIL_RULES_PATH = "/tmp/rules.yaml";
    process.env.GMAIL_STATE_DIR = "/tmp/state";
    process.env.GMAIL_ACCOUNT = "work";
    const cfg = loadConfig();
    expect(cfg.rulesPath).toBe("/tmp/rules.yaml");
    expect(cfg.stateDir).toBe("/tmp/state");
    expect(cfg.account).toBe("work");
  });
});
```

**Step 2: Run test to verify it fails**

Run: `cd plugins/gmail && npx vitest run tests/config.test.ts`
Expected: FAIL — cannot resolve `../src/config.js`

**Step 3: Create config.ts**

Create `plugins/gmail/src/config.ts`:

```typescript
export interface PluginConfig {
  account: string;
  rulesPath: string;
  stateDir: string;
}

export function loadConfig(): PluginConfig {
  return {
    account: process.env.GMAIL_ACCOUNT || "personal",
    rulesPath: process.env.GMAIL_RULES_PATH || "/var/odin/config/gmail-rules.yaml",
    stateDir: process.env.GMAIL_STATE_DIR || "/var/odin/state/gmail",
  };
}
```

**Step 4: Run test to verify it passes**

Run: `cd plugins/gmail && npx vitest run tests/config.test.ts`
Expected: PASS — 2 tests

**Step 5: Create gmail-client.ts**

Create `plugins/gmail/src/gmail-client.ts`:

```typescript
import { google, type gmail_v1 } from "googleapis";
import { OAuth2Client } from "google-auth-library";

let cachedClient: gmail_v1.Gmail | null = null;

export function getGmailClient(): gmail_v1.Gmail {
  if (cachedClient) return cachedClient;

  const token = process.env.ODIN_GMAIL_TOKEN;
  if (!token) {
    throw new Error("ODIN_GMAIL_TOKEN not set — run 'odin gmail connect' first");
  }

  let credentials: { access_token: string; refresh_token: string; client_id: string; client_secret: string };
  try {
    credentials = JSON.parse(token);
  } catch {
    throw new Error("ODIN_GMAIL_TOKEN is not valid JSON");
  }

  const oauth2 = new OAuth2Client(credentials.client_id, credentials.client_secret);
  oauth2.setCredentials({
    access_token: credentials.access_token,
    refresh_token: credentials.refresh_token,
  });

  cachedClient = google.gmail({ version: "v1", auth: oauth2 });
  return cachedClient;
}

export async function listMessages(
  client: gmail_v1.Gmail,
  query: string,
  maxResults: number = 20,
): Promise<gmail_v1.Schema$Message[]> {
  const res = await client.users.messages.list({
    userId: "me",
    q: query,
    maxResults,
  });
  return res.data.messages || [];
}

export async function getMessage(
  client: gmail_v1.Gmail,
  messageId: string,
): Promise<gmail_v1.Schema$Message> {
  const res = await client.users.messages.get({
    userId: "me",
    id: messageId,
    format: "full",
  });
  return res.data;
}

export async function applyLabel(
  client: gmail_v1.Gmail,
  messageId: string,
  labelId: string,
): Promise<void> {
  await client.users.messages.modify({
    userId: "me",
    id: messageId,
    requestBody: { addLabelIds: [labelId] },
  });
}

export async function archiveMessage(
  client: gmail_v1.Gmail,
  messageId: string,
): Promise<void> {
  await client.users.messages.modify({
    userId: "me",
    id: messageId,
    requestBody: { removeLabelIds: ["INBOX"] },
  });
}

export async function trashMessage(
  client: gmail_v1.Gmail,
  messageId: string,
): Promise<void> {
  await client.users.messages.trash({ userId: "me", id: messageId });
}

export async function createDraft(
  client: gmail_v1.Gmail,
  to: string,
  subject: string,
  body: string,
  threadId?: string,
): Promise<string> {
  const raw = Buffer.from(
    `To: ${to}\r\nSubject: ${subject}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n${body}`,
  ).toString("base64url");

  const res = await client.users.drafts.create({
    userId: "me",
    requestBody: {
      message: { raw, threadId },
    },
  });
  return res.data.id || "";
}

export async function sendDraft(
  client: gmail_v1.Gmail,
  draftId: string,
): Promise<string> {
  const res = await client.users.drafts.send({
    userId: "me",
    requestBody: { id: draftId },
  });
  return res.data.id || "";
}

export async function getLabels(
  client: gmail_v1.Gmail,
): Promise<gmail_v1.Schema$Label[]> {
  const res = await client.users.labels.list({ userId: "me" });
  return res.data.labels || [];
}

export async function ensureLabel(
  client: gmail_v1.Gmail,
  name: string,
): Promise<string> {
  const labels = await getLabels(client);
  const existing = labels.find((l) => l.name === name);
  if (existing) return existing.id!;

  const res = await client.users.labels.create({
    userId: "me",
    requestBody: { name, labelListVisibility: "labelShow", messageListVisibility: "show" },
  });
  return res.data.id!;
}

export async function getHistory(
  client: gmail_v1.Gmail,
  startHistoryId: string,
): Promise<gmail_v1.Schema$History[]> {
  const res = await client.users.history.list({
    userId: "me",
    startHistoryId,
    historyTypes: ["messageAdded"],
  });
  return res.data.history || [];
}
```

**Step 6: Build and verify**

Run: `cd plugins/gmail && npm run build`
Expected: Compiles without errors

**Step 7: Commit**

```bash
git add plugins/gmail/src/config.ts plugins/gmail/src/gmail-client.ts plugins/gmail/tests/config.test.ts
git commit -m "feat(gmail): add config loader and Gmail API client wrapper"
```

---

### Task 4: Triage Rules Engine

**Files:**
- Create: `plugins/gmail/src/triage.ts`
- Create: `plugins/gmail/config/gmail-rules.yaml`
- Create: `plugins/gmail/tests/triage.test.ts`

**Step 1: Write the failing test**

Create `plugins/gmail/tests/triage.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { loadRules, evaluateRules, type TriageRule, type MessageMeta } from "../src/triage.js";
import * as fs from "node:fs";
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
      from: "no-reply@receipt.example.com",
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
```

**Step 2: Run test to verify it fails**

Run: `cd plugins/gmail && npx vitest run tests/triage.test.ts`
Expected: FAIL — cannot resolve `../src/triage.js`

**Step 3: Create the default rules file**

Create `plugins/gmail/config/gmail-rules.yaml`:

```yaml
schema_version: 1
account: personal

rules:
  - name: receipts
    match:
      from_pattern: "(receipt|order|invoice|payment)@"
      or_subject_pattern: "(order confirm|receipt|invoice)"
    actions:
      - label: "Receipts"
      - archive: true

  - name: newsletters
    match:
      headers:
        List-Unsubscribe: present
    actions:
      - label: "Newsletters"
      - archive: true

  - name: spam_candidates
    match:
      subject_pattern: "(urgent|act now|limited time|winner|congratulations)"
    actions:
      - label: "Spam/Review"
      - request_trash: true

  - name: needs_reply
    match:
      is_direct: true
      from_in_contacts: true
      has_question: true
    actions:
      - label: "Needs Reply"
      - draft_reply: true

  - name: uncategorized
    match:
      no_rule_matched: true
    actions:
      - label: "Triage/Review"
```

**Step 4: Create triage.ts**

Create `plugins/gmail/src/triage.ts`:

```typescript
import * as fs from "node:fs";
import { parse as parseYaml } from "yaml";

export interface MessageMeta {
  id: string;
  threadId: string;
  from: string;
  to: string;
  subject: string;
  headers: Record<string, string>;
  snippet: string;
}

export interface TriageAction {
  label?: string;
  archive?: boolean;
  request_trash?: boolean;
  draft_reply?: boolean;
}

export interface TriageMatch {
  from_pattern?: string;
  or_subject_pattern?: string;
  subject_pattern?: string;
  headers?: Record<string, string>;
  is_direct?: boolean;
  from_in_contacts?: boolean;
  has_question?: boolean;
  no_rule_matched?: boolean;
}

export interface TriageRule {
  name: string;
  match: TriageMatch;
  actions: TriageAction[];
}

export interface TriageResult {
  name: string;
  actions: TriageAction[];
}

export function loadRules(rulesPath: string): TriageRule[] {
  const content = fs.readFileSync(rulesPath, "utf-8");
  const parsed = parseYaml(content) as { rules: TriageRule[] };
  return parsed.rules || [];
}

function matchesPattern(text: string, pattern: string): boolean {
  try {
    return new RegExp(pattern, "i").test(text);
  } catch {
    return false;
  }
}

function matchesRule(rule: TriageRule, msg: MessageMeta): boolean {
  const m = rule.match;

  // Catch-all rule
  if (m.no_rule_matched) return true;

  // Skip LLM-dependent matchers at this level (handled by plugin orchestration)
  if (m.has_question || m.from_in_contacts || m.is_direct) {
    // These require external context — only match if all non-contextual
    // conditions also match. For MVP, skip rules that ONLY have contextual matchers.
    const hasNonContextual = m.from_pattern || m.or_subject_pattern || m.subject_pattern || m.headers;
    if (!hasNonContextual) return false;
  }

  let matched = false;

  if (m.from_pattern) {
    if (matchesPattern(msg.from, m.from_pattern)) matched = true;
  }

  if (m.or_subject_pattern) {
    if (matchesPattern(msg.subject, m.or_subject_pattern)) matched = true;
  }

  if (m.subject_pattern) {
    if (!matchesPattern(msg.subject, m.subject_pattern)) return false;
    matched = true;
  }

  if (m.headers) {
    for (const [key, value] of Object.entries(m.headers)) {
      if (value === "present") {
        if (!(key in msg.headers)) return false;
        matched = true;
      }
    }
  }

  return matched;
}

export function evaluateRules(
  rules: TriageRule[],
  msg: MessageMeta,
): TriageResult | null {
  for (const rule of rules) {
    if (matchesRule(rule, msg)) {
      return { name: rule.name, actions: rule.actions };
    }
  }
  return null;
}
```

**Step 5: Run test to verify it passes**

Run: `cd plugins/gmail && npx vitest run tests/triage.test.ts`
Expected: PASS — 5 tests

**Step 6: Commit**

```bash
git add plugins/gmail/src/triage.ts plugins/gmail/config/ plugins/gmail/tests/triage.test.ts
git commit -m "feat(gmail): add triage rules engine with YAML config and pattern matching"
```

---

### Task 5: Wire Event Handlers

**Files:**
- Modify: `plugins/gmail/src/index.ts`
- Create: `plugins/gmail/tests/event-loop.test.ts`

**Step 1: Write the failing test**

Create `plugins/gmail/tests/event-loop.test.ts`:

```typescript
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
```

**Step 2: Run test to verify it fails**

Run: `cd plugins/gmail && npx vitest run tests/event-loop.test.ts`
Expected: FAIL — cannot resolve `../src/handlers.js`

**Step 3: Create handlers.ts**

Create `plugins/gmail/src/handlers.ts`:

```typescript
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
  // Execution happens in index.ts with the Gmail client
  // This function is a placeholder for the dispatch logic
  const capId = (event.payload.capability_id as string) || "";
  return [{
    action: "enqueue_task",
    task_type: "gmail.result",
    project: event.project,
    reason: `Executed ${capId}`,
    payload: { status: "executed", capability: capId },
  }];
}
```

**Step 4: Run test to verify it passes**

Run: `cd plugins/gmail && npx vitest run tests/event-loop.test.ts`
Expected: PASS — 3 tests

**Step 5: Update index.ts to use handlers**

Replace the `handleEvent` function in `plugins/gmail/src/index.ts`:

```typescript
import * as readline from "node:readline";
import type { EventEnvelope, PluginDirective } from "./protocol.js";
import { handleTaskReceived } from "./handlers.js";
import { loadConfig } from "./config.js";
import { getGmailClient, listMessages, getMessage, applyLabel, archiveMessage, trashMessage, createDraft, sendDraft, ensureLabel, getHistory } from "./gmail-client.js";
import { loadRules, evaluateRules, type MessageMeta } from "./triage.js";
import * as fs from "node:fs";

const config = loadConfig();

function emit(directive: PluginDirective): void {
  process.stdout.write(JSON.stringify(directive) + "\n");
}

function emitNoop(): void {
  emit({ action: "noop" });
}

function extractHeaders(msg: { payload?: { headers?: Array<{ name?: string; value?: string }> } }): Record<string, string> {
  const headers: Record<string, string> = {};
  for (const h of msg.payload?.headers || []) {
    if (h.name && h.value) headers[h.name] = h.value;
  }
  return headers;
}

function toMessageMeta(msg: { id?: string; threadId?: string; snippet?: string; payload?: { headers?: Array<{ name?: string; value?: string }> } }): MessageMeta {
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
          // Emit a sensitive capability request for trash
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
```

**Step 6: Build and run all tests**

Run: `cd plugins/gmail && npm run build && npx vitest run`
Expected: All tests pass, build succeeds

**Step 7: Commit**

```bash
git add plugins/gmail/src/ plugins/gmail/tests/
git commit -m "feat(gmail): wire event handlers with triage execution and capability routing"
```

---

### Task 6: n8n Webhook Workflow

**Files:**
- Create: `scripts/odin/n8n-workflows/odin-gmail-push.json` (in odin-orchestrator)

**Step 1: Create the n8n workflow JSON**

This workflow receives Google Pub/Sub push notifications and dispatches them to the Odin inbox.

Create `/home/orchestrator/odin-orchestrator/scripts/odin/n8n-workflows/odin-gmail-push.json`:

```json
{
  "name": "odin-gmail-push",
  "nodes": [
    {
      "parameters": {
        "httpMethod": "POST",
        "path": "gmail-push",
        "responseMode": "onReceived",
        "responseCode": 200,
        "options": {}
      },
      "id": "webhook-gmail",
      "name": "Gmail Push Webhook",
      "type": "n8n-nodes-base.webhook",
      "typeVersion": 2,
      "position": [250, 300]
    },
    {
      "parameters": {
        "jsCode": "const body = $input.first().json.body || $input.first().json;\nconst message = body.message || {};\nconst data = message.data ? Buffer.from(message.data, 'base64').toString() : '{}';\nconst parsed = JSON.parse(data);\nconst historyId = parsed.historyId || '';\nconst emailAddress = parsed.emailAddress || '';\nconst ts = Date.now();\nconst rand = Math.random().toString(36).slice(2, 6);\n\nreturn [{\n  json: {\n    schema_version: 1,\n    task_id: `gmail-push-${ts}-${rand}`,\n    type: 'gmail_triage',\n    source: 'n8n',\n    created_at: new Date().toISOString(),\n    payload: {\n      task_type: 'gmail_triage',\n      history_id: historyId,\n      account: 'personal',\n      email_address: emailAddress\n    }\n  }\n}];"
      },
      "id": "build-envelope",
      "name": "Build Task Envelope",
      "type": "n8n-nodes-base.code",
      "typeVersion": 2,
      "position": [470, 300]
    },
    {
      "parameters": {
        "command": "=printf '%s' '{{ JSON.stringify($json) }}' | ssh -i /home/node/.ssh/odin_ingress -o StrictHostKeyChecking=no -o ConnectTimeout=5 orchestrator@172.17.0.1"
      },
      "id": "dispatch-inbox",
      "name": "Dispatch to Odin Inbox",
      "type": "n8n-nodes-base.executeCommand",
      "typeVersion": 1,
      "position": [690, 300]
    }
  ],
  "connections": {
    "Gmail Push Webhook": { "main": [[{ "node": "Build Task Envelope", "type": "main", "index": 0 }]] },
    "Build Task Envelope": { "main": [[{ "node": "Dispatch to Odin Inbox", "type": "main", "index": 0 }]] }
  },
  "settings": { "executionOrder": "v1" },
  "tags": [{ "name": "odin" }, { "name": "gmail" }]
}
```

**Step 2: Commit (in odin-orchestrator)**

```bash
cd /home/orchestrator/odin-orchestrator
git add scripts/odin/n8n-workflows/odin-gmail-push.json
git commit -m "feat(gmail): add n8n webhook workflow for Gmail Pub/Sub push notifications"
```

---

### Task 7: Smoke Test

**Files:**
- Create: `plugins/gmail/tests/smoke.test.ts` (in odin-core)

**Step 1: Write the smoke test**

This test verifies the full event loop without a real Gmail connection — it pipes JSON to the built plugin and checks stdout.

Create `plugins/gmail/tests/smoke.test.ts`:

```typescript
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
```

**Step 2: Build and run smoke test**

Run: `cd plugins/gmail && npm run build && npx vitest run tests/smoke.test.ts`
Expected: PASS — 4 tests

**Step 3: Commit**

```bash
git add plugins/gmail/tests/smoke.test.ts
git commit -m "test(gmail): add plugin smoke tests for event loop and capability routing"
```

---

### Task 8: Task Routing & Documentation

**Files:**
- Modify: `scripts/odin/odin-inbox-processor.sh` (in odin-orchestrator) — add `gmail_triage` routing
- Create: `plugins/gmail/README.md` (in odin-core)

**Step 1: Add gmail_triage to task routing (odin-orchestrator)**

In `/home/orchestrator/odin-orchestrator/scripts/odin/odin-inbox-processor.sh`, add `gmail_triage` to the route_task function, mapping to a gmail-capable worker or the plugin directly. Find the routing case statement and add:

```bash
gmail_triage|gmail_monitor)
    AGENT="worker-1"
    PROMPT_PATH="scripts/odin/agent-prompts/worker.md"
    ;;
```

**Step 2: Create plugin README**

Create `plugins/gmail/README.md`:

```markdown
# Odin Gmail Plugin

Gmail inbox triage assistant — auto-labels, archives, deletes spam, unsubscribes, and drafts replies.

## Setup

1. Create Google Cloud project with Gmail API enabled
2. Create OAuth2 credentials (Desktop app type)
3. Run: `odin gmail connect`
4. Deploy rules: `cp config/gmail-rules.yaml /var/odin/config/gmail-rules.yaml`
5. Deploy n8n workflow: import `odin-gmail-push.json`
6. Register Pub/Sub push subscription pointing to `https://n8n.marcusgoll.com/webhook/gmail-push`

## Configuration

Edit `/var/odin/config/gmail-rules.yaml` to customize triage rules.

## Capabilities

| Capability | Risk | Description |
|---|---|---|
| gmail.inbox.list | safe | List inbox messages |
| gmail.message.read | safe | Read message body |
| gmail.label.apply | safe | Apply/remove labels |
| gmail.thread.archive | safe | Archive (reversible) |
| gmail.draft.create | safe | Create draft |
| gmail.unsubscribe | sensitive | Unsubscribe from list |
| gmail.draft.send | sensitive | Send draft |
| gmail.message.trash | sensitive | Move to trash |
| gmail.message.delete | destructive | Permanent delete |

## Development

```bash
npm install
npm run build
npm test
```
```

**Step 3: Commit both repos**

odin-orchestrator:
```bash
cd /home/orchestrator/odin-orchestrator
git add scripts/odin/odin-inbox-processor.sh
git commit -m "feat(routing): add gmail_triage task type to inbox processor routing"
```

odin-core:
```bash
cd /home/orchestrator/odin-core
git add plugins/gmail/README.md
git commit -m "docs(gmail): add plugin README with setup and capability reference"
```

---

## Verification

After all 8 tasks:

1. **Unit tests pass:** `cd plugins/gmail && npx vitest run` — all protocol, config, triage, handler, and smoke tests green
2. **Build succeeds:** `cd plugins/gmail && npm run build` — clean TypeScript compilation
3. **Smoke test:** `echo '{"event_id":"t","event_type":"task.received","payload":{"task_type":"gmail_triage"}}' | node dist/index.js serve` — outputs `request_capability` directive
4. **Manifest validates:** `jq empty plugins/gmail/odin.plugin.yaml` is valid YAML (use `python3 -c "import yaml; yaml.safe_load(open('plugins/gmail/odin.plugin.yaml'))"`)
5. **Rules parse:** `node -e "import('./dist/triage.js').then(m => console.log(m.loadRules('config/gmail-rules.yaml')))"` — prints parsed rules
