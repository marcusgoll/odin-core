# Odin Skill + Plugin Governance Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement scoped skill/plugin governance with trust-gated installs, explicit plugin permissions (including Stagehand safety defaults), and least-privilege capability manifests for delegated sub-agents.

**Architecture:** Add a focused `odin-governance` crate for scoped registries, trust/risk evaluation, plugin permission envelopes, and capability-manifest validation. Extend protocol/runtime/CLI incrementally, reusing existing policy and audit systems. Keep v1 persistence file-backed (`user/project/global` registries + append-only audit events) and verifiable through tests and smoke scripts.

**Tech Stack:** Rust workspace crates (`odin-plugin-protocol`, `odin-governance`, `odin-core-runtime`, `odin-cli`), YAML/JSON schema files, Bash verification scripts.

---

Execution notes:
- Use `@test-driven-development` for every code path.
- Use `@verification-before-completion` before any “done” claim.

### Task 1: Add protocol contracts and schemas for governance data

**Files:**
- Modify: `crates/odin-plugin-protocol/src/lib.rs`
- Create: `schemas/skill-registry.v1.schema.json`
- Create: `schemas/capability-manifest.v1.schema.json`

**Step 1: Write failing protocol tests first**

Add tests in `crates/odin-plugin-protocol/src/lib.rs` for round-trip serde of new governance contracts:

```rust
#[test]
fn skill_registry_round_trip() {
    let registry = SkillRegistry {
        schema_version: 1,
        scope: SkillScope::Project,
        skills: vec![SkillRecord {
            name: "brainstorming".to_string(),
            trust_level: TrustLevel::Trusted,
            source: "local:/skills/brainstorming".to_string(),
            pinned_version: Some("abc123".to_string()),
            ..SkillRecord::default_for("brainstorming")
        }],
    };

    let encoded = serde_json::to_string(&registry).expect("encode");
    let decoded: SkillRegistry = serde_json::from_str(&encoded).expect("decode");
    assert_eq!(decoded.scope, SkillScope::Project);
}
```

**Step 2: Run tests to verify RED**

Run: `cargo test -p odin-plugin-protocol skill_registry_round_trip -- --nocapture`  
Expected: FAIL (missing governance types).

**Step 3: Implement minimal governance protocol structs**

In `lib.rs`, add:

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel { Trusted, Caution, Untrusted }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillScope { Global, Project, User }
```

Add `SkillRecord`, `SkillRegistry`, `PluginPermissionEnvelope`, `CapabilityManifest`, and `DelegationCapability` with defaults needed by tests.

**Step 4: Re-run tests to verify GREEN**

Run: `cargo test -p odin-plugin-protocol -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/odin-plugin-protocol/src/lib.rs schemas/skill-registry.v1.schema.json schemas/capability-manifest.v1.schema.json
git commit -m "feat(protocol): add governance contracts for skills plugins and manifests"
```

### Task 2: Create `odin-governance` crate and scoped skill resolver

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/odin-governance/Cargo.toml`
- Create: `crates/odin-governance/src/lib.rs`
- Create: `crates/odin-governance/src/skills.rs`
- Create: `crates/odin-governance/tests/skill_scope_resolution.rs`

**Step 1: Write failing precedence test**

Create `crates/odin-governance/tests/skill_scope_resolution.rs`:

```rust
#[test]
fn user_overrides_project_and_global() {
    let resolved = resolve_skill(
        "brainstorming",
        Some(user_registry()),
        Some(project_registry()),
        Some(global_registry()),
    ).expect("resolved");

    assert_eq!(resolved.scope, SkillScope::User);
}
```

**Step 2: Run test to verify RED**

Run: `cargo test -p odin-governance user_overrides_project_and_global -- --nocapture`  
Expected: FAIL (crate/module/function missing).

**Step 3: Implement minimal resolver and loaders**

In `skills.rs`, implement:

```rust
pub fn resolve_skill(name: &str, user: Option<&SkillRegistry>, project: Option<&SkillRegistry>, global: Option<&SkillRegistry>) -> Option<SkillRecord> {
    find(name, user).or_else(|| find(name, project)).or_else(|| find(name, global))
}
```

Add YAML loaders for scoped registries and normalize trust/scope metadata.

**Step 4: Re-run test to verify GREEN**

Run: `cargo test -p odin-governance --test skill_scope_resolution -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add Cargo.toml crates/odin-governance
git commit -m "feat(governance): add scoped skill registry resolver"
```

### Task 3: Implement risk scan and import-ack gating

**Files:**
- Create: `crates/odin-governance/src/risk_scan.rs`
- Create: `crates/odin-governance/src/import.rs`
- Modify: `crates/odin-governance/src/lib.rs`
- Create: `crates/odin-governance/tests/skill_import_gates.rs`

**Step 1: Write failing gate tests**

Create tests for scan findings and ack requirements:

```rust
#[test]
fn untrusted_skill_requires_ack() {
    let plan = evaluate_install(&candidate_untrusted_with_script(), Ack::None).expect("plan");
    assert_eq!(plan.status, InstallGateStatus::BlockedAckRequired);
}

#[test]
fn trusted_skill_without_scripts_can_proceed() {
    let plan = evaluate_install(&candidate_trusted_local(), Ack::None).expect("plan");
    assert_eq!(plan.status, InstallGateStatus::Allowed);
}
```

**Step 2: Run tests to verify RED**

Run: `cargo test -p odin-governance --test skill_import_gates -- --nocapture`  
Expected: FAIL (missing scanner/gate logic).

**Step 3: Implement scanner + gate evaluator**

Add lightweight scan patterns:

```rust
const HIGH_RISK_PATTERNS: &[&str] = &["curl | sh", "rm -rf", "export AWS_SECRET", "http://", "https://"];
```

Implement gate decisions:
- block/install ack-required for untrusted
- block/install ack-required for script/secret-touching skills
- allow trusted with no risky findings

**Step 4: Re-run tests to verify GREEN**

Run: `cargo test -p odin-governance --test skill_import_gates -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/odin-governance/src/lib.rs crates/odin-governance/src/risk_scan.rs crates/odin-governance/src/import.rs crates/odin-governance/tests/skill_import_gates.rs
git commit -m "feat(governance): enforce risk scan and acknowledgment gates for skill import"
```

### Task 4: Add plugin permission registry and Stagehand policy defaults

**Files:**
- Create: `crates/odin-governance/src/plugins.rs`
- Modify: `crates/odin-governance/src/lib.rs`
- Create: `crates/odin-governance/tests/stagehand_policy.rs`
- Create: `policy/stagehand.permissions.example.yaml`

**Step 1: Write failing Stagehand policy tests**

```rust
#[test]
fn stagehand_denies_login_by_default() {
    let policy = stagehand_default_policy();
    let decision = policy.evaluate(Action::Login);
    assert!(matches!(decision, PermissionDecision::Deny { .. }));
}

#[test]
fn stagehand_denies_domain_outside_allowlist() {
    let policy = stagehand_with_domains(["example.com"]);
    let decision = policy.evaluate(Action::ObserveUrl("https://not-allowed.dev".into()));
    assert!(matches!(decision, PermissionDecision::Deny { .. }));
}
```

**Step 2: Run tests to verify RED**

Run: `cargo test -p odin-governance --test stagehand_policy -- --nocapture`  
Expected: FAIL.

**Step 3: Implement permission evaluator and defaults**

Implement model enforcing:
- default disabled
- read/observe default mode
- disallowed actions: login/payment/pii_submit/file_upload
- explicit allowlists for domains/workspaces/commands

**Step 4: Re-run tests to verify GREEN**

Run: `cargo test -p odin-governance --test stagehand_policy -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/odin-governance/src/lib.rs crates/odin-governance/src/plugins.rs crates/odin-governance/tests/stagehand_policy.rs policy/stagehand.permissions.example.yaml
git commit -m "feat(governance): add plugin permission envelope and stagehand safety policy"
```

### Task 5: Enforce capability manifests in runtime and emit governance audit events

**Files:**
- Modify: `crates/odin-core-runtime/Cargo.toml`
- Modify: `crates/odin-core-runtime/src/lib.rs`
- Create: `crates/odin-core-runtime/tests/capability_manifest_enforcement.rs`

**Step 1: Write failing runtime enforcement tests**

Create tests for least privilege:

```rust
#[test]
fn denies_capability_not_in_manifest() {
    let outcome = run_with_manifest(manifest_allowing("repo.read"), request_for("repo.delete"));
    assert_eq!(outcome.status, ActionStatus::Blocked);
    assert_eq!(outcome.detail, "manifest_capability_not_granted");
}

#[test]
fn denies_stagehand_for_non_browser_agent() {
    let outcome = run_with_manifest(non_browser_manifest(), stagehand_request());
    assert_eq!(outcome.status, ActionStatus::Blocked);
}
```

**Step 2: Run tests to verify RED**

Run: `cargo test -p odin-core-runtime --test capability_manifest_enforcement -- --nocapture`  
Expected: FAIL.

**Step 3: Implement manifest checks + audit events**

Add runtime entrypoint:

```rust
pub fn handle_action_with_manifest(
    &self,
    request: ActionRequest,
    manifest: &CapabilityManifest,
) -> RuntimeResult<ActionOutcome> { /* enforce allowlist + plugin permissions */ }
```

Emit audit events:
- `governance.manifest.validated`
- `governance.manifest.denied`
- `governance.capability.used`

**Step 4: Re-run tests to verify GREEN**

Run: `cargo test -p odin-core-runtime --test capability_manifest_enforcement -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/odin-core-runtime/Cargo.toml crates/odin-core-runtime/src/lib.rs crates/odin-core-runtime/tests/capability_manifest_enforcement.rs
git commit -m "feat(runtime): enforce capability manifests and governance audit events"
```

### Task 6: Add CLI governance operations (discover/install/verify/enable-plugin)

**Files:**
- Modify: `bin/odin-cli/Cargo.toml`
- Modify: `bin/odin-cli/src/main.rs`
- Create: `bin/odin-cli/tests/governance_cli.rs`
- Create: `config/skills.project.example.yaml`

**Step 1: Write failing CLI contract tests**

In `bin/odin-cli/tests/governance_cli.rs`:

```rust
#[test]
fn governance_discover_prints_candidates() {
    Command::cargo_bin("odin-cli").unwrap()
        .args(["governance", "discover", "--scope", "project"])
        .assert()
        .success()
        .stdout(predicates::str::contains("candidates"));
}
```

Add tests for:
- install requires `--ack` for untrusted
- enable-plugin stagehand requires explicit domains/workspaces
- verify prints pass/fail checks

**Step 2: Run tests to verify RED**

Run: `cargo test -p odin-cli --test governance_cli -- --nocapture`  
Expected: FAIL.

**Step 3: Implement minimal CLI paths**

Add subcommand parser and handlers:
- `odin-cli governance discover`
- `odin-cli governance install`
- `odin-cli governance enable-plugin`
- `odin-cli governance verify`

Wire handlers to `odin-governance` crate and print machine-readable JSON summary.

**Step 4: Re-run tests to verify GREEN**

Run: `cargo test -p odin-cli --test governance_cli -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add bin/odin-cli/Cargo.toml bin/odin-cli/src/main.rs bin/odin-cli/tests/governance_cli.rs config/skills.project.example.yaml
git commit -m "feat(cli): add governance subcommands for skill and plugin operations"
```

### Task 7: Add docs and end-to-end governance smoke verification

**Files:**
- Create: `docs/skill-system.md`
- Create: `docs/stagehand-safety.md`
- Modify: `docs/plugin-system.md`
- Modify: `README.md`
- Create: `scripts/verify/skill-plugin-governance-smoke.sh`
- Modify: `docs/release-readiness.md`

**Step 1: Write failing smoke script first**

Create `scripts/verify/skill-plugin-governance-smoke.sh` with checks:
- untrusted skill install without ack -> blocked
- stagehand enable without domains/workspaces -> blocked
- capability not in manifest -> blocked
- allowed manifest capability -> executed

**Step 2: Run smoke script to verify RED**

Run: `bash scripts/verify/skill-plugin-governance-smoke.sh`  
Expected: FAIL before docs/flows are complete.

**Step 3: Document operator workflows and evidence expectations**

Write docs for:
- scoped skill registries and trust model
- import risk scan and ack behavior
- Stagehand permission envelope and disallowed defaults
- capability manifest requirements for delegation
- audit evidence required before “working” claims

**Step 4: Run full verification matrix (GREEN required)**

Run:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `bash scripts/verify/plugin-install-matrix.sh`
- `bash scripts/verify/skill-plugin-governance-smoke.sh`

Expected: all pass.

**Step 5: Commit**

```bash
git add docs/skill-system.md docs/stagehand-safety.md docs/plugin-system.md README.md scripts/verify/skill-plugin-governance-smoke.sh docs/release-readiness.md
git commit -m "docs(verify): add governance playbook and smoke verification matrix"
```

