use std::collections::HashSet;
use std::fs;
use std::path::Path;

use odin_plugin_protocol::{
    DelegationCapability, SkillRecord, SkillRegistry, SkillScope, TrustLevel,
};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillRegistryLoadError {
    #[error("registry read failed: {0}")]
    Io(String),
    #[error("registry parse failed: {0}")]
    Parse(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSkillRegistry {
    schema_version: u32,
    scope: String,
    #[serde(default)]
    skills: Vec<RawSkillRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSkillRecord {
    name: String,
    trust_level: String,
    source: String,
    #[serde(default)]
    pinned_version: Option<String>,
    #[serde(default)]
    capabilities: Vec<RawDelegationCapability>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDelegationCapability {
    id: String,
    #[serde(default)]
    scope: Vec<String>,
}

pub fn resolve_skill(
    name: &str,
    user: Option<&SkillRegistry>,
    project: Option<&SkillRegistry>,
    global: Option<&SkillRegistry>,
) -> Result<Option<SkillRecord>, SkillRegistryLoadError> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }

    if let Some(record) = find(name, user, SkillScope::User)? {
        return Ok(Some(record));
    }
    if let Some(record) = find(name, project, SkillScope::Project)? {
        return Ok(Some(record));
    }
    if let Some(record) = find(name, global, SkillScope::Global)? {
        return Ok(Some(record));
    }
    Ok(None)
}

pub fn load_user_registry(path: &Path) -> Result<SkillRegistry, SkillRegistryLoadError> {
    load_scoped_registry(path, SkillScope::User)
}

pub fn load_project_registry(path: &Path) -> Result<SkillRegistry, SkillRegistryLoadError> {
    load_scoped_registry(path, SkillScope::Project)
}

pub fn load_global_registry(path: &Path) -> Result<SkillRegistry, SkillRegistryLoadError> {
    load_scoped_registry(path, SkillScope::Global)
}

pub fn load_scoped_registry(
    path: &Path,
    scope: SkillScope,
) -> Result<SkillRegistry, SkillRegistryLoadError> {
    let raw = fs::read_to_string(path).map_err(|e| SkillRegistryLoadError::Io(e.to_string()))?;
    parse_scoped_registry(&raw, scope)
}

pub fn parse_scoped_registry(
    raw: &str,
    scope: SkillScope,
) -> Result<SkillRegistry, SkillRegistryLoadError> {
    let raw_registry: RawSkillRegistry =
        serde_yaml::from_str(raw).map_err(|e| SkillRegistryLoadError::Parse(e.to_string()))?;
    let schema_version = raw_registry.schema_version;
    if schema_version != 1 {
        return Err(SkillRegistryLoadError::Parse(format!(
            "unsupported schema_version: {schema_version}"
        )));
    }

    let configured_scope = parse_scope(&raw_registry.scope)?;
    if configured_scope != scope {
        return Err(SkillRegistryLoadError::Parse(format!(
            "scope mismatch: expected {}, found {}",
            scope_prefix(scope.clone()),
            scope_prefix(configured_scope),
        )));
    }

    let skills = raw_registry
        .skills
        .into_iter()
        .map(|record| normalize_record(record, scope.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    ensure_unique_skill_names(&skills)?;

    Ok(SkillRegistry {
        schema_version,
        scope,
        skills,
    })
}

fn find(
    name: &str,
    registry: Option<&SkillRegistry>,
    expected_scope: SkillScope,
) -> Result<Option<SkillRecord>, SkillRegistryLoadError> {
    let Some(registry) = registry else {
        return Ok(None);
    };

    if registry.scope != expected_scope {
        return Err(SkillRegistryLoadError::Parse(format!(
            "scope mismatch: expected {}, found {}",
            scope_prefix(expected_scope),
            scope_prefix(registry.scope.clone()),
        )));
    }

    Ok(registry
        .skills
        .iter()
        .find(|record| record.name == name)
        .cloned())
}

fn normalize_record(
    record: RawSkillRecord,
    _scope: SkillScope,
) -> Result<SkillRecord, SkillRegistryLoadError> {
    let normalized_name = record.name.trim();
    if normalized_name.is_empty() {
        return Err(SkillRegistryLoadError::Parse(
            "invalid name: empty".to_string(),
        ));
    }
    if record.source.trim().is_empty() {
        return Err(SkillRegistryLoadError::Parse(
            "invalid source: empty".to_string(),
        ));
    }

    let mut normalized = SkillRecord::default_for(normalized_name.to_string());
    normalized.trust_level = parse_trust_level(&record.trust_level)?;
    normalized.source = normalize_source(&record.source);
    normalized.pinned_version = record.pinned_version;
    normalized.capabilities = record
        .capabilities
        .into_iter()
        .map(normalize_capability)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(normalized)
}

fn normalize_capability(
    capability: RawDelegationCapability,
) -> Result<DelegationCapability, SkillRegistryLoadError> {
    let id = capability.id.trim();
    if id.is_empty() {
        return Err(SkillRegistryLoadError::Parse(
            "invalid capability id: empty".to_string(),
        ));
    }

    Ok(DelegationCapability {
        id: id.to_string(),
        scope: capability.scope,
    })
}

fn ensure_unique_skill_names(skills: &[SkillRecord]) -> Result<(), SkillRegistryLoadError> {
    let mut seen = HashSet::new();
    for skill in skills {
        if !seen.insert(skill.name.clone()) {
            return Err(SkillRegistryLoadError::Parse(format!(
                "duplicate skill name: {}",
                skill.name
            )));
        }
    }
    Ok(())
}

fn normalize_source(source: &str) -> String {
    let trimmed = source.trim();
    if let Some((prefix, rest)) = trimmed.split_once(':') {
        if is_scope_prefix(prefix) {
            return format!("{}:{}", prefix.trim().to_ascii_lowercase(), rest);
        }
        return trimmed.to_string();
    }

    trimmed.to_string()
}

fn parse_trust_level(value: &str) -> Result<TrustLevel, SkillRegistryLoadError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "trusted" => Ok(TrustLevel::Trusted),
        "caution" => Ok(TrustLevel::Caution),
        "untrusted" => Ok(TrustLevel::Untrusted),
        other => Err(SkillRegistryLoadError::Parse(format!(
            "invalid trust_level: {other}"
        ))),
    }
}

fn parse_scope(value: &str) -> Result<SkillScope, SkillRegistryLoadError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "global" => Ok(SkillScope::Global),
        "project" => Ok(SkillScope::Project),
        "user" => Ok(SkillScope::User),
        other => Err(SkillRegistryLoadError::Parse(format!(
            "invalid scope: {other}"
        ))),
    }
}

fn is_scope_prefix(prefix: &str) -> bool {
    matches!(
        prefix.trim().to_ascii_lowercase().as_str(),
        "global" | "project" | "user"
    )
}

fn scope_prefix(scope: SkillScope) -> &'static str {
    match scope {
        SkillScope::Global => "global",
        SkillScope::Project => "project",
        SkillScope::User => "user",
    }
}
