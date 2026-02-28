import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { loadConfig } from "../src/config.js";
import "dotenv/config";

describe("Phase 2: Data Extraction — cfipros.com", () => {
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
    await stagehand.page.goto("https://cfipros.com", {
      waitUntil: "domcontentloaded",
    });
  }, 60_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should extract the hero section heading", async () => {
    const result = await stagehand.page.extract({
      instruction: "extract the main hero heading text on the page",
      schema: z.object({
        heading: z.string().describe("The main hero heading text"),
      }),
    });
    expect(result.heading).toBeTruthy();
    expect(typeof result.heading).toBe("string");
    expect(result.heading.length).toBeGreaterThan(3);
    console.log("Extracted hero heading:", result.heading);
  }, 45_000);

  it("should extract navigation links", async () => {
    const result = await stagehand.page.extract({
      instruction: "extract all navigation menu link labels",
      schema: z.object({
        links: z.array(z.string()).describe("Navigation link labels"),
      }),
    });
    expect(result.links.length).toBeGreaterThan(0);
    console.log("Extracted navigation links:", result.links);
  }, 45_000);

  it("should extract feature descriptions from the homepage", async () => {
    const result = await stagehand.page.extract({
      instruction:
        "extract the feature section: each feature title and its description",
      schema: z.object({
        features: z.array(
          z.object({
            title: z.string().describe("Feature title"),
            description: z.string().describe("Feature description"),
          }),
        ),
      }),
    });
    expect(result.features.length).toBeGreaterThan(0);
    for (const feature of result.features) {
      expect(feature.title).toBeTruthy();
      expect(feature.description).toBeTruthy();
    }
    console.log(
      "Extracted features:",
      JSON.stringify(result.features, null, 2),
    );
  }, 60_000);
});
