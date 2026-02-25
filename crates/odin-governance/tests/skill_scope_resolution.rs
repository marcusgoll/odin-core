use odin_plugin_protocol::{SkillRecord, SkillRegistry, SkillScope, TrustLevel};

use odin_governance::skills::{parse_scoped_registry, resolve_skill, SkillRegistryLoadError};

fn global_registry() -> SkillRegistry {
    SkillRegistry {
        schema_version: 1,
        scope: SkillScope::Global,
        skills: vec![SkillRecord {
            source: "global:/skills/brainstorming".to_string(),
            trust_level: TrustLevel::Untrusted,
            ..SkillRecord::default_for("brainstorming")
        }],
    }
}

fn project_registry() -> SkillRegistry {
    SkillRegistry {
        schema_version: 1,
        scope: SkillScope::Project,
        skills: vec![SkillRecord {
            source: "project:/skills/brainstorming".to_string(),
            trust_level: TrustLevel::Caution,
            ..SkillRecord::default_for("brainstorming")
        }],
    }
}

fn user_registry() -> SkillRegistry {
    SkillRegistry {
        schema_version: 1,
        scope: SkillScope::User,
        skills: vec![SkillRecord {
            source: "user:/skills/brainstorming".to_string(),
            trust_level: TrustLevel::Trusted,
            ..SkillRecord::default_for("brainstorming")
        }],
    }
}

#[test]
fn user_overrides_project_and_global() {
    let resolved = resolve_skill(
        "brainstorming",
        Some(&user_registry()),
        Some(&project_registry()),
        Some(&global_registry()),
    )
    .expect("resolved");

    assert_eq!(resolved.trust_level, TrustLevel::Trusted);
    assert_eq!(resolved.source, "user:/skills/brainstorming");
}

#[test]
fn loader_rejects_scope_override_attempt() {
    let yaml = r#"
schema_version: 1
scope: project
skills:
  - name: brainstorming
    trust_level: trusted
    source: /skills/brainstorming
"#;

    let err = parse_scoped_registry(yaml, SkillScope::User).expect_err("must reject mismatch");
    assert_parse_error_contains(err, "scope");
}

#[test]
fn loader_rejects_invalid_trust_level() {
    let yaml = r#"
schema_version: 1
scope: user
skills:
  - name: brainstorming
    trust_level: definitely_trusted
    source: /skills/brainstorming
"#;

    let err = parse_scoped_registry(yaml, SkillScope::User).expect_err("must reject trust");
    assert_parse_error_contains(err, "trust_level");
}

#[test]
fn loader_rejects_invalid_scope_value() {
    let yaml = r#"
schema_version: 1
scope: enterprise
skills:
  - name: brainstorming
    trust_level: trusted
    source: /skills/brainstorming
"#;

    let err = parse_scoped_registry(yaml, SkillScope::User).expect_err("must reject scope");
    assert_parse_error_contains(err, "scope");
}

#[test]
fn loader_rejects_unsupported_schema_version() {
    let yaml = r#"
schema_version: 2
scope: user
skills:
  - name: brainstorming
    trust_level: trusted
    source: /skills/brainstorming
"#;

    let err = parse_scoped_registry(yaml, SkillScope::User).expect_err("must reject schema");
    assert_parse_error_contains(err, "schema_version");
}

fn assert_parse_error_contains(err: SkillRegistryLoadError, expected: &str) {
    match err {
        SkillRegistryLoadError::Parse(message) => {
            assert!(
                message.contains(expected),
                "expected parse error containing {expected:?}, got {message:?}"
            );
        }
        other => panic!("expected parse error, got {other:?}"),
    }
}
