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
) -> Option<SkillRecord> {
    find(name, user, SkillScope::User)
        .or_else(|| find(name, project, SkillScope::Project))
        .or_else(|| find(name, global, SkillScope::Global))
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

    let normalized_scope = raw_registry
        .scope
        .as_deref()
        .and_then(parse_scope)
        .unwrap_or(scope);

    let skills = raw_registry
        .skills
        .into_iter()
        .map(|record| normalize_record(record, normalized_scope.clone()))
        .collect();

    Ok(SkillRegistry {
        schema_version: raw_registry.schema_version.unwrap_or(1),
        scope: normalized_scope,
        skills,
    })
}

fn find(name: &str, registry: Option<&SkillRegistry>, scope: SkillScope) -> Option<SkillRecord> {
    registry.and_then(|registry| {
        registry
            .skills
            .iter()
            .find(|record| record.name == name)
            .cloned()
            .map(|record| normalize_resolved_record(record, scope))
    })
}

fn normalize_record(record: RawSkillRecord, scope: SkillScope) -> SkillRecord {
    let mut normalized = SkillRecord::default_for(record.name.clone());
    normalized.trust_level = record
        .trust_level
        .as_deref()
        .map(parse_trust_level)
        .unwrap_or(TrustLevel::Untrusted);
    normalized.source = normalize_source(
        record
            .source
            .as_deref()
            .unwrap_or(&format!("/skills/{}", record.name)),
        scope,
    );
    normalized.pinned_version = record.pinned_version;
    normalized.capabilities = record.capabilities;
    normalized
}

fn normalize_resolved_record(mut record: SkillRecord, scope: SkillScope) -> SkillRecord {
    record.source = normalize_source(&record.source, scope);
    record
}

fn normalize_source(source: &str, scope: SkillScope) -> String {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return format!("{}:unknown", scope_prefix(scope));
    }

    let prefix = format!("{}:", scope_prefix(scope));
    if trimmed.starts_with(&prefix) || trimmed.contains(':') {
        return trimmed.to_string();
    }

    format!("{prefix}{trimmed}")
}

fn parse_trust_level(value: &str) -> TrustLevel {
    match value.trim().to_ascii_lowercase().as_str() {
        "trusted" => TrustLevel::Trusted,
        "caution" => TrustLevel::Caution,
        _ => TrustLevel::Untrusted,
    }
}

fn parse_scope(value: &str) -> Option<SkillScope> {
    match value.trim().to_ascii_lowercase().as_str() {
        "global" => Some(SkillScope::Global),
        "project" => Some(SkillScope::Project),
        "user" => Some(SkillScope::User),
        _ => None,
    }
}

fn scope_prefix(scope: SkillScope) -> &'static str {
    match scope {
        SkillScope::Global => "global",
        SkillScope::Project => "project",
        SkillScope::User => "user",
    }
}
