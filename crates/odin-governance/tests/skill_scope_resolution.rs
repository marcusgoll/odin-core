use odin_plugin_protocol::{SkillRecord, SkillRegistry, SkillScope, TrustLevel};

use odin_governance::skills::resolve_skill;

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
