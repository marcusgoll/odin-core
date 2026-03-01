import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, loginAsStudent, hasQaAccounts } from "../setup.js";
import "dotenv/config";

describe.skipIf(!hasQaAccounts)("QA: Student Endorsements", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
    await loginAsStudent(stagehand);
    await stagehand.page.goto(`${APP_URL}/endorsements`, {
      waitUntil: "domcontentloaded",
    });
  }, 90_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should load the endorsements page", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page has an "Endorsements" heading and shows either endorsement cards/items or an empty state message',
      schema: z.object({
        hasTitle: z.boolean(),
        hasContent: z.boolean(),
      }),
    });
    expect(result.hasTitle).toBe(true);
  }, 30_000);

  it("should display endorsement status indicators or empty state", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page shows endorsement status indicators (Valid, Expired, Missing) or an empty state. Look for colored badges, check marks, warning icons, or text like "No endorsements".',
      schema: z.object({
        hasStatusIndicators: z.boolean(),
        hasEmptyState: z.boolean(),
      }),
    });
    expect(result.hasStatusIndicators || result.hasEmptyState).toBe(true);
  }, 30_000);
});
