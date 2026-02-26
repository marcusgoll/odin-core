use odin_migration::model::{
    LearningPackMetadata, ManifestSectionRef, SkillPackMetadata, UserDataManifest,
};
use odin_migration::validate::{
    validate_learning_pack_metadata, validate_manifest, validate_skill_pack_metadata,
    ValidationError,
};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

fn section() -> ManifestSectionRef {
    ManifestSectionRef {}
}

fn valid_manifest() -> UserDataManifest {
    UserDataManifest {
        schema_version: 1,
        user_data_model_version: 1,
        skills: Some(section()),
        learnings: Some(section()),
        runtime: Some(section()),
        checkpoints: Some(section()),
        events: Some(section()),
        opaque: None,
        quarantine: None,
        meta: None,
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn model_validation_manifest_with_required_top_level_sections_and_versions_is_valid() {
    let manifest = valid_manifest();

    let result = validate_manifest(&manifest);

    assert!(result.is_ok(), "expected valid manifest, got: {result:?}");
}

#[test]
fn model_validation_manifest_missing_required_section_fails_validation() {
    let mut manifest = valid_manifest();
    manifest.skills = None;

    let result = validate_manifest(&manifest);

    assert_eq!(
        result,
        Err(ValidationError::MissingTopLevelObject("skills"))
    );
}

#[test]
fn model_validation_manifest_missing_learnings_fails_validation() {
    let mut manifest = valid_manifest();
    manifest.learnings = None;

    let result = validate_manifest(&manifest);

    assert_eq!(
        result,
        Err(ValidationError::MissingTopLevelObject("learnings"))
    );
}

#[test]
fn model_validation_manifest_missing_runtime_fails_validation() {
    let mut manifest = valid_manifest();
    manifest.runtime = None;

    let result = validate_manifest(&manifest);

    assert_eq!(
        result,
        Err(ValidationError::MissingTopLevelObject("runtime"))
    );
}

#[test]
fn model_validation_manifest_missing_checkpoints_fails_validation() {
    let mut manifest = valid_manifest();
    manifest.checkpoints = None;

    let result = validate_manifest(&manifest);

    assert_eq!(
        result,
        Err(ValidationError::MissingTopLevelObject("checkpoints"))
    );
}

#[test]
fn model_validation_manifest_missing_events_fails_validation() {
    let mut manifest = valid_manifest();
    manifest.events = None;

    let result = validate_manifest(&manifest);

    assert_eq!(
        result,
        Err(ValidationError::MissingTopLevelObject("events"))
    );
}

#[test]
fn model_validation_manifest_wrong_schema_version_fails_validation() {
    let mut manifest = valid_manifest();
    manifest.schema_version = 2;

    let result = validate_manifest(&manifest);

    assert_eq!(
        result,
        Err(ValidationError::UnsupportedSchemaVersion {
            context: "manifest",
            expected: 1,
            actual: 2,
        })
    );
}

#[test]
fn model_validation_manifest_wrong_user_data_model_version_fails_validation() {
    let mut manifest = valid_manifest();
    manifest.user_data_model_version = 9;

    let result = validate_manifest(&manifest);

    assert_eq!(
        result,
        Err(ValidationError::UnsupportedUserDataModelVersion {
            expected: 1,
            actual: 9,
        })
    );
}

#[test]
fn model_validation_skill_pack_metadata_with_required_fields_is_valid() {
    let metadata = SkillPackMetadata {
        schema_version: 1,
        pack_id: "project-cfipros".to_string(),
    };

    let result = validate_skill_pack_metadata(&metadata);

    assert!(result.is_ok(), "expected valid metadata, got: {result:?}");
}

#[test]
fn model_validation_skill_pack_metadata_wrong_schema_version_fails() {
    let metadata = SkillPackMetadata {
        schema_version: 4,
        pack_id: "project-cfipros".to_string(),
    };

    let result = validate_skill_pack_metadata(&metadata);

    assert_eq!(
        result,
        Err(ValidationError::UnsupportedSchemaVersion {
            context: "skill_pack",
            expected: 1,
            actual: 4,
        })
    );
}

#[test]
fn model_validation_skill_pack_metadata_empty_pack_id_fails() {
    let metadata = SkillPackMetadata {
        schema_version: 1,
        pack_id: "   ".to_string(),
    };

    let result = validate_skill_pack_metadata(&metadata);

    assert_eq!(result, Err(ValidationError::MissingField("pack_id")));
}

#[test]
fn model_validation_learning_pack_metadata_with_required_fields_is_valid() {
    let metadata = LearningPackMetadata {
        schema_version: 1,
        pack_id: "memory-hot".to_string(),
    };

    let result = validate_learning_pack_metadata(&metadata);

    assert!(result.is_ok(), "expected valid metadata, got: {result:?}");
}

#[test]
fn model_validation_learning_pack_metadata_wrong_schema_version_fails() {
    let metadata = LearningPackMetadata {
        schema_version: 3,
        pack_id: "memory-hot".to_string(),
    };

    let result = validate_learning_pack_metadata(&metadata);

    assert_eq!(
        result,
        Err(ValidationError::UnsupportedSchemaVersion {
            context: "learning_pack",
            expected: 1,
            actual: 3,
        })
    );
}

#[test]
fn model_validation_learning_pack_metadata_empty_pack_id_fails() {
    let metadata = LearningPackMetadata {
        schema_version: 1,
        pack_id: "".to_string(),
    };

    let result = validate_learning_pack_metadata(&metadata);

    assert_eq!(result, Err(ValidationError::MissingField("pack_id")));
}

#[test]
fn model_validation_skill_pack_schema_pack_id_rejects_whitespace_only_values() {
    let schema = fs::read_to_string(repo_root().join("schemas/skill-pack.v1.schema.json"))
        .expect("read skill pack schema");
    let json: Value = serde_json::from_str(&schema).expect("parse skill pack schema json");
    let pattern = json
        .pointer("/properties/pack_id/pattern")
        .and_then(Value::as_str)
        .expect("skill pack schema should have pack_id pattern");

    assert_eq!(
        pattern, ".*\\S.*",
        "expected schema-level non-whitespace pattern for pack_id"
    );
}

#[test]
fn model_validation_learning_pack_schema_pack_id_rejects_whitespace_only_values() {
    let schema = fs::read_to_string(repo_root().join("schemas/learning-pack.v1.schema.json"))
        .expect("read learning pack schema");
    let json: Value = serde_json::from_str(&schema).expect("parse learning pack schema json");
    let pattern = json
        .pointer("/properties/pack_id/pattern")
        .and_then(Value::as_str)
        .expect("learning pack schema should have pack_id pattern");

    assert_eq!(
        pattern, ".*\\S.*",
        "expected schema-level non-whitespace pattern for pack_id"
    );
}

#[test]
fn model_validation_manifest_deserialization_rejects_unknown_fields() {
    let raw = json!({
        "schema_version": 1,
        "user_data_model_version": 1,
        "skills": {},
        "learnings": {},
        "runtime": {},
        "checkpoints": {},
        "events": {},
        "unexpected": {}
    });

    let decoded = serde_json::from_value::<UserDataManifest>(raw);

    assert!(
        decoded.is_err(),
        "expected unknown top-level field to fail deserialization"
    );
}

#[test]
fn model_validation_skill_pack_deserialization_rejects_unknown_fields() {
    let raw = json!({
        "schema_version": 1,
        "pack_id": "project-cfipros",
        "unexpected": true
    });

    let decoded = serde_json::from_value::<SkillPackMetadata>(raw);

    assert!(
        decoded.is_err(),
        "expected unknown top-level field to fail deserialization"
    );
}

#[test]
fn model_validation_learning_pack_deserialization_rejects_unknown_fields() {
    let raw = json!({
        "schema_version": 1,
        "pack_id": "memory-hot",
        "unexpected": true
    });

    let decoded = serde_json::from_value::<LearningPackMetadata>(raw);

    assert!(
        decoded.is_err(),
        "expected unknown top-level field to fail deserialization"
    );
}
