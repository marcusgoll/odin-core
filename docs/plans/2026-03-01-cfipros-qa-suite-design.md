# CFIPros QA Suite with Stagehand — Design Document

**Date:** 2026-03-01
**Status:** Approved
**Author:** Odin Orchestrator

## Overview

Use the Stagehand plugin to build a comprehensive browser-based QA suite for CFIPros. Two goals:

1. **QA all shipped features** — automated E2E tests for auth, logbook, endorsements, readiness/audit, AKTR, experience tracking, CFI dashboard, and DPE packets
2. **Pre-write Maneuver UI test specs** — E2E tests for Task #6 (critical path), initially skipped, enabled when the feature ships

## Decision Record

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Test location | `plugins/stagehand/tests/qa/` | Tests demonstrate Stagehand capabilities against a real app |
| Test accounts | Self-service signup via Stagehand | Exercises the signup flow as QA test #1 |
| Email verification | Database bypass script | No email provider API dependency; works across Unosend→AWS migration |
| Test runner | Vitest (already configured) | Consistent with existing Stagehand test suite |
| Flakiness strategy | selfHeal + domSettle + semantic assertions | AI-powered resilience, not brittle CSS selectors |

## Test Accounts

Two accounts created via Stagehand's signup flow:

| Account | Email | Role | Purpose |
|---------|-------|------|---------|
| Student | `qa-student@cfipros.com` | student | Student feature tests |
| CFI | `qa-cfi@cfipros.com` | cfi | CFI dashboard + maneuver grading tests |

Credentials stored in `.env` (gitignored):
```
QA_STUDENT_EMAIL=qa-student@cfipros.com
QA_STUDENT_PASSWORD=<generated>
QA_CFI_EMAIL=qa-cfi@cfipros.com
QA_CFI_PASSWORD=<generated>
```

After signup, a utility script (`tests/qa/scripts/verify-accounts.ts`) connects to the CFIPros PostgreSQL database and sets `email_verified = true` and `email_verified_at = NOW()` on the `"user"` table for both accounts.

## Test Structure

```
plugins/stagehand/tests/qa/
├── scripts/
│   └── verify-accounts.ts       # DB script: mark test accounts email-verified
├── setup.ts                     # Shared setup: account existence check, login helper
├── public/
│   └── marketing.test.ts        # Homepage, navigation, pricing, CTAs
├── auth/
│   ├── signup.test.ts           # Register student + CFI accounts
│   ├── login.test.ts            # Login both accounts, session persistence, logout
│   └── password-reset.test.ts   # Request password reset flow
├── student/
│   ├── dashboard.test.ts        # Dashboard loads, readiness score visible, sidebar nav
│   ├── logbook.test.ts          # Create flight entry, edit, verify list, CSV export
│   ├── endorsements.test.ts     # View endorsements, validity indicators
│   ├── audit.test.ts            # Readiness page, GO/NO-GO status, breakdown sections
│   ├── experience.test.ts       # Hour breakdowns, currency status
│   ├── aktr.test.ts             # Upload PDF, extraction results, weak areas
│   └── dpe-packets.test.ts      # Create packet, summary, shareable link
├── cfi/
│   ├── dashboard.test.ts        # CFI dashboard, student roster
│   ├── students.test.ts         # Filter/sort, student detail, readiness data
│   ├── endorsements.test.ts     # Request history, search
│   └── link-student.test.ts     # Invite student, verify linkage
└── maneuvers/                   # Skipped until feature ships
    ├── log-maneuvers.test.ts    # CFI: select flight, pick ACS tasks, grade, submit
    ├── student-view.test.ts     # Student: read-only graded maneuvers
    └── progress.test.ts         # Maneuver progress charts render
```

## Domain Allowlist

Already configured in `config.ts`:
- `cfipros.com` — marketing site
- `app.cfipros.com` — authenticated app

## Execution Order

```
Sequential (account setup):
  1. signup.test.ts        → Creates qa-student + qa-cfi accounts
  2. verify-accounts.ts    → Marks accounts email-verified in DB
  3. login.test.ts         → Verifies both accounts can log in
  4. link-student.test.ts  → CFI invites student, student accepts

Independent (run in any order after setup):
  5. student/*.test.ts     → All student feature tests
  6. cfi/*.test.ts         → All CFI feature tests
  7. public/*.test.ts      → Marketing site tests
```

## Timeouts

| Action Type | Timeout |
|-------------|---------|
| Navigation / page load | 30s |
| AI-powered actions (act/extract/observe) | 45s |
| Multi-step agent flows (signup, journeys) | 120s |

## Reliability

- `selfHeal: true` — Stagehand auto-corrects for minor DOM changes
- `domSettleTimeoutMs: 30000` — waits for SPA hydration
- Semantic assertions — test headings, roles, extracted data, not CSS selectors
- `vitest --retry=1` for integration tests
- Each test file gets its own Stagehand instance via `createTestStagehand()`

## Database Connection for Verification Script

```
PostgreSQL 15+
Table: "user"
Columns: email_verified (boolean), email_verified_at (datetime)
Connection: postgresql://cfipros:<password>@<host>:5432/cfipros
```

The script uses a synchronous `pg` client (not asyncpg) since it's a one-shot utility.

## Maneuver UI Test Specs (Pre-written)

Tests for Task #6 use `describe.skipIf(!maneuverFeatureEnabled)` with a feature flag env var `QA_MANEUVER_UI_ENABLED=true`. When the Maneuver UI ships, flip the flag to enable:

- **log-maneuvers.test.ts**: CFI navigates to `/students/[id]/log-maneuvers`, selects a flight, picks ACS tasks from the searchable list, grades each on the 4-point scale (Unsatisfactory/Needs Practice/Satisfactory/Proficient), adds notes, submits. Verify `ManeuverEntry` records appear.
- **student-view.test.ts**: Student logs in, navigates to `/dashboard/maneuvers`, sees graded maneuvers (read-only), grades match what CFI submitted.
- **progress.test.ts**: After multiple grading sessions, verify progress charts render with correct data points.

## Explicitly Out of Scope (YAGNI)

- School admin role tests (no test account for that tier)
- Stripe/billing flow tests (requires test payment methods)
- Email delivery verification (bypassed via DB)
- Visual regression screenshots (existing Playwright suite)
- Parallel browser sessions
- Mobile viewport testing
