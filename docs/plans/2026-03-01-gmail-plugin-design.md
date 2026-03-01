# Gmail Plugin Design — Personal Assistant for Inbox Triage

**Date:** 2026-03-01
**Status:** Approved
**Approach:** Native Odin Plugin (Approach A)

## Overview

Add a Gmail plugin to Odin that acts as a personal assistant for inbox triage — auto-labeling, archiving, spam deletion, unsubscribing from mailing lists, and drafting replies. The plugin follows the stagehand pattern: TypeScript external process, capability-scoped, policy-governed.

## Architecture

```
Google Gmail ──Pub/Sub──▶ n8n webhook ──inbox task──▶ Odin Orchestrator
                                                           │
                                                      dispatch task
                                                           │
                                                      Gmail Plugin
                                                      (TypeScript)
                                                           │
                                                   request_capability
                                                           │
                                                      Policy Engine
                                                      safe → auto
                                                      sensitive → Telegram
```

**Flow:**

1. Gmail pushes notification via Pub/Sub to n8n webhook endpoint
2. n8n creates a `gmail_triage` task in Odin's inbox
3. Orchestrator dispatches task to the Gmail plugin (external process)
4. Plugin fetches new messages since last `historyId`, evaluates triage rules
5. Plugin requests capabilities (read, label, archive, trash, draft, unsubscribe)
6. Policy engine auto-approves safe actions, routes sensitive ones to Telegram
7. Plugin executes approved actions via Gmail API, enqueues results

## Authentication

- **Method:** Google OAuth2 (Desktop app type)
- **Scopes:** `gmail.modify`, `gmail.compose`, `gmail.labels`
- **Token storage:** Odin secret bindings (`secret://personal/gmail/oauth_token`)
- **Refresh:** Plugin auto-refreshes expired access tokens using refresh token
- **Revocation recovery:** Enqueues `gmail.auth_expired` alert → Telegram notification
- **Setup:** `odin gmail connect` one-time flow (prints auth URL, exchanges code for tokens)

## Capabilities & Risk Tiers

| Capability | Risk Tier | Approval | Description |
|---|---|---|---|
| `gmail.inbox.list` | safe | Auto | List message metadata |
| `gmail.message.read` | safe | Auto | Read full message body |
| `gmail.label.list` | safe | Auto | List available labels |
| `gmail.label.apply` | safe | Auto | Apply/remove labels |
| `gmail.thread.archive` | safe | Auto | Archive threads (reversible) |
| `gmail.thread.mark_read` | safe | Auto | Mark as read |
| `gmail.draft.create` | safe | Auto | Create draft (no send) |
| `gmail.unsubscribe` | sensitive | Telegram | Unsubscribe from mailing list |
| `gmail.draft.send` | sensitive | Telegram | Send a drafted reply |
| `gmail.message.trash` | sensitive | Telegram | Move to trash |
| `gmail.message.delete` | destructive | Telegram | Permanently delete |

**Principle:** Reversible actions are auto-approved. Actions that leave the inbox or send outbound require Telegram confirmation.

## Triage Rules Engine

Rules defined in `/var/odin/config/gmail-rules.yaml`:

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
      from_not_in_contacts: true
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

**Evaluation:** Top-to-bottom, first match wins. Content-based matchers (`has_question`) use Claude via the orchestrator's LLM routing for lightweight classification.

## Pub/Sub Webhook & n8n Integration

- **Pub/Sub topic:** `projects/odin-personal/topics/gmail-push`
- **Push subscription:** `https://n8n.marcusgoll.com/webhook/gmail-push`
- **Watch registration:** `gmail.users.watch()` — expires every 7 days, n8n cron re-registers every 6 days
- **Dedup:** Plugin tracks `last_history_id` in `/var/odin/state/gmail/last_history_id`
- **Idempotency:** Duplicate historyIds are skipped; gaps are filled by fetching delta

## File Layout

### odin-core

```
plugins/gmail/
├── odin.plugin.yaml
├── package.json
├── tsconfig.json
├── src/
│   ├── index.ts              # stdin/stdout event loop
│   ├── gmail-client.ts       # Gmail API wrapper
│   ├── triage.ts             # Rule engine
│   ├── classifier.ts         # LLM classification requests
│   └── types.ts              # Shared types
└── tests/
    ├── triage.test.ts
    └── classifier.test.ts

config/gmail-rules.yaml       # default rules (deployed to /var/odin/config/)
```

### odin-orchestrator

```
scripts/odin/n8n-workflows/odin-gmail-push.json
scripts/odin/tests/gmail-plugin-smoke-test.sh
```

### Runtime state (/var/odin/)

```
state/gmail/last_history_id
config/gmail-rules.yaml
```

## Account Model

MVP: single personal Gmail account. The rules config and secret bindings are keyed by `account` field, making multi-account a config-only change later (no code changes needed).

## Governance

- Plugin permission envelope registered in governance registry
- Risk tier overrides declared in manifest
- All actions logged to `events.jsonl`
- Telegram approval gates for sensitive/destructive capabilities

## Decisions

1. **TypeScript over Python** — matches stagehand, uses googleapis npm package
2. **Pub/Sub over polling** — near real-time, official Google-supported push mechanism
3. **Rules YAML over hardcoded** — declarative, editable, versionable
4. **LLM classification via orchestrator** — plugin doesn't embed its own model, delegates to existing routing
5. **No `mail.google.com` scope** — request only `gmail.modify`, `gmail.compose`, `gmail.labels` (least privilege)
