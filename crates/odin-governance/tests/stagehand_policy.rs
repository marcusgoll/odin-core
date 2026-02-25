use odin_governance::plugins::{
    Action, PermissionDecision, stagehand_default_policy, stagehand_with_domains,
};

#[test]
fn stagehand_denies_login_by_default() {
    let policy = stagehand_default_policy();
    let decision = policy.evaluate(Action::Login);

    assert!(matches!(decision, PermissionDecision::Deny { .. }));
}

#[test]
fn stagehand_denies_domain_outside_allowlist() {
    let policy = stagehand_with_domains(["example.com"]);
    let decision = policy.evaluate(Action::ObserveUrl("https://not-allowed.dev".to_string()));

    assert!(matches!(decision, PermissionDecision::Deny { .. }));
}
