# Odin Gmail Plugin

Gmail inbox triage assistant — auto-labels, archives, deletes spam, unsubscribes, and drafts replies.

## Setup

1. Create Google Cloud project with Gmail API enabled
2. Create OAuth2 credentials (Desktop app type)
3. Run: `odin gmail connect`
4. Deploy rules: `cp config/gmail-rules.yaml /var/odin/config/gmail-rules.yaml`
5. Deploy n8n workflow: import `odin-gmail-push.json`
6. Register Pub/Sub push subscription pointing to `https://n8n.marcusgoll.com/webhook/gmail-push`

## Configuration

Edit `/var/odin/config/gmail-rules.yaml` to customize triage rules.

## Capabilities

| Capability | Risk | Description |
|---|---|---|
| gmail.inbox.list | safe | List inbox messages |
| gmail.message.read | safe | Read message body |
| gmail.label.apply | safe | Apply/remove labels |
| gmail.thread.archive | safe | Archive (reversible) |
| gmail.draft.create | safe | Create draft |
| gmail.unsubscribe | sensitive | Unsubscribe from list |
| gmail.draft.send | sensitive | Send draft |
| gmail.message.trash | sensitive | Move to trash |
| gmail.message.delete | destructive | Permanent delete |

## Development

```bash
npm install
npm run build
npm test
```
