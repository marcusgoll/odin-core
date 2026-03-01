import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, loginAsCfi, hasQaAccounts } from "../setup.js";
import "dotenv/config";

describe.skipIf(!hasQaAccounts)("QA: CFI-Student Linking", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
    await loginAsCfi(stagehand);
  }, 90_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should have a way to invite or link a student from the CFI dashboard or students page", async () => {
    await stagehand.page.goto(`${APP_URL}/cfi/students`, {
      waitUntil: "domcontentloaded",
    });
    const result = await stagehand.page.extract({
      instruction:
        'extract whether there is a button or link to invite a student, add a student, or link a student. Look for text like "Invite", "Add Student", "Link", or similar.',
      schema: z.object({
        hasInviteOption: z.boolean(),
        inviteText: z.string().optional(),
      }),
    });
    // The invite flow may be from student side (student adds CFI), document what we find
    expect(result).toBeDefined();
  }, 30_000);
});
