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
const SECRET_PATTERNS: &[&str] = &["aws_secret", "secret_key", "api_key", "token=", "password="];
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
    collect_matches(&normalized, NETWORK_PATTERNS, RiskCategory::Network, findings);
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
