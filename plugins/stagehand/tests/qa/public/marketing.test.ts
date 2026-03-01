import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { SITE_URL } from "../setup.js";
import "dotenv/config";

describe("QA: Marketing Site — cfipros.com", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
    await stagehand.page.goto(SITE_URL, { waitUntil: "domcontentloaded" });
  }, 60_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should load the homepage with a heading", async () => {
    const result = await stagehand.page.extract({
      instruction: "extract the main hero heading text on the page",
      schema: z.object({
        heading: z.string().describe("The main hero heading"),
      }),
    });
    expect(result.heading).toBeTruthy();
    expect(result.heading.length).toBeGreaterThan(3);
  }, 30_000);

  it("should have navigation links", async () => {
    const result = await stagehand.page.extract({
      instruction: "extract all top-level navigation menu link labels",
      schema: z.object({
        links: z.array(z.string()).describe("Navigation link labels"),
      }),
    });
    expect(result.links.length).toBeGreaterThan(0);
  }, 30_000);

  it("should have a pricing or features section", async () => {
    const result = await stagehand.page.extract({
      instruction:
        "extract whether the page has a pricing section or features section. Return the section title if found.",
      schema: z.object({
        hasPricing: z.boolean().describe("Whether a pricing section exists"),
        hasFeatures: z.boolean().describe("Whether a features section exists"),
        sectionTitle: z.string().optional().describe("The section title found"),
      }),
    });
    expect(result.hasPricing || result.hasFeatures).toBe(true);
  }, 30_000);

  it("should have CTA buttons linking to signup or the app", async () => {
    const actions = await stagehand.page.observe(
      "find all call-to-action buttons or links that say things like Sign Up, Get Started, Try Free, or similar",
    );
    expect(actions.length).toBeGreaterThan(0);
  }, 30_000);
});
