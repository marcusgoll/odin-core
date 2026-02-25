# Odin Skill + Plugin Governance Design

## Goal

Enable Odin to discover, install, validate, and apply skills/plugins across `global/project/user` scopes, and delegate the minimum required capabilities to sub-agents with auditable, verifiable outcomes.

## Constraints

- LLM-agnostic implementation.
- Skills are untrusted until proven otherwise.
- Stagehand/browser automation is high risk and disabled by default.
- Never claim installed/enabled/working without verification.
- Ask at most 1-3 targeted questions only when blocked, while still proposing a safe provisional path.

## Current State (Repo Evidence)

- Plugin install and verification flow exists in `odin-plugin-manager` (source resolution, checksum, optional signature verification, manifest parsing).
- Policy engine is default-deny capability based and can require approval for destructive actions.
- Runtime already emits audit records for decisions and outcomes.
- No first-class in-repo skill registry or scoped skill resolver is present yet.

## Approaches Considered

### A) File-only control plane

Use scoped registry files and file audit logs only.

Pros:
- simple operations
- transparent and git-friendly

Cons:
- limited queryability for audits and drift analysis

### B) DB-only control plane

Store registries and audit records in SQLite only.

Pros:
- strong query and consistency semantics

Cons:
- higher operational/runtime coupling

### C) Hybrid control plane (recommended)

Use scoped file registries for deterministic behavior plus append-only audit logs; optionally index audit logs later.

Pros:
- fastest fit for existing repo patterns
- explicit scope resolution and human-readable state
- clear migration path to richer indexing

Cons:
- requires schema/version discipline across registries and audits

## Approved Design

### 1) Skill Registry + Trust Model

Create scoped registries with precedence `user > project > global`.

Each skill record includes:
- `name`
- `description`
- `scope`
- `source`
- `pinned_version`
- `trust_level` (`TRUSTED|CAUTION|UNTRUSTED`)
- `required_tools`
- `allowed_commands`
- `when_to_use`
- `scripts_present`
- `last_verified_at`

Trust assignment:
- `TRUSTED`: local/repo-authored or reviewed and pinned source.
- `CAUTION`: remote pinned tag/commit, SKILL.md reviewed, scripts scanned.
- `UNTRUSTED`: unpinned remote, unclear scripts, or unresolved provenance/risk.

### 2) Required Skill Import Protocol

For any install/import request:
1. Discover candidates (local first).
2. Show provenance (`source`, version/commit, target scope, trust level).
3. Preview SKILL summary and included files.
4. Run lightweight risk scan for shell execution, `curl|sh`, network calls, credential handling, file deletion.
5. Require explicit acknowledgement for `UNTRUSTED` and any skill executing scripts or touching secrets.
6. Install into chosen scope and record registry entry.
7. Verify discoverability by Odin and target agent(s).

### 3) Skill Activation Rules

- Progressive disclosure by default.
- Auto-activation only for `TRUSTED` skills with clear trigger match.
- `CAUTION`/`UNTRUSTED` skills require explicit invocation or confirmation.
- Conflict resolution: precedence first (`user > project > global`), then newest verified entry.

### 4) Plugin Registry + Permission Envelope

Maintain plugin registry entries with:
- `name`
- `capabilities`
- `risk_tier`
- `permissions`
- `audit_log_path`
- `approved_by`
- `approved_at`

Permission object enforces least privilege:
- `allowed_workspaces`
- `allowed_domains` (web plugins)
- `allowed_commands`
- `data_handling` policy (`allow_pii`, retention, artifact boundaries)

High-risk actions remain denied without explicit policy grants.

### 5) Stagehand Plugin Safety Policy

Default:
- disabled
- READ/OBSERVE mode only
- disallowed actions default deny:
  - login
  - payment/purchase
  - PII form submit
  - file upload

Escalation to ACT is allowed only when:
- required for task objective,
- explicitly allowed by permissions,
- and approval is logged.

Hard prohibitions:
- purchases/payments
- destructive account actions
- credential entry unless user explicitly provides credentials in-session and requests login

Fallback:
- if Stagehand is unavailable or blocked, use text-based web research and log fallback reason.

### 6) Capability Manifest for Sub-Agents

Every spawned sub-agent receives a per-task manifest:
- `task_id`, `task_goal`, `expires_at`
- enabled skills (`name`, `scope`, `trust_level`, `version`)
- enabled plugins (`name`, `permissions`, `risk_tier`)
- explicit `allowed_commands`
- explicit `disallowed_actions`
- required verification evidence/artifacts

Delegation rules:
- one atomic task per sub-agent
- no implicit capability inheritance
- browser tasks only assigned to dedicated Browser Agent with Stagehand permissions
- non-browser agents must request re-delegation through Odin for browser needs

### 7) Auditability Requirements

Persist auditable records for:
- skill installs (source, pinned version, scope, trust, reviewer)
- plugin enablement (approver, permissions, timestamps)
- task execution (capabilities used, policy decisions, artifacts, verification status)

Status semantics:
- `verified`
- `partial`
- `failed`

No success claim without evidence for required checks.

### 8) Error Handling + Recovery

Failure classes:
- `DISCOVERY_ERROR`
- `PROVENANCE_ERROR`
- `RISK_SCAN_FAIL`
- `ACK_REQUIRED`
- `PERMISSION_DENIED`
- `VERIFICATION_FAIL`

On failure:
- transition to `RECOVERY`
- do not auto-escalate privileges
- provide one safe provisional path
- ask up to 1-3 targeted questions only if blocked

## Verification / Definition of Done

Required checks:
- scope precedence works (`user > project > global`)
- untrusted skill install blocked without acknowledgement
- script-bearing skill blocked without explicit approval
- Stagehand blocked outside allowed domain/workspace boundaries
- non-browser agents lack browser powers
- install/enable/delegate/verify events are present in audit trail

Done when:
- all checks pass and logs/artifacts prove each step.

## Out of Scope (This Design)

- Marketplace UX and ranking/recommendation systems.
- Cross-organization trust sharing/federation.
- Purchasing/payment automation.
