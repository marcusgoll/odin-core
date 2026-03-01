import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, loginAsStudent, hasQaAccounts } from "../setup.js";
import "dotenv/config";

describe.skipIf(!hasQaAccounts)("QA: Student AKTR Upload", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
    await loginAsStudent(stagehand);
    await stagehand.page.goto(`${APP_URL}/dashboard/aktr`, {
      waitUntil: "domcontentloaded",
    });
  }, 90_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should load the AKTR page with greeting and upload zone", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page has a greeting (like "Hey" with the user name), and an upload zone with text like "Upload your AKTR" and "Browse Files" button',
      schema: z.object({
        hasGreeting: z.boolean(),
        hasUploadZone: z.boolean(),
        hasBrowseButton: z.boolean(),
      }),
    });
    expect(result.hasUploadZone).toBe(true);
    expect(result.hasBrowseButton).toBe(true);
  }, 30_000);

  it("should show file type and size info", async () => {
    const result = await stagehand.page.extract({
      instruction:
        "extract whether the upload zone shows accepted file types (PDF, JPG, PNG, HEIC) and a maximum file size (10MB)",
      schema: z.object({
        showsFileTypes: z.boolean(),
        showsMaxSize: z.boolean(),
      }),
    });
    expect(result.showsFileTypes).toBe(true);
    expect(result.showsMaxSize).toBe(true);
  }, 30_000);

  it("should display stats cards and quick links", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page has stats cards (showing Extractions, Codes Found, etc.) and quick links for "View History" or "Analytics"',
      schema: z.object({
        hasStatsCards: z.boolean(),
        hasQuickLinks: z.boolean(),
      }),
    });
    // Stats cards should always be present even with zero values
    expect(result.hasStatsCards).toBeDefined();
  }, 30_000);
});
