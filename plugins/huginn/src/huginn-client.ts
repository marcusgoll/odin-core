import type { HuginnPluginConfig } from "./config.js";

type HuginnHealth = {
  ok: boolean;
  engine: string;
  browser: boolean;
  page: boolean;
  url: string | null;
};

type HuginnSnapshot = {
  snapshot: string;
  refs: Record<string, string>;
  stats: {
    totalRefs: number;
    interactiveRefs: number;
  };
};

type HuginnLaunchResponse = {
  ok: boolean;
  url?: string;
};

export type HuginnObservation = HuginnSnapshot & {
  engine: string;
  browser: boolean;
  page: boolean;
  url: string | null;
};

export class HuginnClient {
  constructor(private readonly config: HuginnPluginConfig) {}

  async observe(targetUrl?: string): Promise<HuginnObservation> {
    const health = await this.health();

    if (!health.browser) {
      await this.launch(targetUrl);
    } else if (targetUrl) {
      await this.navigate(targetUrl);
    } else if (!health.page) {
      throw new Error("No active Huginn page and no target url was provided");
    }

    const current = await this.health();
    const snapshot = await this.snapshot();
    return {
      ...snapshot,
      engine: current.engine,
      browser: current.browser,
      page: current.page,
      url: current.url,
    };
  }

  async health(): Promise<HuginnHealth> {
    return this.request<HuginnHealth>("GET", "/health");
  }

  async launch(targetUrl?: string): Promise<HuginnLaunchResponse> {
    return this.request<HuginnLaunchResponse>("POST", "/launch", {
      browser: this.config.defaultBrowser,
      headless: this.config.headless,
      ...(targetUrl ? { url: targetUrl } : {}),
    });
  }

  async navigate(targetUrl: string): Promise<{ ok: boolean; url: string }> {
    return this.request<{ ok: boolean; url: string }>("POST", "/navigate", {
      url: targetUrl,
      timeout_ms: this.config.navigationTimeoutMs,
    });
  }

  async snapshot(): Promise<HuginnSnapshot> {
    return this.request<HuginnSnapshot>("GET", "/snapshot?compact=1");
  }

  private async request<T>(
    method: "GET" | "POST",
    path: string,
    body?: unknown,
  ): Promise<T> {
    const url = new URL(path, this.config.serverUrl);
    const headers = new Headers({
      Accept: "application/json",
    });
    if (this.config.authToken) {
      headers.set("Authorization", `Bearer ${this.config.authToken}`);
    }
    if (body !== undefined) {
      headers.set("Content-Type", "application/json");
    }

    const response = await fetch(url, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    });

    const raw = await response.text();
    const payload =
      raw.length > 0 ? (JSON.parse(raw) as Record<string, unknown>) : {};

    if (!response.ok) {
      const message =
        typeof payload.error === "string"
          ? payload.error
          : `Huginn request failed with ${response.status}`;
      throw new Error(message);
    }

    return payload as T;
  }
}
