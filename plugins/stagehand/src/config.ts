export interface StagehandPluginConfig {
  headless: boolean;
  chromePath?: string;
  primaryModel: string;
  fallbackModel: string;
  domSettleTimeout: number;
  idleTimeoutMs: number;
  allowlistedDomains: string[];
}

export function loadConfig(): StagehandPluginConfig {
  return {
    headless: process.env.STAGEHAND_HEADLESS !== "false",
    chromePath: process.env.STAGEHAND_CHROME_PATH || undefined,
    primaryModel: process.env.STAGEHAND_PRIMARY_MODEL || "anthropic/claude-sonnet-4-6",
    fallbackModel: process.env.STAGEHAND_FALLBACK_MODEL || "openai/gpt-4o-mini",
    domSettleTimeout: parseInt(process.env.STAGEHAND_DOM_SETTLE_TIMEOUT || "30000", 10),
    idleTimeoutMs: parseInt(process.env.STAGEHAND_IDLE_TIMEOUT_MS || "300000", 10),
    allowlistedDomains: (process.env.STAGEHAND_ALLOWED_DOMAINS || "cfipros.com,app.cfipros.com,localhost")
      .split(",")
      .map((d) => d.trim())
      .filter(Boolean),
  };
}

export function isDomainAllowed(url: string, config: StagehandPluginConfig): boolean {
  try {
    const hostname = new URL(url).hostname;
    return config.allowlistedDomains.some(
      (d) => hostname === d || hostname.endsWith(`.${d}`),
    );
  } catch {
    return false;
  }
}
