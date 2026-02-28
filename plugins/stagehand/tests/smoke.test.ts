import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { loadConfig } from "../src/config.js";
import "dotenv/config";

describe("Phase 1: Smoke Tests — cfipros.com", () => {
  let stagehand: Stagehand;
  const config = loadConfig();

  beforeAll(async () => {
    stagehand = new Stagehand({
      env: "LOCAL",
      modelName: config.primaryModel,
      localBrowserLaunchOptions: {
        headless: config.headless,
        ...(config.chromePath ? { executablePath: config.chromePath } : {}),
        args: ["--no-sandbox", "--disable-setuid-sandbox"],
      },
      domSettleTimeoutMs: config.domSettleTimeout,
      selfHeal: true,
      verbose: 0,
    });
    await stagehand.init();
  }, 60_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should navigate to cfipros.com and get a title", async () => {
    await stagehand.page.goto("https://cfipros.com", {
      waitUntil: "domcontentloaded",
    });
    const title = await stagehand.page.title();
    expect(title).toBeTruthy();
    expect(typeof title).toBe("string");
  }, 30_000);

  it("should observe available actions on the homepage", async () => {
    const actions = await stagehand.page.observe(
      "find all navigation links on the page",
    );
    expect(Array.isArray(actions)).toBe(true);
    expect(actions.length).toBeGreaterThan(0);
  }, 30_000);

  it("should extract the page heading via AI", async () => {
    const result = await stagehand.page.extract(
      "extract the main heading text and the meta description of the page",
    );
    expect(result).toBeTruthy();
    expect(result.extraction).toBeTruthy();
    expect(typeof result.extraction).toBe("string");
  }, 30_000);

  it("should perform an action on the page", async () => {
    const result = await stagehand.page.act(
      "scroll down to the bottom of the page",
    );
    expect(result.success).toBe(true);
  }, 30_000);
});
