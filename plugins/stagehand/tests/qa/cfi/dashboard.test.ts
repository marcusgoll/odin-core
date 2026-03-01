import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, loginAsCfi, hasQaAccounts } from "../setup.js";
import "dotenv/config";

describe.skipIf(!hasQaAccounts)("QA: CFI Dashboard", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
    await loginAsCfi(stagehand);
    await stagehand.page.goto(`${APP_URL}/cfi/dashboard`, {
      waitUntil: "domcontentloaded",
    });
  }, 90_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should load the CFI dashboard", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page is a CFI dashboard. Look for "CFI Dashboard" in the title, headings, or metadata, or instructor-specific content.',
      schema: z.object({
        isCfiDashboard: z.boolean(),
      }),
    });
    expect(result.isCfiDashboard).toBe(true);
  }, 30_000);

  it("should have CFI-specific navigation (Students, Endorsements)", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract the sidebar or navigation links visible on the page. Look for CFI-specific links like "Students", "CFI Dashboard", "Endorsements".',
      schema: z.object({
        links: z.array(z.string()),
        hasStudentsLink: z.boolean(),
      }),
    });
    expect(result.hasStudentsLink).toBe(true);
  }, 30_000);

  it("should show student roster overview or empty state", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the dashboard shows a student roster, student cards, pending requests, or an empty state message like "No students".',
      schema: z.object({
        hasStudentInfo: z.boolean(),
        hasEmptyState: z.boolean(),
      }),
    });
    expect(result.hasStudentInfo || result.hasEmptyState).toBe(true);
  }, 30_000);
});
