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
