import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { createServer } from "node:http";

import { HuginnClient } from "../src/huginn-client.js";
import type { HuginnPluginConfig } from "../src/config.js";

let serverUrl = "";
let sawAuthHeader = false;
let browserOpen = false;
let activeUrl: string | null = null;
let server: ReturnType<typeof createServer>;

const config = (): HuginnPluginConfig => ({
  serverUrl,
  authToken: "secret-token",
  defaultBrowser: "chromium",
  headless: true,
  navigationTimeoutMs: 30_000,
  allowlistedDomains: ["cfipros.com"],
});

beforeAll(async () => {
  server = createServer(async (req, res) => {
    sawAuthHeader = req.headers.authorization === "Bearer secret-token";

    const chunks: Buffer[] = [];
    for await (const chunk of req) {
      chunks.push(Buffer.from(chunk));
    }
    const body =
      chunks.length > 0
        ? (JSON.parse(Buffer.concat(chunks).toString("utf8")) as Record<
            string,
            unknown
          >)
        : {};

    if (req.url === "/health" && req.method === "GET") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify({
          ok: true,
          engine: "huginn-playwright",
          browser: browserOpen,
          page: browserOpen,
          url: activeUrl,
        }),
      );
      return;
    }

    if (req.url === "/launch" && req.method === "POST") {
      browserOpen = true;
      activeUrl = typeof body.url === "string" ? body.url : "about:blank";
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: true, url: activeUrl }));
      return;
    }

    if (req.url === "/navigate" && req.method === "POST") {
      activeUrl = typeof body.url === "string" ? body.url : activeUrl;
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: true, url: activeUrl }));
      return;
    }

    if (req.url === "/snapshot?compact=1" && req.method === "GET") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify({
          snapshot: "[ref=e1] link \"Pricing\"",
          refs: { e1: '[ref=e1] link "Pricing"' },
          stats: { totalRefs: 1, interactiveRefs: 1 },
        }),
      );
      return;
    }

    res.writeHead(404, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ error: "not found" }));
  });

  await new Promise<void>((resolve) => {
    server.listen(0, "127.0.0.1", () => resolve());
  });

  const address = server.address();
  if (!address || typeof address === "string") {
    throw new Error("expected tcp server address");
  }

  serverUrl = `http://127.0.0.1:${address.port}`;
});

afterAll(async () => {
  await new Promise<void>((resolve, reject) => {
    server.close((err) => {
      if (err) {
        reject(err);
        return;
      }
      resolve();
    });
  });
});

describe("HuginnClient timeout", () => {
  let slowServerUrl = "";
  let slowServer: ReturnType<typeof createServer>;

  beforeAll(async () => {
    slowServer = createServer((_req, _res) => {
      // Intentionally never respond to simulate a hung Huginn server
    });
    await new Promise<void>((resolve) => {
      slowServer.listen(0, "127.0.0.1", () => resolve());
    });
    const addr = slowServer.address();
    if (!addr || typeof addr === "string") throw new Error("expected tcp address");
    slowServerUrl = `http://127.0.0.1:${addr.port}`;
  });

  afterAll(async () => {
    await new Promise<void>((resolve, reject) => {
      slowServer.close((err) => (err ? reject(err) : resolve()));
    });
  });

  it("aborts the request when the server does not respond within navigationTimeoutMs", async () => {
    const client = new HuginnClient({
      serverUrl: slowServerUrl,
      authToken: undefined,
      defaultBrowser: "chromium",
      headless: true,
      navigationTimeoutMs: 50,
      allowlistedDomains: [],
    });

    await expect(client.health()).rejects.toThrow();
  }, 5_000);
});

describe("HuginnClient", () => {
  it("launches the browser when none is active", async () => {
    browserOpen = false;
    activeUrl = null;
    const client = new HuginnClient(config());

    const observation = await client.observe("https://cfipros.com");

    expect(sawAuthHeader).toBe(true);
    expect(observation.url).toBe("https://cfipros.com");
    expect(observation.snapshot).toContain('Pricing');
  });

  it("reuses the active browser and navigates to a new url", async () => {
    browserOpen = true;
    activeUrl = "https://cfipros.com";
    const client = new HuginnClient(config());

    const observation = await client.observe("https://app.cfipros.com");

    expect(observation.url).toBe("https://app.cfipros.com");
  });
});
