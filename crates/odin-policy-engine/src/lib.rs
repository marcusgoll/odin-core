//! Policy engine contracts and baseline implementation.

use std::collections::HashSet;

use odin_plugin_protocol::{ActionRequest, PolicyDecision, RiskTier};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("evaluation error: {0}")]
    Evaluation(String),
}

pub type PolicyResult<T> = Result<T, PolicyError>;

pub trait PolicyEngine: Send + Sync {
    fn decide(&self, request: &ActionRequest) -> PolicyResult<PolicyDecision>;
}

#[derive(Clone, Debug, Default)]
pub struct StaticPolicyEngine {
    allowed: HashSet<(String, String, String)>,
    pub require_approval_for_destructive: bool,
}

impl StaticPolicyEngine {
    pub fn set_require_approval_for_destructive(&mut self, required: bool) {
        self.require_approval_for_destructive = required;
    }

    pub fn allow_capability(&mut self, plugin: &str, project: &str, capability: &str) {
        self.allowed.insert((
            plugin.to_string(),
            project.to_string(),
            capability.to_string(),
        ));
    }

    fn is_allowed(&self, plugin: &str, project: &str, capability: &str) -> bool {
        self.allowed.contains(&(
            plugin.to_string(),
            project.to_string(),
            capability.to_string(),
        )) || self
            .allowed
            .contains(&(plugin.to_string(), "*".to_string(), capability.to_string()))
    }
}

impl PolicyEngine for StaticPolicyEngine {
    fn decide(&self, request: &ActionRequest) -> PolicyResult<PolicyDecision> {
        let cap = &request.capability;
        if cap.plugin.trim().is_empty() || cap.capability.trim().is_empty() {
            return Err(PolicyError::InvalidRequest(
                "plugin and capability are required".to_string(),
            ));
        }

        if !self.is_allowed(&cap.plugin, &cap.project, &cap.capability) {
            return Ok(PolicyDecision::Deny {
                reason_code: "capability_not_granted".to_string(),
            });
        }

        if request.capability.capability == "prod.deploy"
            && request
                .run_context
                .as_ref()
                .is_some_and(|ctx| ctx.autonomy_level.trim().to_ascii_uppercase() == "L1")
        {
            return Ok(PolicyDecision::Deny {
                reason_code: "autonomy_level_block".to_string(),
            });
        }

        if matches!(request.risk_tier, RiskTier::Destructive)
            && self.require_approval_for_destructive
        {
            return Ok(PolicyDecision::RequireApproval {
                reason_code: "destructive_requires_approval".to_string(),
                tier: RiskTier::Destructive,
            });
        }

        Ok(PolicyDecision::Allow {
            reason_code: "capability_granted".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use odin_plugin_protocol::{ActionRequest, CapabilityRequest, RiskTier, RunContext};

    use super::{PolicyEngine, StaticPolicyEngine};

    fn make_request(risk_tier: RiskTier) -> ActionRequest {
        ActionRequest {
            request_id: "req-1".to_string(),
            risk_tier,
            capability: CapabilityRequest {
                plugin: "example.safe-github".to_string(),
                project: "demo".to_string(),
                capability: "repo.read".to_string(),
                scope: vec!["project".to_string()],
                reason: "read repository metadata".to_string(),
            },
            input: serde_json::Value::Null,
            run_context: None,
        }
    }

    #[test]
    fn denies_when_not_granted() {
        let engine = StaticPolicyEngine::default();
        let decision = engine
            .decide(&make_request(RiskTier::Safe))
            .expect("decision");
        assert!(matches!(
            decision,
            odin_plugin_protocol::PolicyDecision::Deny { .. }
        ));
    }

    #[test]
    fn allows_when_granted() {
        let mut engine = StaticPolicyEngine::default();
        engine.allow_capability("example.safe-github", "demo", "repo.read");

        let decision = engine
            .decide(&make_request(RiskTier::Safe))
            .expect("decision");
        assert!(matches!(
            decision,
            odin_plugin_protocol::PolicyDecision::Allow { .. }
        ));
    }

    #[test]
    fn requires_approval_for_destructive_when_enabled() {
        let mut engine = StaticPolicyEngine {
            require_approval_for_destructive: true,
            ..StaticPolicyEngine::default()
        };
        engine.allow_capability("example.safe-github", "demo", "repo.read");

        let decision = engine
            .decide(&make_request(RiskTier::Destructive))
            .expect("decision");
        assert!(matches!(
            decision,
            odin_plugin_protocol::PolicyDecision::RequireApproval { .. }
        ));
    }

    #[test]
    fn l1_blocks_prod_deploy_capability() {
        let mut engine = StaticPolicyEngine::default();
        engine.allow_capability("example.safe-github", "demo", "prod.deploy");

        let mut request = make_request(RiskTier::Safe);
        request.capability.capability = "prod.deploy".to_string();
        request.run_context = Some(RunContext {
            run_id: "run-1".to_string(),
            autonomy_level: "L1".to_string(),
            risk_class: "safe".to_string(),
            policy_profile: "default".to_string(),
            tool_subset_id: "core".to_string(),
        });

        let decision = engine.decide(&request).expect("decision");
        assert_eq!(
            decision,
            odin_plugin_protocol::PolicyDecision::Deny {
                reason_code: "autonomy_level_block".to_string(),
            }
        );
    }

    #[test]
    fn l1_case_whitespace_variant_blocks_prod_deploy_capability() {
        let mut engine = StaticPolicyEngine::default();
        engine.allow_capability("example.safe-github", "demo", "prod.deploy");

        let mut request = make_request(RiskTier::Safe);
        request.capability.capability = "prod.deploy".to_string();
        request.run_context = Some(RunContext {
            run_id: "run-1".to_string(),
            autonomy_level: " l1 ".to_string(),
            risk_class: "safe".to_string(),
            policy_profile: "default".to_string(),
            tool_subset_id: "core".to_string(),
        });

        let decision = engine.decide(&request).expect("decision");
        assert_eq!(
            decision,
            odin_plugin_protocol::PolicyDecision::Deny {
                reason_code: "autonomy_level_block".to_string(),
            }
        );
    }

    #[test]
    fn non_l1_context_allows_prod_deploy_when_capability_granted() {
        let mut engine = StaticPolicyEngine::default();
        engine.allow_capability("example.safe-github", "demo", "prod.deploy");

        let mut request = make_request(RiskTier::Safe);
        request.capability.capability = "prod.deploy".to_string();
        request.run_context = Some(RunContext {
            run_id: "run-1".to_string(),
            autonomy_level: "L2".to_string(),
            risk_class: "safe".to_string(),
            policy_profile: "default".to_string(),
            tool_subset_id: "core".to_string(),
        });

        let decision = engine.decide(&request).expect("decision");
        assert_eq!(
            decision,
            odin_plugin_protocol::PolicyDecision::Allow {
                reason_code: "capability_granted".to_string(),
            }
        );
    }

    #[test]
    fn missing_run_context_allows_prod_deploy_when_capability_granted() {
        let mut engine = StaticPolicyEngine::default();
        engine.allow_capability("example.safe-github", "demo", "prod.deploy");

        let mut request = make_request(RiskTier::Safe);
        request.capability.capability = "prod.deploy".to_string();
        request.run_context = None;

        let decision = engine.decide(&request).expect("decision");
        assert_eq!(
            decision,
            odin_plugin_protocol::PolicyDecision::Allow {
                reason_code: "capability_granted".to_string(),
            }
        );
    }
}
