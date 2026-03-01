import { google, type gmail_v1 } from "googleapis";
import { OAuth2Client } from "google-auth-library";

let cachedClient: gmail_v1.Gmail | null = null;

export function getGmailClient(): gmail_v1.Gmail {
  if (cachedClient) return cachedClient;

  const token = process.env.ODIN_GMAIL_TOKEN;
  if (!token) {
    throw new Error("ODIN_GMAIL_TOKEN not set — run 'odin gmail connect' first");
  }

  let credentials: { access_token: string; refresh_token: string; client_id: string; client_secret: string };
  try {
    credentials = JSON.parse(token);
  } catch {
    throw new Error("ODIN_GMAIL_TOKEN is not valid JSON");
  }

  const oauth2 = new OAuth2Client(credentials.client_id, credentials.client_secret);
  oauth2.setCredentials({
    access_token: credentials.access_token,
    refresh_token: credentials.refresh_token,
  });

  cachedClient = google.gmail({ version: "v1", auth: oauth2 });
  return cachedClient;
}

export async function listMessages(
  client: gmail_v1.Gmail,
  query: string,
  maxResults: number = 20,
): Promise<gmail_v1.Schema$Message[]> {
  const res = await client.users.messages.list({
    userId: "me",
    q: query,
    maxResults,
  });
  return res.data.messages || [];
}

export async function getMessage(
  client: gmail_v1.Gmail,
  messageId: string,
): Promise<gmail_v1.Schema$Message> {
  const res = await client.users.messages.get({
    userId: "me",
    id: messageId,
    format: "full",
  });
  return res.data;
}

export async function applyLabel(
  client: gmail_v1.Gmail,
  messageId: string,
  labelId: string,
): Promise<void> {
  await client.users.messages.modify({
    userId: "me",
    id: messageId,
    requestBody: { addLabelIds: [labelId] },
  });
}

export async function archiveMessage(
  client: gmail_v1.Gmail,
  messageId: string,
): Promise<void> {
  await client.users.messages.modify({
    userId: "me",
    id: messageId,
    requestBody: { removeLabelIds: ["INBOX"] },
  });
}

export async function trashMessage(
  client: gmail_v1.Gmail,
  messageId: string,
): Promise<void> {
  await client.users.messages.trash({ userId: "me", id: messageId });
}

export async function createDraft(
  client: gmail_v1.Gmail,
  to: string,
  subject: string,
  body: string,
  threadId?: string,
): Promise<string> {
  const raw = Buffer.from(
    `To: ${to}\r\nSubject: ${subject}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n${body}`,
  ).toString("base64url");

  const res = await client.users.drafts.create({
    userId: "me",
    requestBody: {
      message: { raw, threadId },
    },
  });
  return res.data.id || "";
}

export async function sendDraft(
  client: gmail_v1.Gmail,
  draftId: string,
): Promise<string> {
  const res = await client.users.drafts.send({
    userId: "me",
    requestBody: { id: draftId },
  });
  return res.data.id || "";
}

export async function getLabels(
  client: gmail_v1.Gmail,
): Promise<gmail_v1.Schema$Label[]> {
  const res = await client.users.labels.list({ userId: "me" });
  return res.data.labels || [];
}

export async function ensureLabel(
  client: gmail_v1.Gmail,
  name: string,
): Promise<string> {
  const labels = await getLabels(client);
  const existing = labels.find((l) => l.name === name);
  if (existing) return existing.id!;

  const res = await client.users.labels.create({
    userId: "me",
    requestBody: { name, labelListVisibility: "labelShow", messageListVisibility: "show" },
  });
  return res.data.id!;
}

export async function getHistory(
  client: gmail_v1.Gmail,
  startHistoryId: string,
): Promise<gmail_v1.Schema$History[]> {
  const res = await client.users.history.list({
    userId: "me",
    startHistoryId,
    historyTypes: ["messageAdded"],
  });
  return res.data.history || [];
}
