import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL } from "../setup.js";
import "dotenv/config";

describe("QA: Password Reset Flow", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
  }, 60_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should load the password reset page with email field", async () => {
    await stagehand.page.goto(`${APP_URL}/auth/reset-password`, {
      waitUntil: "domcontentloaded",
    });
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page has an email address input field and a "Send reset link" button',
      schema: z.object({
        hasEmailField: z.boolean(),
        hasSendButton: z.boolean(),
      }),
    });
    expect(result.hasEmailField).toBe(true);
    expect(result.hasSendButton).toBe(true);
  }, 30_000);

  it("should show success message after submitting an email", async () => {
    await stagehand.page.act({
      action: 'Type "test-reset@example.com" into the email address field',
    });
    await stagehand.page.act({
      action: 'Click the "Send reset link" button',
    });

    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page shows a success or confirmation message about a reset link being sent. Look for text like "reset link has been sent" or a check icon.',
      schema: z.object({
        showsSuccess: z.boolean(),
        message: z.string().optional(),
      }),
    });
    expect(result.showsSuccess).toBe(true);
  }, 45_000);

  it("should have a link back to login", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether there is a "Return to login" or "Sign in" link on this page',
      schema: z.object({
        hasLoginLink: z.boolean(),
      }),
    });
    expect(result.hasLoginLink).toBe(true);
  }, 30_000);
});
