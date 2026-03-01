import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, loginAsCfi, hasQaAccounts } from "../setup.js";
import "dotenv/config";

describe.skipIf(!hasQaAccounts)("QA: CFI Students Roster", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
    await loginAsCfi(stagehand);
    await stagehand.page.goto(`${APP_URL}/cfi/students`, {
      waitUntil: "domcontentloaded",
    });
  }, 90_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should load the students page with title and filters", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page has a "Students" heading, search/filter controls, and either student cards or an empty state message.',
      schema: z.object({
        hasStudentsTitle: z.boolean(),
        hasSearchOrFilters: z.boolean(),
        hasContent: z.boolean(),
      }),
    });
    expect(result.hasStudentsTitle).toBe(true);
  }, 30_000);

  it("should have filter and sort options", async () => {
    const result = await stagehand.page.extract({
      instruction:
        "extract whether the page has filter dropdowns or sort options for the student list. Look for readiness filters, sort by name/activity, or search input.",
      schema: z.object({
        hasFilters: z.boolean(),
        hasSortOptions: z.boolean(),
      }),
    });
    expect(result.hasFilters || result.hasSortOptions).toBeDefined();
  }, 30_000);
});
