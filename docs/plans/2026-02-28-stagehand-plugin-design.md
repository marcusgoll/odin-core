# Stagehand Odin-Core Plugin — Design Document

**Date:** 2026-02-28
**Status:** Approved
**Author:** Odin Orchestrator

## Overview

Add [Stagehand](https://www.stagehand.dev/) as a native Odin-core plugin, providing AI-powered browser automation for testing, QA, and data extraction. Initial validation against cfipros.com.

## Decision Record

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Extension type | Odin-core plugin (not skill) | Capability-gated, out-of-process isolation, follows plugin contract v1 |
| Runtime | Local headless Chrome on homelab | Free, no external dependency, fits existing infrastructure |
| Approach | Native Node.js plugin | Stagehand is TypeScript-native; plugin protocol supports any language |
| LLM providers | Anthropic Claude + OpenAI | API keys required (Max/Pro subscription plans not supported by Stagehand) |
| Browser service | Browserbase not used | Local-only for now; can add later if needed |

## Plugin Structure

```
odin-core/plugins/stagehand/
├── odin.plugin.yaml          # Plugin manifest (contract v1)
├── package.json              # Node.js dependencies
├── tsconfig.json             # TypeScript config
├── src/
│   ├── index.ts              # Entrypoint — plugin server (stdin/stdout JSON protocol)
│   ├── capabilities/
│   │   ├── navigate.ts       # browser.navigate — go to URL, wait for load
│   │   ├── act.ts            # browser.act — natural language actions
│   │   ├── extract.ts        # browser.extract — structured data extraction
│   │   ├── observe.ts        # browser.observe — discover available actions
│   │   └── agent.ts          # browser.agent — multi-step autonomous workflows
│   ├── config.ts             # LLM provider config, browser options
│   └── protocol.ts           # Odin plugin protocol types
├── tests/
│   ├── smoke.test.ts         # Phase 1: cfipros.com smoke tests
│   ├── extract.test.ts       # Phase 2: structured data extraction
│   └── journey.test.ts       # Phase 3: authenticated user journeys
└── .env.example              # Required env vars template
```

## Plugin Manifest

```yaml
schema_version: 1
plugin:
  name: odin.stagehand
  version: 0.1.0
  runtime: external-process
  entrypoint:
    command: node
    args: ["dist/index.js", "serve"]
  compatibility:
    core_version: ">=0.1.0 <1.0.0"
  capabilities:
    - id: network.http
      scope: [allowlisted_domains]
    - id: browser.navigate
      scope: [allowlisted_domains]
    - id: browser.act
      scope: [allowlisted_domains]
    - id: browser.extract
      scope: [allowlisted_domains]
    - id: browser.agent
      scope: [allowlisted_domains]
  hooks:
    - event: task.received
      handler: on_task_received
  storage:
    - kind: kv
      name: session_state
      quota_mb: 100
```

## Protocol Bridge

Communication via JSON-over-stdin/stdout (external-process convention).

### Request Flow

```
odin-core-runtime                          stagehand-plugin (Node.js)
      │                                          │
      │──── ActionRequest (JSON) ────────────────▶│
      │     {                                     │
      │       request_id: "abc",                  │  1. Parse request
      │       capability: "browser.extract",      │  2. Route to handler
      │       input: {                            │  3. Call stagehand API
      │         url: "https://cfipros.com",       │  4. Return result
      │         instruction: "extract pricing",   │
      │         schema: { ... }                   │
      │       }                                   │
      │     }                                     │
      │                                           │
      │◀──── ActionOutcome (JSON) ────────────────│
      │     {                                     │
      │       request_id: "abc",                  │
      │       status: "Executed",                 │
      │       output: { pricing: [...] }          │
      │     }                                     │
```

### Browser Lifecycle

- Stagehand instance created on first request, reused across requests (warm browser)
- `domSettleTimeout: 30000` for page stability
- `selfHeal: true` for resilient automation
- Idle timeout: 5 minutes — browser closes if no requests, cold-starts on next

### LLM Configuration

```typescript
// Primary: Anthropic Claude for high-quality reasoning
const primaryModel = "anthropic/claude-sonnet-4-6";
// Fallback: OpenAI for cost-effective routine tasks
const fallbackModel = "openai/gpt-4o-mini";
```

Environment variables:
- `ANTHROPIC_API_KEY` — for Claude models
- `OPENAI_API_KEY` — for OpenAI models
- `STAGEHAND_HEADLESS` — `true` (default) or `false` for debugging

## Progressive Testing on cfipros.com

### Phase 1 — Smoke Tests

- Navigate to `https://cfipros.com` (marketing site, no auth)
- `observe()`: discover available page elements
- `act("scroll to pricing section")`
- `extract("the page title and meta description")`
- Verify: page loads, key sections exist, no JS errors
- Success criteria: all smoke tests pass, < 30s per action

### Phase 2 — Data Extraction

- Extract structured data from cfipros.com using Zod schemas:
  - Pricing tiers (plan names, prices, features)
  - Feature list from homepage
  - FAQ content
- Compare extracted data against known values (regression guard)
- Success criteria: extraction matches expected schema, >90% field accuracy

### Phase 3 — Authenticated User Journey

- Use `stagehand.agent()` for multi-step flows on `app.cfipros.com`:
  - Navigate to login page
  - Fill credentials (using Stagehand `variables` for secret protection)
  - Verify dashboard loads
  - Navigate to a feature page
  - Perform an action (e.g., view a student record)
- Session state saved to plugin KV storage for reuse
- Success criteria: agent completes full journey autonomously, < 2 min total

Domain access: cfipros.com and app.cfipros.com are Tier 1 (auto-allowed).

## Error Handling

| Error Type | Response |
|------------|----------|
| Element not found / timeout | `ActionOutcome.status: "Failed"`, descriptive detail |
| Browser crash | Auto-restart, retry once, then fail |
| LLM rate limit | Exponential backoff (3 retries: 1s, 2s, 4s) |
| Domain not in allowlist | `ActionOutcome.status: "Blocked"`, reason: `domain_not_allowed` |

## Integration Points

- Installed and verified by `odin-plugin-manager` (SHA256 checksum)
- Agent-invocable via task dispatch (`capability: "browser.extract"`, etc.)
- Audit trail via `odin-audit` event system
- Secrets via `odin-secrets` (reference-only, never in logs)

## Explicitly Out of Scope (YAGNI)

- Browserbase cloud integration
- Visual regression testing (existing Playwright suite)
- Custom MCP server integration
- Streaming agent output
- Multi-browser parallel sessions
