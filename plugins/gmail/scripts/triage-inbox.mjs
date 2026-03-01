#!/usr/bin/env node
import { google } from "googleapis";
import { OAuth2Client } from "google-auth-library";
import * as fs from "node:fs";
import { parse as parseYaml } from "yaml";

const token = JSON.parse(fs.readFileSync("/var/odin/secrets/gmail-token.json", "utf-8"));
const oauth2 = new OAuth2Client(token.client_id, token.client_secret);
oauth2.setCredentials({ refresh_token: token.refresh_token });

const rulesContent = fs.readFileSync("/var/odin/config/gmail-rules.yaml", "utf-8");
const rules = parseYaml(rulesContent).rules;

function matchesPattern(text, pattern) {
  try { return new RegExp(pattern, "i").test(text); } catch { return false; }
}

function evaluateRules(msg) {
  for (const rule of rules) {
    const m = rule.match;
    if (m.no_rule_matched) return rule;
    if (m.has_question || m.from_in_contacts || m.is_direct) {
      if (!m.from_pattern && !m.or_subject_pattern && !m.subject_pattern && !m.headers) continue;
    }
    let matched = false;
    if (m.from_pattern && matchesPattern(msg.from, m.from_pattern)) matched = true;
    if (m.or_subject_pattern && matchesPattern(msg.subject, m.or_subject_pattern)) matched = true;
    if (m.subject_pattern) {
      if (!matchesPattern(msg.subject, m.subject_pattern)) continue;
      matched = true;
    }
    if (m.headers) {
      let headersOk = true;
      for (const [key, value] of Object.entries(m.headers)) {
        if (value === "present") {
          if (!(key in msg.headers)) { headersOk = false; break; }
          matched = true;
        }
      }
      if (!headersOk) continue;
    }
    if (matched) return rule;
  }
  return rules.find(r => r.match.no_rule_matched) || null;
}

async function main() {
  const { credentials } = await oauth2.refreshAccessToken();
  oauth2.setCredentials(credentials);
  const gmail = google.gmail({ version: "v1", auth: oauth2 });

  // Get all labels (for creating missing ones)
  const labelsRes = await gmail.users.labels.list({ userId: "me" });
  const existingLabels = new Map(labelsRes.data.labels.map(l => [l.name, l.id]));

  async function ensureLabel(name) {
    if (existingLabels.has(name)) return existingLabels.get(name);
    const res = await gmail.users.labels.create({
      userId: "me",
      requestBody: { name, labelListVisibility: "labelShow", messageListVisibility: "show" },
    });
    existingLabels.set(name, res.data.id);
    console.log(`  [CREATED] Label: ${name}`);
    return res.data.id;
  }

  // Get all inbox messages
  let allMessages = [];
  let pageToken = undefined;
  do {
    const res = await gmail.users.messages.list({
      userId: "me",
      labelIds: ["INBOX"],
      maxResults: 100,
      pageToken,
    });
    allMessages.push(...(res.data.messages || []));
    pageToken = res.data.nextPageToken;
  } while (pageToken);

  console.log(`Total inbox messages: ${allMessages.length}\n`);

  const dryRun = process.argv.includes("--dry-run");
  if (dryRun) console.log("=== DRY RUN (no changes) ===\n");

  const summary = {};
  let archived = 0;
  let labeled = 0;

  for (const m of allMessages) {
    const msg = await gmail.users.messages.get({
      userId: "me",
      id: m.id,
      format: "metadata",
      metadataHeaders: ["From", "Subject", "List-Unsubscribe"],
    });
    const headers = {};
    for (const h of msg.data.payload.headers || []) {
      headers[h.name] = h.value;
    }
    const from = headers["From"] || "";
    const subject = headers["Subject"] || "";

    const rule = evaluateRules({ from, subject, headers });
    const ruleName = rule?.name || "uncategorized";
    summary[ruleName] = (summary[ruleName] || 0) + 1;

    const labelAction = rule?.actions?.find(a => a.label);
    const archiveAction = rule?.actions?.some(a => a.archive);

    const tag = archiveAction ? "[ARCHIVE]" : "[KEEP]   ";
    const labelName = labelAction?.label || "-";
    console.log(`${tag} ${labelName.padEnd(18)} ${from.substring(0, 45).padEnd(47)} ${subject.substring(0, 60)}`);

    if (!dryRun) {
      // Apply label
      if (labelAction) {
        const labelId = await ensureLabel(labelAction.label);
        await gmail.users.messages.modify({
          userId: "me",
          id: m.id,
          requestBody: { addLabelIds: [labelId] },
        });
        labeled++;
      }
      // Archive (remove INBOX label)
      if (archiveAction) {
        await gmail.users.messages.modify({
          userId: "me",
          id: m.id,
          requestBody: { removeLabelIds: ["INBOX"] },
        });
        archived++;
      }
    }
  }

  console.log("\n=== Summary ===");
  for (const [name, count] of Object.entries(summary).sort((a, b) => b[1] - a[1])) {
    console.log(`  ${name}: ${count}`);
  }
  if (!dryRun) {
    console.log(`\nApplied: ${labeled} labeled, ${archived} archived`);
  }
}

main().catch(err => { console.error("Fatal:", err.message); process.exit(1); });
