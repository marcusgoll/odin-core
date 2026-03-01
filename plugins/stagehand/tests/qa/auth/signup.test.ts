import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL } from "../setup.js";
import "dotenv/config";

describe("QA: Signup Flow — app.cfipros.com", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
  }, 60_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should load the registration page with email field and OAuth buttons", async () => {
    await stagehand.page.goto(`${APP_URL}/auth/register`, {
      waitUntil: "domcontentloaded",
    });
    const result = await stagehand.page.extract({
      instruction:
        "extract whether the page has: an email input field, a Continue button, a Google sign-in button, and a GitHub sign-in button",
      schema: z.object({
        hasEmailField: z.boolean(),
        hasContinueButton: z.boolean(),
        hasGoogleButton: z.boolean(),
        hasGithubButton: z.boolean(),
      }),
    });
    expect(result.hasEmailField).toBe(true);
    expect(result.hasContinueButton).toBe(true);
    expect(result.hasGoogleButton).toBe(true);
    expect(result.hasGithubButton).toBe(true);
  }, 30_000);

  it("should have a link to the login page", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether there is a link that says "Already have an account?" or links to a login page',
      schema: z.object({
        hasLoginLink: z.boolean(),
      }),
    });
    expect(result.hasLoginLink).toBe(true);
  }, 30_000);

  it("should show check-your-email screen after submitting a valid email", async () => {
    // Use a unique throwaway email to avoid conflicts
    const testEmail = `qa-signup-test-${Date.now()}@example.com`;
    await stagehand.page.goto(`${APP_URL}/auth/register`, {
      waitUntil: "domcontentloaded",
    });
    await stagehand.page.act({
      action: `Type "${testEmail}" into the email input field`,
    });
    await stagehand.page.act({ action: 'Click the "Continue" button' });

    // Wait for the confirmation screen
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page now shows a "Check your email" message or email verification confirmation',
      schema: z.object({
        showsConfirmation: z.boolean(),
        confirmationText: z.string().optional(),
      }),
    });
    expect(result.showsConfirmation).toBe(true);
  }, 45_000);
});
