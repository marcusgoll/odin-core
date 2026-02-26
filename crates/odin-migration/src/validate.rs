use crate::model::{LearningPackMetadata, SkillPackMetadata, UserDataManifest};
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationError {
    UnsupportedSchemaVersion {
        context: &'static str,
        expected: u32,
        actual: u32,
    },
    UnsupportedUserDataModelVersion {
        expected: u32,
        actual: u32,
    },
    MissingTopLevelObject(&'static str),
    MissingField(&'static str),
}

impl Display for ValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::UnsupportedSchemaVersion {
                context,
                expected,
                actual,
            } => write!(
                f,
                "unsupported schema_version for {context}: expected {expected}, got {actual}"
            ),
            ValidationError::UnsupportedUserDataModelVersion { expected, actual } => write!(
                f,
                "unsupported user_data_model_version: expected {expected}, got {actual}"
            ),
            ValidationError::MissingTopLevelObject(name) => {
                write!(f, "missing required top-level object: {name}")
            }
            ValidationError::MissingField(name) => write!(f, "missing required field: {name}"),
        }
    }
}

impl Error for ValidationError {}

pub fn validate_manifest(manifest: &UserDataManifest) -> Result<(), ValidationError> {
    ensure_schema_version("manifest", manifest.schema_version)?;

    if manifest.user_data_model_version != 1 {
        return Err(ValidationError::UnsupportedUserDataModelVersion {
            expected: 1,
            actual: manifest.user_data_model_version,
        });
    }

    if manifest.skills.is_none() {
        return Err(ValidationError::MissingTopLevelObject("skills"));
    }
    if manifest.learnings.is_none() {
        return Err(ValidationError::MissingTopLevelObject("learnings"));
    }
    if manifest.runtime.is_none() {
        return Err(ValidationError::MissingTopLevelObject("runtime"));
    }
    if manifest.checkpoints.is_none() {
        return Err(ValidationError::MissingTopLevelObject("checkpoints"));
    }
    if manifest.events.is_none() {
        return Err(ValidationError::MissingTopLevelObject("events"));
    }

    Ok(())
}

pub fn validate_skill_pack_metadata(metadata: &SkillPackMetadata) -> Result<(), ValidationError> {
    ensure_schema_version("skill_pack", metadata.schema_version)?;
    ensure_non_empty("pack_id", &metadata.pack_id)
}

pub fn validate_learning_pack_metadata(
    metadata: &LearningPackMetadata,
) -> Result<(), ValidationError> {
    ensure_schema_version("learning_pack", metadata.schema_version)?;
    ensure_non_empty("pack_id", &metadata.pack_id)
}

fn ensure_schema_version(context: &'static str, actual: u32) -> Result<(), ValidationError> {
    if actual != 1 {
        return Err(ValidationError::UnsupportedSchemaVersion {
            context,
            expected: 1,
            actual,
        });
    }
    Ok(())
}

fn ensure_non_empty(field: &'static str, value: &str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        return Err(ValidationError::MissingField(field));
    }
    Ok(())
}
