#!/usr/bin/env node
/**
 * Renews the Gmail Pub/Sub watch.
 * Run via cron every 6 days to prevent expiration (watch lasts ~7 days).
 *
 * Usage: node scripts/renew-gmail-watch.mjs
 * Reads credentials from /var/odin/secrets/gmail-token.json
 */

import { google } from "googleapis";
import { OAuth2Client } from "google-auth-library";
import * as fs from "node:fs";

const TOKEN_PATH = process.env.GMAIL_TOKEN_PATH || "/var/odin/secrets/gmail-token.json";
const PROJECT_ID = process.env.GCP_PROJECT_ID || "cfipro-436322";
const TOPIC_NAME = process.env.PUBSUB_TOPIC || "gmail-push-notifications";
const FULL_TOPIC = `projects/${PROJECT_ID}/topics/${TOPIC_NAME}`;

const token = JSON.parse(fs.readFileSync(TOKEN_PATH, "utf-8"));
const oauth2 = new OAuth2Client(token.client_id, token.client_secret);
oauth2.setCredentials({ refresh_token: token.refresh_token });

async function main() {
  const { credentials } = await oauth2.refreshAccessToken();
  oauth2.setCredentials(credentials);

  const gmail = google.gmail({ version: "v1", auth: oauth2 });
  const watchRes = await gmail.users.watch({
    userId: "me",
    requestBody: {
      topicName: FULL_TOPIC,
      labelIds: ["INBOX"],
    },
  });

  const expiration = new Date(Number(watchRes.data.expiration)).toISOString();
  console.log(`[${new Date().toISOString()}] Gmail watch renewed. historyId=${watchRes.data.historyId} expires=${expiration}`);
}

main().catch((err) => {
  console.error(`[${new Date().toISOString()}] Gmail watch renewal FAILED:`, err.message);
  process.exit(1);
});
