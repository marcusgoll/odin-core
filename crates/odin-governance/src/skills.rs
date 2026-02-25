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
struct RawSkillRegistry {
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    skills: Vec<RawSkillRecord>,
}

#[derive(Debug, Deserialize)]
struct RawSkillRecord {
    name: String,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    trust_level: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    pinned_version: Option<String>,
    #[serde(default)]
    capabilities: Vec<DelegationCapability>,
}

pub fn resolve_skill(
    name: &str,
    user: Option<&SkillRegistry>,
    project: Option<&SkillRegistry>,
    global: Option<&SkillRegistry>,
) -> Result<Option<SkillRecord>, SkillRegistryLoadError> {
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
    let schema_version = raw_registry.schema_version.unwrap_or(1);
    if schema_version != 1 {
        return Err(SkillRegistryLoadError::Parse(format!(
            "unsupported schema_version: {schema_version}"
        )));
    }

    if let Some(configured_scope) = raw_registry.scope.as_deref() {
        let configured_scope = parse_scope(configured_scope)?;
        if configured_scope != scope {
            return Err(SkillRegistryLoadError::Parse(format!(
                "scope mismatch: expected {}, found {}",
                scope_prefix(scope.clone()),
                scope_prefix(configured_scope),
            )));
        }
    }

    let skills = raw_registry
        .skills
        .into_iter()
        .map(|record| normalize_record(record, scope.clone()))
        .collect::<Result<Vec<_>, _>>()?;

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
    scope: SkillScope,
) -> Result<SkillRecord, SkillRegistryLoadError> {
    if let Some(record_scope) = record.scope.as_deref() {
        let record_scope = parse_scope(record_scope)?;
        if record_scope != scope {
            return Err(SkillRegistryLoadError::Parse(format!(
                "scope mismatch: expected {}, found {}",
                scope_prefix(scope),
                scope_prefix(record_scope),
            )));
        }
    }

    let mut normalized = SkillRecord::default_for(record.name.clone());
    normalized.trust_level = match record.trust_level.as_deref() {
        Some(value) => parse_trust_level(value)?,
        None => TrustLevel::Untrusted,
    };
    normalized.source = normalize_source(
        record
            .source
            .as_deref()
            .unwrap_or(&format!("/skills/{}", record.name)),
    );
    normalized.pinned_version = record.pinned_version;
    normalized.capabilities = record.capabilities;
    Ok(normalized)
}

fn normalize_source(source: &str) -> String {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return "local:unknown".to_string();
    }

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
