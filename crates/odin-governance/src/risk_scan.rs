#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RiskCategory {
    Shell,
    Network,
    Secret,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiskFinding {
    pub category: RiskCategory,
    pub pattern: &'static str,
}

const SHELL_PATTERNS: &[&str] = &["curl | sh", "| sh", "| bash", "bash -c", "sh -c"];
const NETWORK_PATTERNS: &[&str] = &[
    "curl ",
    "wget ",
    "invoke-webrequest",
    "invoke-restmethod",
    "requests.",
    "http.client",
    "reqwest::",
    "net/http",
    "axios.",
    "fetch(",
];
const SECRET_PATTERNS: &[&str] = &[
    "aws_secret",
    "github_token",
    "access_token",
    "secret_key",
    "api_key",
    "token=",
    "password=",
];
const DELETE_PATTERNS: &[&str] = &["rm -rf", "del /f", "shred "];

pub fn scan_skill_content(scripts: &[String], readme: Option<&str>) -> Vec<RiskFinding> {
    let mut findings = Vec::new();

    for script in scripts {
        scan_text(script, &mut findings);
    }

    if let Some(readme_text) = readme {
        scan_text(readme_text, &mut findings);
    }

    findings
}

fn scan_text(text: &str, findings: &mut Vec<RiskFinding>) {
    let normalized = text.to_ascii_lowercase();
    collect_matches(&normalized, SHELL_PATTERNS, RiskCategory::Shell, findings);
    collect_matches(
        &normalized,
        NETWORK_PATTERNS,
        RiskCategory::Network,
        findings,
    );
    collect_matches(&normalized, SECRET_PATTERNS, RiskCategory::Secret, findings);
    collect_matches(&normalized, DELETE_PATTERNS, RiskCategory::Delete, findings);
}

fn collect_matches(
    haystack: &str,
    patterns: &[&'static str],
    category: RiskCategory,
    findings: &mut Vec<RiskFinding>,
) {
    for pattern in patterns {
        if haystack.contains(pattern)
            && !findings
                .iter()
                .any(|finding| finding.category == category && finding.pattern == *pattern)
        {
            findings.push(RiskFinding {
                category: category.clone(),
                pattern,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{scan_skill_content, RiskCategory};

    #[test]
    fn scanner_detects_shell_findings() {
        let scripts = vec!["#!/usr/bin/env bash\nbash -c 'echo hi'".to_string()];

        let findings = scan_skill_content(&scripts, None);

        assert!(
            findings
                .iter()
                .any(|finding| finding.category == RiskCategory::Shell),
            "expected shell finding"
        );
    }

    #[test]
    fn scanner_detects_network_findings() {
        let scripts = vec!["#!/usr/bin/env bash\nwget https://example.com/install.sh".to_string()];

        let findings = scan_skill_content(&scripts, None);

        assert!(
            findings
                .iter()
                .any(|finding| finding.category == RiskCategory::Network),
            "expected network finding"
        );
    }

    #[test]
    fn scanner_detects_delete_findings() {
        let scripts = vec!["#!/usr/bin/env bash\nrm -rf ./tmp/cache".to_string()];

        let findings = scan_skill_content(&scripts, None);

        assert!(
            findings
                .iter()
                .any(|finding| finding.category == RiskCategory::Delete),
            "expected delete finding"
        );
    }

    #[test]
    fn scanner_deduplicates_identical_findings_per_source() {
        let scripts = vec![
            "#!/usr/bin/env bash\nwget https://example.com/a\nwget https://example.com/b"
                .to_string(),
        ];

        let findings = scan_skill_content(&scripts, None);
        let network_wget_matches = findings
            .iter()
            .filter(|finding| {
                finding.category == RiskCategory::Network && finding.pattern == "wget "
            })
            .count();

        assert_eq!(
            network_wget_matches, 1,
            "expected a single deduplicated finding for repeated wget pattern"
        );
    }

    #[test]
    fn scanner_detects_common_secret_markers_without_equals_sign() {
        let scripts = vec![
            "#!/usr/bin/env bash\necho GITHUB_TOKEN".to_string(),
            "#!/usr/bin/env bash\necho ACCESS_TOKEN".to_string(),
            "#!/usr/bin/env bash\necho API_KEY".to_string(),
            "#!/usr/bin/env bash\necho SECRET_KEY".to_string(),
        ];

        let findings = scan_skill_content(&scripts, None);

        for marker in ["github_token", "access_token", "api_key", "secret_key"] {
            assert!(
                findings.iter().any(|finding| {
                    finding.category == RiskCategory::Secret && finding.pattern == marker
                }),
                "expected secret finding for marker {marker}"
            );
        }
    }
}
