import * as fs from "node:fs";
import { parse as parseYaml } from "yaml";

export interface MessageMeta {
  id: string;
  threadId: string;
  from: string;
  to: string;
  subject: string;
  headers: Record<string, string>;
  snippet: string;
}

export interface TriageAction {
  label?: string;
  archive?: boolean;
  request_trash?: boolean;
  draft_reply?: boolean;
}

export interface TriageMatch {
  from_pattern?: string;
  or_subject_pattern?: string;
  subject_pattern?: string;
  headers?: Record<string, string>;
  is_direct?: boolean;
  from_in_contacts?: boolean;
  has_question?: boolean;
  no_rule_matched?: boolean;
}

export interface TriageRule {
  name: string;
  match: TriageMatch;
  actions: TriageAction[];
}

export interface TriageResult {
  name: string;
  actions: TriageAction[];
}

export function loadRules(rulesPath: string): TriageRule[] {
  const content = fs.readFileSync(rulesPath, "utf-8");
  const parsed = parseYaml(content) as { rules: TriageRule[] };
  return parsed.rules || [];
}

function matchesPattern(text: string, pattern: string): boolean {
  try {
    return new RegExp(pattern, "i").test(text);
  } catch {
    return false;
  }
}

function matchesRule(rule: TriageRule, msg: MessageMeta): boolean {
  const m = rule.match;

  // Catch-all rule
  if (m.no_rule_matched) return true;

  // Skip LLM-dependent matchers at this level (handled by plugin orchestration)
  if (m.has_question || m.from_in_contacts || m.is_direct) {
    // These require external context — only match if all non-contextual
    // conditions also match. For MVP, skip rules that ONLY have contextual matchers.
    const hasNonContextual = m.from_pattern || m.or_subject_pattern || m.subject_pattern || m.headers;
    if (!hasNonContextual) return false;
  }

  let matched = false;

  if (m.from_pattern) {
    if (matchesPattern(msg.from, m.from_pattern)) matched = true;
  }

  if (m.or_subject_pattern) {
    if (matchesPattern(msg.subject, m.or_subject_pattern)) matched = true;
  }

  if (m.subject_pattern) {
    if (!matchesPattern(msg.subject, m.subject_pattern)) return false;
    matched = true;
  }

  if (m.headers) {
    for (const [key, value] of Object.entries(m.headers)) {
      if (value === "present") {
        if (!(key in msg.headers)) return false;
        matched = true;
      }
    }
  }

  return matched;
}

export function evaluateRules(
  rules: TriageRule[],
  msg: MessageMeta,
): TriageResult | null {
  for (const rule of rules) {
    if (matchesRule(rule, msg)) {
      return { name: rule.name, actions: rule.actions };
    }
  }
  return null;
}
