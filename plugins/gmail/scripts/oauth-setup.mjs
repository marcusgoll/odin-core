#!/usr/bin/env node
/**
 * Gmail OAuth2 setup helper for Odin Gmail Plugin.
 * Starts a temporary local server to handle the OAuth callback.
 *
 * Usage: node scripts/oauth-setup.mjs
 */

import { OAuth2Client } from "google-auth-library";
import * as http from "node:http";
import * as fs from "node:fs";
import * as url from "node:url";

const CLIENT_ID = process.env.GMAIL_CLIENT_ID;
const CLIENT_SECRET = process.env.GMAIL_CLIENT_SECRET;

if (!CLIENT_ID || !CLIENT_SECRET) {
  console.error("Error: GMAIL_CLIENT_ID and GMAIL_CLIENT_SECRET environment variables are required.");
  console.error("Usage: GMAIL_CLIENT_ID=... GMAIL_CLIENT_SECRET=... node scripts/oauth-setup.mjs");
  process.exit(1);
}
const PORT = 8844;
const REDIRECT_URI = `http://localhost:${PORT}/oauth2callback`;
const SCOPES = [
  "https://www.googleapis.com/auth/gmail.modify",
  "https://www.googleapis.com/auth/gmail.compose",
  "https://www.googleapis.com/auth/gmail.labels",
];

const oauth2 = new OAuth2Client(CLIENT_ID, CLIENT_SECRET, REDIRECT_URI);

const authUrl = oauth2.generateAuthUrl({
  access_type: "offline",
  scope: SCOPES,
  prompt: "consent",
});

console.log("\n=== Open this URL in your browser ===\n");
console.log(authUrl);
console.log("\n=== Waiting for OAuth callback on port", PORT, "===\n");

const server = http.createServer(async (req, res) => {
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
    res.end(`<h1>Authorization failed</h1><p>${error}</p>`);
    console.error("Authorization failed:", error);
    server.close();
    process.exit(1);
  }

  if (!code) {
    res.writeHead(400, { "Content-Type": "text/html" });
    res.end("<h1>No code received</h1>");
    return;
  }

  try {
    const { tokens } = await oauth2.getToken(code);
    const tokenPayload = JSON.stringify({
      access_token: tokens.access_token,
      refresh_token: tokens.refresh_token,
      client_id: CLIENT_ID,
      client_secret: CLIENT_SECRET,
    });

    // Save to /var/odin/secrets/gmail-token.json
    const secretsDir = "/var/odin/secrets";
    fs.mkdirSync(secretsDir, { recursive: true, mode: 0o700 });
    const tokenPath = `${secretsDir}/gmail-token.json`;
    fs.writeFileSync(tokenPath, tokenPayload, { mode: 0o600 });

    res.writeHead(200, { "Content-Type": "text/html" });
    res.end("<h1>Success!</h1><p>Gmail OAuth token saved. You can close this tab.</p>");

    console.log(`\nToken saved to ${tokenPath}`);
    console.log("\nTo use with the Gmail plugin, set:");
    console.log(`  export ODIN_GMAIL_TOKEN='${tokenPayload}'`);
    console.log("\nDone!");
  } catch (err) {
    res.writeHead(500, { "Content-Type": "text/html" });
    res.end(`<h1>Token exchange failed</h1><p>${err.message}</p>`);
    console.error("Token exchange failed:", err.message);
  }

  server.close();
});

server.listen(PORT);
