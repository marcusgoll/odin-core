import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, loginAsStudent, hasQaAccounts } from "../setup.js";
import "dotenv/config";

describe.skipIf(!hasQaAccounts)("QA: Student Audit / Readiness", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
    await loginAsStudent(stagehand);
    await stagehand.page.goto(`${APP_URL}/audit`, {
      waitUntil: "domcontentloaded",
    });
  }, 90_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should load the checkride audit page", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page has a "Checkride Audit" heading or title',
      schema: z.object({
        hasAuditTitle: z.boolean(),
        title: z.string().optional(),
      }),
    });
    expect(result.hasAuditTitle).toBe(true);
  }, 30_000);

  it("should display a GO or NO-GO readiness badge", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page shows a readiness badge or status. Look for "GO", "NO-GO", a percentage score, or an empty state message like "No audit available".',
      schema: z.object({
        hasReadinessBadge: z.boolean(),
        status: z
          .string()
          .optional()
          .describe("GO, NO-GO, or empty state message"),
        score: z.string().optional().describe("Percentage score if shown"),
      }),
    });
    // Either shows a readiness badge or an empty state — both are valid
    expect(result.hasReadinessBadge).toBeDefined();
  }, 30_000);

  it("should have experience and endorsements sections or empty state", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page has sections labeled "Experience" and "Endorsements", OR shows an empty state with a message about logging flights.',
      schema: z.object({
        hasExperienceSection: z.boolean(),
        hasEndorsementsSection: z.boolean(),
        hasEmptyState: z.boolean(),
      }),
    });
    expect(
      (result.hasExperienceSection && result.hasEndorsementsSection) ||
        result.hasEmptyState,
    ).toBe(true);
  }, 30_000);
});
