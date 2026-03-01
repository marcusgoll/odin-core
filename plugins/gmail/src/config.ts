export interface PluginConfig {
  account: string;
  rulesPath: string;
  stateDir: string;
}

export function loadConfig(): PluginConfig {
  return {
    account: process.env.GMAIL_ACCOUNT || "personal",
    rulesPath: process.env.GMAIL_RULES_PATH || "/var/odin/config/gmail-rules.yaml",
    stateDir: process.env.GMAIL_STATE_DIR || "/var/odin/state/gmail",
  };
}
