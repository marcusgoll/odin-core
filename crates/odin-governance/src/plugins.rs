use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

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
                .filter_map(|command| normalize_command(command.as_ref())),
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

        let Some((command_name, args)) = parse_command(command) else {
            return deny("command_not_allowlisted");
        };

        if !self.allowed_commands.contains(&command_name) {
            return deny("command_not_allowlisted");
        }

        if first_absolute_path_outside_workspaces(&args, &self.allowed_workspaces).is_some() {
            return deny("command_path_outside_allowlisted_workspace");
        }

        allow("command_allowlisted")
    }

    fn is_workspace_allowlisted(&self, workspace: &str) -> bool {
        let candidate = Path::new(workspace);
        self.allowed_workspaces.iter().any(|allowed| {
            let allowed = Path::new(allowed);
            candidate == allowed || candidate.starts_with(allowed)
        })
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
    let mut policy = stagehand_default_policy().with_enabled(envelope.trust_level != TrustLevel::Untrusted);

    for permission in &envelope.permissions {
        apply_permission_scope(&mut policy, permission);
    }

    policy
}

fn apply_permission_scope(policy: &mut StagehandPolicy, permission: &DelegationCapability) {
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
                    .filter_map(|command| normalize_command(command)),
            );
        }
        "stagehand.enabled" => {
            policy.enabled = true;
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
    let (allow_subdomains, domain_part) = if let Some(stripped) = trimmed.strip_prefix("*.") {
        (true, stripped)
    } else {
        (false, trimmed)
    };

    let lowered = domain_part.to_ascii_lowercase();
    let normalized = lowered
        .strip_prefix("https://")
        .or_else(|| lowered.strip_prefix("http://"))
        .unwrap_or(&lowered);
    let host = normalized
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(normalized)
        .split(':')
        .next()
        .unwrap_or(normalized)
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

    Some(trimmed.to_string())
}

fn normalize_command(command: &str) -> Option<String> {
    let first = command.split_whitespace().next()?;
    let normalized = first.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn parse_command(command: &str) -> Option<(String, Vec<String>)> {
    let mut tokens = command.split_whitespace();
    let command_name = normalize_command(tokens.next()?)?;
    let args = tokens.map(strip_wrapping_quotes).collect::<Vec<_>>();
    Some((command_name, args))
}

fn first_absolute_path_outside_workspaces(
    args: &[String],
    allowed_workspaces: &BTreeSet<String>,
) -> Option<String> {
    for arg in args {
        if let Some(path) = extract_absolute_path(arg) {
            let path_obj = Path::new(&path);
            let in_allowed_workspace = allowed_workspaces
                .iter()
                .any(|workspace| path_obj.starts_with(Path::new(workspace)));
            if !in_allowed_workspace {
                return Some(path);
            }
        }
    }
    None
}

fn extract_absolute_path(token: &str) -> Option<String> {
    let stripped = strip_wrapping_quotes(token);
    if Path::new(&stripped).is_absolute() {
        return Some(stripped);
    }

    let (_, value) = stripped.split_once('=')?;
    let value = strip_wrapping_quotes(value);
    if Path::new(&value).is_absolute() {
        Some(value)
    } else {
        None
    }
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
    if host == allowed.host {
        return true;
    }
    allowed.allow_subdomains && host.ends_with(&format!(".{}", allowed.host))
}

#[cfg(test)]
mod tests {
    use super::*;

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
            permissions: vec![DelegationCapability {
                id: "browser.observe".to_string(),
                scope: vec!["example.com".to_string()],
            }],
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
}
