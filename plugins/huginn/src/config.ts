export interface HuginnPluginConfig {
  serverUrl: string;
  authToken?: string;
  defaultBrowser: "chromium" | "firefox" | "webkit";
  headless: boolean;
  navigationTimeoutMs: number;
  allowlistedDomains: string[];
}

export function loadConfig(): HuginnPluginConfig {
  const defaultBrowser = (
    process.env.HUGINN_DEFAULT_BROWSER || "chromium"
  ).toLowerCase();

  return {
    serverUrl: process.env.HUGINN_SERVER_URL || "http://127.0.0.1:9227",
    authToken: process.env.HUGINN_AUTH_TOKEN || undefined,
    defaultBrowser:
      defaultBrowser === "firefox" || defaultBrowser === "webkit"
        ? defaultBrowser
        : "chromium",
    headless: process.env.HUGINN_HEADLESS !== "false",
    navigationTimeoutMs: parseInt(
      process.env.HUGINN_NAVIGATION_TIMEOUT_MS || "30000",
      10,
    ),
    allowlistedDomains: (process.env.HUGINN_ALLOWED_DOMAINS ||
      "cfipros.com,app.cfipros.com,localhost")
      .split(",")
      .map((d) => d.trim())
      .filter(Boolean),
  };
}

export function isDomainAllowed(url: string, config: HuginnPluginConfig): boolean {
  try {
    const hostname = new URL(url).hostname;
    return config.allowlistedDomains.some(
      (d) => hostname === d || hostname.endsWith(`.${d}`),
    );
  } catch {
    return false;
  }
}

export function normalizeObserveTarget(
  input: Record<string, unknown>,
): string | undefined {
  const rawUrl = typeof input.url === "string" ? input.url.trim() : "";
  if (rawUrl) {
    return rawUrl;
  }

  const rawDomain =
    typeof input.domain === "string" ? input.domain.trim() : "";
  if (!rawDomain) {
    return undefined;
  }

  if (
    rawDomain.startsWith("http://") ||
    rawDomain.startsWith("https://")
  ) {
    return rawDomain;
  }

  return `https://${rawDomain}`;
}
