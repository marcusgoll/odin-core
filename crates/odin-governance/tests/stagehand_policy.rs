use odin_governance::plugins::{
    Action, PermissionDecision, stagehand_default_policy, stagehand_policy_from_envelope,
    stagehand_with_domains,
};
use odin_plugin_protocol::{DelegationCapability, PluginPermissionEnvelope, TrustLevel};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn stagehand_denies_relative_path_in_option_value_when_workspace_boundaries_active() {
    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat"])
        .with_workspaces(["/home/orchestrator/odin-core"]);

    let decision = policy.evaluate(Action::RunCommand(
        "cat --input=relative/path/file.txt".to_string(),
    ));

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

#[test]
fn stagehand_rejects_whitespace_command_scope_entry() {
    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat --color=always"]);

    let decision = policy.evaluate(Action::RunCommand("cat".to_string()));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "command_not_allowlisted".to_string()
        }
    );
}

#[test]
fn stagehand_denies_command_with_newline_control_separator() {
    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat"]);

    let decision = policy.evaluate(Action::RunCommand("cat /tmp/file\nid".to_string()));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "command_unsafe_shell_syntax".to_string()
        }
    );
}

#[test]
fn stagehand_allows_absolute_option_path_within_workspace() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!("odin-stagehand-allow-{unique}"));
    let file = workspace.join("input.txt");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::write(&file, "fixture").expect("write fixture");

    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat"])
        .with_workspaces([workspace.to_string_lossy().to_string()]);

    let decision = policy.evaluate(Action::RunCommand(
        format!("cat --input={}", file.to_string_lossy()),
    ));

    assert_eq!(
        decision,
        PermissionDecision::Allow {
            reason_code: "command_allowlisted".to_string()
        }
    );

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

#[test]
fn stagehand_allows_scalar_option_value_when_workspace_boundaries_active() {
    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat"])
        .with_workspaces(["/home/orchestrator/odin-core"]);

    let decision = policy.evaluate(Action::RunCommand("cat --color=always".to_string()));

    assert_eq!(
        decision,
        PermissionDecision::Allow {
            reason_code: "command_allowlisted".to_string()
        }
    );
}

#[test]
fn stagehand_denies_unresolved_absolute_command_path_fail_closed() {
    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat"])
        .with_workspaces(["/home/orchestrator/odin-core"]);

    let decision = policy.evaluate(Action::RunCommand(
        "cat /home/orchestrator/odin-core/.worktrees/skill-plugin-governance/does-not-exist-4f91de39/secret.txt".to_string(),
    ));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "command_path_outside_allowlisted_workspace".to_string()
        }
    );
}

#[test]
fn stagehand_denies_quoted_parent_traversal_option_value_without_workspace_policy() {
    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat"]);

    let decision = policy.evaluate(Action::RunCommand(
        "cat --input=\"../outside/secrets.txt\"".to_string(),
    ));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "command_unsafe_shell_syntax".to_string()
        }
    );
}

#[test]
fn stagehand_allows_positional_scalar_with_in_workspace_absolute_path() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!("odin-stagehand-positional-allow-{unique}"));
    let file = workspace.join("input.txt");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::write(&file, "fixture").expect("write fixture");

    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat"])
        .with_workspaces([workspace.to_string_lossy().to_string()]);

    let decision = policy.evaluate(Action::RunCommand(format!(
        "cat always {}",
        file.to_string_lossy()
    )));

    assert_eq!(
        decision,
        PermissionDecision::Allow {
            reason_code: "command_allowlisted".to_string()
        }
    );

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

#[test]
fn stagehand_denies_attached_short_option_absolute_path_outside_workspace() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let allowed_workspace = std::env::temp_dir().join(format!("odin-stagehand-allowed-{unique}"));
    let outside_workspace = std::env::temp_dir().join(format!("odin-stagehand-outside-{unique}"));
    let outside_file = outside_workspace.join("secret.txt");
    fs::create_dir_all(&allowed_workspace).expect("create allowed workspace");
    fs::create_dir_all(&outside_workspace).expect("create outside workspace");
    fs::write(&outside_file, "secret").expect("write outside fixture");

    let policy = stagehand_default_policy()
        .with_enabled(true)
        .with_commands(["cat"])
        .with_workspaces([allowed_workspace.to_string_lossy().to_string()]);

    let decision = policy.evaluate(Action::RunCommand(format!(
        "cat -f{}",
        outside_file.to_string_lossy()
    )));

    assert_eq!(
        decision,
        PermissionDecision::Deny {
            reason_code: "command_path_outside_allowlisted_workspace".to_string()
        }
    );

    fs::remove_dir_all(&allowed_workspace).expect("cleanup allowed workspace");
    fs::remove_dir_all(&outside_workspace).expect("cleanup outside workspace");
}
