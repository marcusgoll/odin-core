use odin_governance::plugins::{
    Action, PermissionDecision, stagehand_default_policy, stagehand_with_domains,
};

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
