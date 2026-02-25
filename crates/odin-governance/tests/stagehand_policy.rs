use odin_governance::plugins::{
    Action, PermissionDecision, stagehand_default_policy, stagehand_policy_from_envelope,
    stagehand_with_domains,
};
use odin_plugin_protocol::{DelegationCapability, PluginPermissionEnvelope, TrustLevel};

#[test]
fn stagehand_denies_login_by_default() {
    let policy = stagehand_default_policy();
    let decision = policy.evaluate(Action::Login);

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "action_login_disallowed".to_string()
        }
    );
}

#[test]
fn stagehand_denies_domain_outside_allowlist() {
    let policy = stagehand_with_domains(["example.com"]);
    let decision = policy.evaluate(Action::ObserveUrl("https://not-allowed.dev".to_string()));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "domain_not_allowlisted".to_string()
        }
    );
}

#[test]
fn stagehand_exact_domain_does_not_allow_subdomain() {
    let policy = stagehand_with_domains(["example.com"]);
    let decision = policy.evaluate(Action::ObserveUrl("https://sub.example.com/path".to_string()));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "domain_not_allowlisted".to_string()
        }
    );
}

#[test]
fn stagehand_wildcard_domain_allows_subdomain() {
    let policy = stagehand_with_domains(["*.example.com"]);
    let decision = policy.evaluate(Action::ObserveUrl("https://sub.example.com/path".to_string()));

    assert_eq!(
        decision,
        PermissionDecision::Allow {
            reason_code: "domain_allowlisted".to_string()
        }
    );
}

#[test]
fn stagehand_wildcard_domain_does_not_allow_apex() {
    let policy = stagehand_with_domains(["*.example.com"]);
    let decision = policy.evaluate(Action::ObserveUrl("https://example.com/path".to_string()));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "domain_not_allowlisted".to_string()
        }
    );
}

#[test]
fn stagehand_wildcard_with_scheme_allows_subdomain() {
    let policy = stagehand_with_domains(["https://*.example.com"]);
    let decision = policy.evaluate(Action::ObserveUrl("https://sub.example.com/path".to_string()));

    assert_eq!(
        decision,
        PermissionDecision::Allow {
            reason_code: "domain_allowlisted".to_string()
        }
    );
}

#[test]
fn stagehand_allows_query_form_url_host_parse() {
    let policy = stagehand_with_domains(["example.com"]);
    let decision = policy.evaluate(Action::ObserveUrl("https://example.com?x=1".to_string()));

    assert_eq!(
        decision,
        PermissionDecision::Allow {
            reason_code: "domain_allowlisted".to_string()
        }
    );
}

#[test]
fn stagehand_denies_command_with_absolute_path_outside_workspace() {
    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat"])
        .with_workspaces(["/home/orchestrator/odin-core"]);

    let decision = policy.evaluate(Action::RunCommand("cat /etc/passwd".to_string()));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "command_path_outside_allowlisted_workspace".to_string()
        }
    );
}

#[test]
fn stagehand_denies_workspace_parent_traversal_outside_allowlist() {
    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_workspaces(["/home/orchestrator/allowed"]);

    let decision = policy.evaluate(Action::ReadWorkspace(
        "/home/orchestrator/allowed/../outside".to_string(),
    ));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "workspace_not_allowlisted".to_string()
        }
    );
}

#[test]
fn stagehand_denies_command_with_parent_traversal_outside_workspace() {
    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat"])
        .with_workspaces(["/home/orchestrator/allowed"]);

    let decision = policy.evaluate(Action::RunCommand(
        "cat /home/orchestrator/allowed/../outside/secrets.txt".to_string(),
    ));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "command_path_outside_allowlisted_workspace".to_string()
        }
    );
}

#[test]
fn stagehand_denies_command_with_shell_metacharacters() {
    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat"])
        .with_workspaces(["/home/orchestrator/odin-core"]);

    let decision = policy.evaluate(Action::RunCommand(
        "cat /home/orchestrator/odin-core/README.md; id".to_string(),
    ));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "command_unsafe_shell_syntax".to_string()
        }
    );
}

#[test]
fn stagehand_denies_command_with_relative_parent_traversal() {
    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat"])
        .with_workspaces(["/home/orchestrator/odin-core"]);

    let decision = policy.evaluate(Action::RunCommand("cat ../outside/secrets.txt".to_string()));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "command_relative_path_traversal".to_string()
        }
    );
}

#[test]
fn stagehand_denies_unscoped_relative_command_arg_when_workspace_boundaries_active() {
    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat"])
        .with_workspaces(["/home/orchestrator/odin-core"]);

    let decision = policy.evaluate(Action::RunCommand("cat docs/plan.md".to_string()));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "command_relative_path_unscoped".to_string()
        }
    );
}

#[test]
fn stagehand_untrusted_envelope_cannot_enable_plugin() {
    let envelope = PluginPermissionEnvelope {
        plugin: "stagehand".to_string(),
        trust_level: TrustLevel::Untrusted,
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
    };
    let policy = stagehand_policy_from_envelope(&envelope);
    let decision = policy.evaluate(Action::ObserveUrl("https://example.com".to_string()));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "plugin_disabled".to_string()
        }
    );
}
