#!/usr/bin/env node
/**
 * Sets up Gmail Pub/Sub push notifications:
 * 1. OAuth flow with Gmail + Pub/Sub scopes
 * 2. Creates Pub/Sub topic
 * 3. Grants Gmail publish permission
 * 4. Registers Gmail watch
 *
 * Usage: GMAIL_CLIENT_ID=... GMAIL_CLIENT_SECRET=... GCP_PROJECT_ID=... node scripts/setup-pubsub-watch.mjs
 */

import { google } from "googleapis";
import { OAuth2Client } from "google-auth-library";
import * as http from "node:http";
import * as fs from "node:fs";
import * as url from "node:url";

const CLIENT_ID = process.env.GMAIL_CLIENT_ID;
const CLIENT_SECRET = process.env.GMAIL_CLIENT_SECRET;
const PROJECT_ID = process.env.GCP_PROJECT_ID;
const TOPIC_NAME = process.env.PUBSUB_TOPIC || "gmail-push-notifications";
const WEBHOOK_URL = process.env.WEBHOOK_URL || "https://n8n.marcusgoll.com/webhook/gmail-push";
const PORT = 8844;
const REDIRECT_URI = `http://localhost:${PORT}/oauth2callback`;

if (!CLIENT_ID || !CLIENT_SECRET) {
  console.error("Error: GMAIL_CLIENT_ID and GMAIL_CLIENT_SECRET are required.");
  process.exit(1);
}
if (!PROJECT_ID) {
  console.error("Error: GCP_PROJECT_ID is required.");
  process.exit(1);
}

const SCOPES = [
  "https://www.googleapis.com/auth/gmail.modify",
  "https://www.googleapis.com/auth/gmail.compose",
  "https://www.googleapis.com/auth/gmail.labels",
  "https://www.googleapis.com/auth/pubsub",
];

const FULL_TOPIC = `projects/${PROJECT_ID}/topics/${TOPIC_NAME}`;

const oauth2 = new OAuth2Client(CLIENT_ID, CLIENT_SECRET, REDIRECT_URI);

function waitForCode() {
  return new Promise((resolve, reject) => {
    const server = http.createServer((req, res) => {
      if (!req.url?.startsWith("/oauth2callback")) {
        res.writeHead(404);
        res.end("Not found");
        return;
      }
      const parsed = new url.URL(req.url, `http://localhost:${PORT}`);
      const code = parsed.searchParams.get("code");
      const error = parsed.searchParams.get("error");
      if (error) {
        res.writeHead(400, { "Content-Type": "text/html" });
        res.end(`<h1>Failed</h1><p>${error}</p>`);
        server.close();
        reject(new Error(error));
        return;
      }
      res.writeHead(200, { "Content-Type": "text/html" });
      res.end("<h1>Success!</h1><p>Pub/Sub setup in progress. You can close this tab.</p>");
      server.close();
      resolve(code);
    });
    server.listen(PORT);
  });
}

async function main() {
  // Step 1: OAuth
  const authUrl = oauth2.generateAuthUrl({
    access_type: "offline",
    scope: SCOPES,
    prompt: "consent",
  });
  console.log("\n=== Step 1: Open this URL in your browser ===\n");
  console.log(authUrl);
  console.log("\n=== Waiting for OAuth callback... ===\n");

  const code = await waitForCode();
  const { tokens } = await oauth2.getToken(code);
  oauth2.setCredentials(tokens);
  console.log("[OK] Authenticated with Gmail + Pub/Sub scopes");

  // Save updated token (now includes pubsub scope)
  const tokenPayload = JSON.stringify({
    access_token: tokens.access_token,
    refresh_token: tokens.refresh_token,
    client_id: CLIENT_ID,
    client_secret: CLIENT_SECRET,
  });
  const secretsDir = "/var/odin/secrets";
  fs.mkdirSync(secretsDir, { recursive: true, mode: 0o700 });
  fs.writeFileSync(`${secretsDir}/gmail-token.json`, tokenPayload, { mode: 0o600 });
  console.log("[OK] Token saved to /var/odin/secrets/gmail-token.json");

  // Step 2: Create Pub/Sub topic
  const pubsub = google.pubsub({ version: "v1", auth: oauth2 });
  try {
    await pubsub.projects.topics.create({ name: FULL_TOPIC });
    console.log(`[OK] Created topic: ${FULL_TOPIC}`);
  } catch (err) {
    if (err.code === 409) {
      console.log(`[OK] Topic already exists: ${FULL_TOPIC}`);
    } else {
      throw err;
    }
  }

  // Step 3: Grant Gmail permission to publish
  try {
    const policy = await pubsub.projects.topics.getIamPolicy({ resource: FULL_TOPIC });
    const bindings = policy.data.bindings || [];
    const publisherBinding = bindings.find(
      (b) => b.role === "roles/pubsub.publisher"
    );
    const gmailSA = "serviceAccount:gmail-api-push@system.gserviceaccount.com";

    if (publisherBinding && publisherBinding.members?.includes(gmailSA)) {
      console.log("[OK] Gmail publish permission already granted");
    } else {
      bindings.push({
        role: "roles/pubsub.publisher",
        members: [gmailSA],
      });
      await pubsub.projects.topics.setIamPolicy({
        resource: FULL_TOPIC,
        requestBody: { policy: { bindings } },
      });
      console.log("[OK] Granted publish permission to gmail-api-push service account");
    }
  } catch (err) {
    console.error("[WARN] Could not set IAM policy:", err.message);
    console.error("       You may need to grant roles/pubsub.publisher to");
    console.error("       gmail-api-push@system.gserviceaccount.com manually.");
  }

  // Step 4: Register Gmail watch
  const gmail = google.gmail({ version: "v1", auth: oauth2 });
  try {
    const watchRes = await gmail.users.watch({
      userId: "me",
      requestBody: {
        topicName: FULL_TOPIC,
        labelIds: ["INBOX"],
      },
    });
    console.log(`[OK] Gmail watch registered!`);
    console.log(`     historyId: ${watchRes.data.historyId}`);
    console.log(`     expiration: ${new Date(Number(watchRes.data.expiration)).toISOString()}`);
    console.log(`\n     Watch expires in ~7 days. Set up a cron to renew it.`);
  } catch (err) {
    console.error("[ERROR] Gmail watch failed:", err.message);
    throw err;
  }

  console.log("\n=== Setup complete! ===");
  console.log(`Topic:   ${FULL_TOPIC}`);
  console.log(`Webhook: ${WEBHOOK_URL}`);
  console.log(`Flow:    Gmail → Pub/Sub → n8n webhook → Odin inbox → Gmail plugin`);
}

main().catch((err) => {
  console.error("\nFatal:", err.message);
  process.exit(1);
});
