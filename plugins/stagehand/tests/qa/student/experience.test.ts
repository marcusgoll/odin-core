import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, loginAsStudent, hasQaAccounts } from "../setup.js";
import "dotenv/config";

describe.skipIf(!hasQaAccounts)("QA: Student Experience Tracking", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
    await loginAsStudent(stagehand);
    await stagehand.page.goto(`${APP_URL}/dashboard/experience`, {
      waitUntil: "domcontentloaded",
    });
  }, 90_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should load the experience tracking page", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page has an "Experience Tracking" heading or title, and whether it mentions Part 61 requirements',
      schema: z.object({
        hasTitle: z.boolean(),
        mentionsPart61: z.boolean(),
      }),
    });
    expect(result.hasTitle).toBe(true);
  }, 30_000);

  it("should show flight hour categories or empty state", async () => {
    const result = await stagehand.page.extract({
      instruction:
        "extract whether the page shows flight hour categories (like Total Time, Cross Country, Night, Instrument) with progress bars or hour counts, OR shows an empty state.",
      schema: z.object({
        hasHourCategories: z.boolean(),
        hasEmptyState: z.boolean(),
        categories: z.array(z.string()).optional(),
      }),
    });
    expect(result.hasHourCategories || result.hasEmptyState).toBe(true);
  }, 30_000);
});
