use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

use odin_plugin_protocol::{DelegationCapability, PluginPermissionEnvelope, TrustLevel};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagehandMode {
    ReadObserve,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DomainRule {
    host: String,
    allow_subdomains: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    ObserveUrl(String),
    ReadWorkspace(String),
    RunCommand(String),
    Login,
    Payment,
    PiiSubmit,
    FileUpload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow { reason_code: String },
    Deny { reason_code: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagehandPolicy {
    enabled: bool,
    mode: StagehandMode,
    allowed_domains: BTreeSet<DomainRule>,
    allowed_workspaces: BTreeSet<String>,
    allowed_commands: BTreeSet<String>,
}

impl Default for StagehandPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: StagehandMode::ReadObserve,
            allowed_domains: BTreeSet::new(),
            allowed_workspaces: BTreeSet::new(),
            allowed_commands: BTreeSet::new(),
        }
    }
}

impl StagehandPolicy {
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_domains<I, S>(mut self, domains: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.allowed_domains.extend(
            domains
                .into_iter()
                .filter_map(|domain| normalize_domain(domain.as_ref())),
        );
        self
    }

    pub fn with_workspaces<I, S>(mut self, workspaces: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.allowed_workspaces.extend(
            workspaces
                .into_iter()
                .filter_map(|workspace| normalize_workspace(workspace.as_ref())),
        );
        self
    }

    pub fn with_commands<I, S>(mut self, commands: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.allowed_commands.extend(
            commands
                .into_iter()
                .filter_map(|command| normalize_command_scope_entry(command.as_ref())),
        );
        self
    }

    pub fn evaluate(&self, action: Action) -> PermissionDecision {
        match action {
            Action::Login => deny("action_login_disallowed"),
            Action::Payment => deny("action_payment_disallowed"),
            Action::PiiSubmit => deny("action_pii_submit_disallowed"),
            Action::FileUpload => deny("action_file_upload_disallowed"),
            _ if !self.enabled => deny("plugin_disabled"),
            Action::ObserveUrl(url) => self.evaluate_observe_url(&url),
            Action::ReadWorkspace(workspace) => self.evaluate_workspace(&workspace),
            Action::RunCommand(command) => self.evaluate_command(&command),
        }
    }

    fn evaluate_observe_url(&self, url: &str) -> PermissionDecision {
        let Some(host) = extract_host(url) else {
            return deny("invalid_url");
        };

        if self.allowed_domains.is_empty() {
            return deny("domain_not_allowlisted");
        }

        if self
            .allowed_domains
            .iter()
            .any(|allowed| domain_matches(&host, allowed))
        {
            allow("domain_allowlisted")
        } else {
            deny("domain_not_allowlisted")
        }
    }

    fn evaluate_workspace(&self, workspace: &str) -> PermissionDecision {
        if !matches!(self.mode, StagehandMode::ReadObserve) {
            return deny("mode_not_supported");
        }

        let Some(workspace) = normalize_workspace(workspace) else {
            return deny("workspace_not_allowlisted");
        };

        if self.is_workspace_allowlisted(&workspace) {
            allow("workspace_allowlisted")
        } else {
            deny("workspace_not_allowlisted")
        }
    }

    fn evaluate_command(&self, command: &str) -> PermissionDecision {
        if !matches!(self.mode, StagehandMode::ReadObserve) {
            return deny("mode_not_supported");
        }

        if has_unsafe_shell_syntax(command) {
            return deny("command_unsafe_shell_syntax");
        }

        let Some((command_name, args)) = parse_command(command) else {
            return deny("command_not_allowlisted");
        };

        if !self.allowed_commands.contains(&command_name) {
            return deny("command_not_allowlisted");
        }

        if self.allowed_workspaces.is_empty() {
            return deny("command_workspace_policy_missing");
        }

        if has_relative_parent_traversal(&args) {
            return deny("command_relative_path_traversal");
        }

        if !self.allowed_workspaces.is_empty() && has_unscoped_relative_path(&args) {
            return deny("command_relative_path_unscoped");
        }

        if first_absolute_path_outside_workspaces(&args, &self.allowed_workspaces).is_some() {
            return deny("command_path_outside_allowlisted_workspace");
        }

        allow("command_allowlisted")
    }

    fn is_workspace_allowlisted(&self, workspace: &str) -> bool {
        let Some(candidate) = normalize_boundary_path(Path::new(workspace)) else {
            return false;
        };

        self.allowed_workspaces
            .iter()
            .filter_map(|allowed| normalize_boundary_path(Path::new(allowed)))
            .any(|allowed| candidate == allowed || candidate.starts_with(&allowed))
    }
}

#[derive(Clone, Debug, Default)]
pub struct PluginPermissionRegistry {
    envelopes: BTreeMap<String, PluginPermissionEnvelope>,
}

impl PluginPermissionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_envelopes<I>(envelopes: I) -> Self
    where
        I: IntoIterator<Item = PluginPermissionEnvelope>,
    {
        let mut registry = Self::new();
        for envelope in envelopes {
            registry.insert(envelope);
        }
        registry
    }

    pub fn insert(&mut self, envelope: PluginPermissionEnvelope) {
        self.envelopes.insert(envelope.plugin.clone(), envelope);
    }

    pub fn get(&self, plugin: &str) -> Option<&PluginPermissionEnvelope> {
        self.envelopes.get(plugin)
    }

    pub fn stagehand_policy(&self) -> StagehandPolicy {
        self.get("stagehand")
            .map(stagehand_policy_from_envelope)
            .unwrap_or_else(stagehand_default_policy)
    }
}

pub fn stagehand_default_policy() -> StagehandPolicy {
    StagehandPolicy::default()
}

pub fn stagehand_with_domains<I, S>(domains: I) -> StagehandPolicy
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    stagehand_default_policy()
        .with_enabled(true)
        .with_domains(domains)
}

pub fn stagehand_policy_from_envelope(envelope: &PluginPermissionEnvelope) -> StagehandPolicy {
    if envelope.plugin != "stagehand" {
        return stagehand_default_policy();
    }

    let can_enable = envelope.trust_level != TrustLevel::Untrusted;
    let mut policy = stagehand_default_policy();

    for permission in &envelope.permissions {
        apply_permission_scope(&mut policy, permission, can_enable);
    }

    policy
}

fn apply_permission_scope(
    policy: &mut StagehandPolicy,
    permission: &DelegationCapability,
    can_enable: bool,
) {
    match permission.id.as_str() {
        "browser.observe" | "stagehand.observe_url" | "stagehand.observe_domain" => {
            policy.allowed_domains.extend(
                permission
                    .scope
                    .iter()
                    .filter_map(|domain| normalize_domain(domain)),
            );
        }
        "workspace.read" | "stagehand.workspace.read" => {
            policy.allowed_workspaces.extend(
                permission
                    .scope
                    .iter()
                    .filter_map(|workspace| normalize_workspace(workspace)),
            );
        }
        "command.run" | "stagehand.command.run" => {
            policy.allowed_commands.extend(
                permission
                    .scope
                    .iter()
                    .filter_map(|command| normalize_command_scope_entry(command)),
            );
        }
        "stagehand.enabled" => {
            if can_enable {
                policy.enabled = true;
            }
        }
        _ => {}
    }
}

fn allow(reason_code: &str) -> PermissionDecision {
    PermissionDecision::Allow {
        reason_code: reason_code.to_string(),
    }
}

fn deny(reason_code: &str) -> PermissionDecision {
    PermissionDecision::Deny {
        reason_code: reason_code.to_string(),
    }
}

fn extract_host(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))?;
    let authority = without_scheme
        .split(['/', '?', '#'])
        .next()?
        .trim();
    let host_port = authority.rsplit('@').next()?;
    let host = host_port.split(':').next()?.trim().to_ascii_lowercase();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn normalize_domain(domain: &str) -> Option<DomainRule> {
    let trimmed = domain.trim();
    let lowered = trimmed.to_ascii_lowercase();
    let no_scheme = lowered
        .strip_prefix("https://")
        .or_else(|| lowered.strip_prefix("http://"))
        .unwrap_or(&lowered);
    let (allow_subdomains, domain_part) = if let Some(stripped) = no_scheme.strip_prefix("*.") {
        (true, stripped)
    } else {
        (false, no_scheme)
    };

    let host = domain_part
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(domain_part)
        .split(':')
        .next()
        .unwrap_or(domain_part)
        .trim()
        .to_string();

    if host.is_empty() {
        None
    } else {
        Some(DomainRule {
            host,
            allow_subdomains,
        })
    }
}

fn normalize_workspace(workspace: &str) -> Option<String> {
    let trimmed = workspace.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    let path = Path::new(trimmed);
    if path.is_absolute() {
        return normalize_boundary_path(path).map(pathbuf_to_string);
    }

    Some(trimmed.to_string())
}

fn normalize_command_scope_entry(command: &str) -> Option<String> {
    let normalized = command.trim();
    if normalized.is_empty() {
        return None;
    }

    if normalized.chars().any(char::is_whitespace) {
        return None;
    }

    Some(normalized.to_string())
}

fn normalize_command_name(command: &str) -> Option<String> {
    let normalized = command.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn parse_command(command: &str) -> Option<(String, Vec<String>)> {
    let mut tokens = command.split_whitespace();
    let command_name = normalize_command_name(tokens.next()?)?;
    let args = tokens.map(strip_wrapping_quotes).collect::<Vec<_>>();
    Some((command_name, args))
}

fn first_absolute_path_outside_workspaces(
    args: &[String],
    allowed_workspaces: &BTreeSet<String>,
) -> Option<String> {
    for path in command_scoped_path_values(args)
        .into_iter()
        .filter(|value| Path::new(value).is_absolute())
    {
        let Some(path_obj) = canonicalize_existing_absolute_path(Path::new(&path)) else {
            return Some(path);
        };

        let in_allowed_workspace = allowed_workspaces
            .iter()
            .filter_map(|workspace| normalize_boundary_path(Path::new(workspace)))
            .any(|workspace| path_obj == workspace || path_obj.starts_with(&workspace));
        if !in_allowed_workspace {
            return Some(path);
        }
    }
    None
}

fn strip_wrapping_quotes(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 {
        let starts_single = trimmed.starts_with('\'') && trimmed.ends_with('\'');
        let starts_double = trimmed.starts_with('"') && trimmed.ends_with('"');
        if starts_single || starts_double {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }

    trimmed.to_string()
}

fn domain_matches(host: &str, allowed: &DomainRule) -> bool {
    if allowed.allow_subdomains {
        return host.ends_with(&format!(".{}", allowed.host));
    }
    host == allowed.host
}

fn has_unsafe_shell_syntax(command: &str) -> bool {
    command.chars().any(|ch| {
        matches!(
            ch,
            ';' | '|' | '&' | '>' | '<' | '`' | '$' | '(' | ')' | '\n' | '\r' | '\'' | '"'
        )
    })
}

fn has_relative_parent_traversal(args: &[String]) -> bool {
    command_scoped_path_values(args)
        .into_iter()
        .any(|value| has_relative_parent_segment(&value))
}

fn has_relative_parent_segment(token: &str) -> bool {
    let path = Path::new(token);
    !path.is_absolute()
        && path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
}

fn has_unscoped_relative_path(args: &[String]) -> bool {
    command_scoped_path_values(args)
        .into_iter()
        .any(|value| !Path::new(&value).is_absolute())
}

fn normalize_boundary_path(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        if let Ok(canonical) = fs::canonicalize(path) {
            return Some(canonical);
        }
    }

    normalize_lexical_path(path)
}

fn canonicalize_existing_absolute_path(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }

    fs::canonicalize(path).ok()
}

fn command_scoped_path_values(args: &[String]) -> Vec<String> {
    let mut values = Vec::new();

    for arg in args {
        let token = strip_wrapping_quotes(arg);
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(value) = extract_option_value(trimmed) {
            values.push(value);
            continue;
        }

        if trimmed.starts_with('-') {
            continue;
        }

        values.push(trimmed.to_string());
    }

    values
}

fn extract_option_value(token: &str) -> Option<String> {
    if token.starts_with("--") {
        let (_, value) = token.split_once('=')?;
        return normalize_option_value(value);
    }

    if token.starts_with('-') {
        if token.len() <= 2 {
            return None;
        }

        let attached = &token[2..];
        let attached = attached.strip_prefix('=').unwrap_or(attached);
        return normalize_option_value(attached);
    }

    None
}

fn normalize_option_value(value: &str) -> Option<String> {
    let trimmed = strip_wrapping_quotes(value);
    let trimmed = trimmed.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_lexical_path(path: &Path) -> Option<PathBuf> {
    let mut prefix: Option<OsString> = None;
    let mut has_root = false;
    let mut parts: Vec<OsString> = Vec::new();

    for component in path.components() {
        match component {
            Component::Prefix(value) => {
                prefix = Some(value.as_os_str().to_os_string());
            }
            Component::RootDir => {
                has_root = true;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if let Some(last) = parts.last() {
                    if last != ".." {
                        parts.pop();
                    } else if has_root {
                        return None;
                    } else {
                        parts.push(OsString::from(".."));
                    }
                } else if has_root {
                    return None;
                } else {
                    parts.push(OsString::from(".."));
                }
            }
            Component::Normal(value) => parts.push(value.to_os_string()),
        }
    }

    let mut normalized = PathBuf::new();
    if let Some(value) = prefix {
        normalized.push(value);
    }
    if has_root {
        normalized.push(std::path::MAIN_SEPARATOR.to_string());
    }
    for part in parts {
        normalized.push(part);
    }

    if normalized.as_os_str().is_empty() {
        if has_root {
            normalized.push(std::path::MAIN_SEPARATOR.to_string());
        } else {
            normalized.push(".");
        }
    }

    Some(normalized)
}

fn pathbuf_to_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn default_policy_denies_when_disabled() {
        let policy = stagehand_default_policy();
        let decision = policy.evaluate(Action::ObserveUrl("https://example.com".to_string()));
        assert_eq!(
            decision,
            PermissionDecision::Deny {
                reason_code: "plugin_disabled".to_string()
            }
        );
    }

    #[test]
    fn policy_disallows_sensitive_actions_even_when_enabled() {
        let policy = stagehand_with_domains(["example.com"]);
        let decision = policy.evaluate(Action::Payment);
        assert_eq!(
            decision,
            PermissionDecision::Deny {
                reason_code: "action_payment_disallowed".to_string()
            }
        );
    }

    #[test]
    fn policy_allows_allowlisted_domain() {
        let policy = stagehand_with_domains(["example.com"]);
        let decision = policy.evaluate(Action::ObserveUrl("https://example.com/path".to_string()));
        assert_eq!(
            decision,
            PermissionDecision::Allow {
                reason_code: "domain_allowlisted".to_string()
            }
        );
    }

    #[test]
    fn policy_wildcard_allows_subdomain() {
        let policy = stagehand_with_domains(["*.example.com"]);
        let decision = policy.evaluate(Action::ObserveUrl("https://www.example.com/path".to_string()));
        assert_eq!(
            decision,
            PermissionDecision::Allow {
                reason_code: "domain_allowlisted".to_string()
            }
        );
    }

    #[test]
    fn registry_uses_stagehand_envelope() {
        let mut registry = PluginPermissionRegistry::new();
        registry.insert(PluginPermissionEnvelope {
            plugin: "stagehand".to_string(),
            trust_level: TrustLevel::Trusted,
            permissions: vec![
                DelegationCapability {
                    id: "stagehand.enabled".to_string(),
                    scope: vec![],
                },
                DelegationCapability {
                    id: "browser.observe".to_string(),
                    scope: vec!["example.com".to_string()],
                },
            ],
        });

        let decision = registry
            .stagehand_policy()
            .evaluate(Action::ObserveUrl("https://example.com".to_string()));
        assert_eq!(
            decision,
            PermissionDecision::Allow {
                reason_code: "domain_allowlisted".to_string()
            }
        );
    }

    #[test]
    fn trusted_envelope_without_enable_capability_stays_disabled() {
        let policy = stagehand_policy_from_envelope(&PluginPermissionEnvelope {
            plugin: "stagehand".to_string(),
            trust_level: TrustLevel::Trusted,
            permissions: vec![DelegationCapability {
                id: "browser.observe".to_string(),
                scope: vec!["example.com".to_string()],
            }],
        });

        let decision = policy.evaluate(Action::ObserveUrl("https://example.com".to_string()));
        assert_eq!(
            decision,
            PermissionDecision::Deny {
                reason_code: "plugin_disabled".to_string()
            }
        );
    }

    #[test]
    fn boundary_path_uses_canonical_path_when_target_exists() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("odin-governance-path-existing-{unique}"));
        let leaf = root.join("allowed");

        fs::create_dir_all(&leaf).expect("create temp tree");

        let input = root.join("allowed").join("..").join("allowed");
        let expected = fs::canonicalize(&leaf).expect("canonical leaf");
        let actual = normalize_boundary_path(&input).expect("normalize");

        assert_eq!(actual, expected);

        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn boundary_path_falls_back_to_lexical_normalization_when_missing() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("odin-governance-path-missing-{unique}"));
        let input = root.join("allowed").join("..").join("outside");

        let actual = normalize_boundary_path(&input).expect("normalize");
        let expected = root.join("outside");

        assert_eq!(actual, expected);
    }
}
