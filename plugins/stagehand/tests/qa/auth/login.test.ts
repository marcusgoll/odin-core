import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, QA_STUDENT, QA_CFI, hasQaAccounts } from "../setup.js";
import "dotenv/config";

describe("QA: Login Page UI", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
  }, 60_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should load the login page with email, password fields and sign-in button", async () => {
    await stagehand.page.goto(`${APP_URL}/auth/login`, {
      waitUntil: "domcontentloaded",
    });
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page has: an Email input, a Password input, a "Sign in" button, and a "Forgot password?" link',
      schema: z.object({
        hasEmailField: z.boolean(),
        hasPasswordField: z.boolean(),
        hasSignInButton: z.boolean(),
        hasForgotPasswordLink: z.boolean(),
      }),
    });
    expect(result.hasEmailField).toBe(true);
    expect(result.hasPasswordField).toBe(true);
    expect(result.hasSignInButton).toBe(true);
    expect(result.hasForgotPasswordLink).toBe(true);
  }, 30_000);
});

describe.skipIf(!hasQaAccounts)("QA: Student Login Flow", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
  }, 60_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should log in as student and reach the dashboard", async () => {
    await stagehand.page.goto(`${APP_URL}/auth/login`, {
      waitUntil: "domcontentloaded",
    });
    await stagehand.page.act({
      action: `Type "${QA_STUDENT.email}" into the Email field`,
    });
    await stagehand.page.act({
      action: `Type "${QA_STUDENT.password}" into the Password field`,
    });
    await stagehand.page.act({ action: 'Click the "Sign in" button' });

    // Verify we landed on a dashboard page
    const result = await stagehand.page.extract({
      instruction:
        "extract whether the current page is a dashboard. Look for welcome messages, navigation sidebar, or dashboard-specific content. Also extract the current page URL.",
      schema: z.object({
        isDashboard: z.boolean(),
        pageUrl: z.string(),
      }),
    });
    expect(result.isDashboard).toBe(true);
  }, 45_000);
});

describe.skipIf(!hasQaAccounts)("QA: CFI Login Flow", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
  }, 60_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should log in as CFI and reach the dashboard", async () => {
    await stagehand.page.goto(`${APP_URL}/auth/login`, {
      waitUntil: "domcontentloaded",
    });
    await stagehand.page.act({
      action: `Type "${QA_CFI.email}" into the Email field`,
    });
    await stagehand.page.act({
      action: `Type "${QA_CFI.password}" into the Password field`,
    });
    await stagehand.page.act({ action: 'Click the "Sign in" button' });

    const result = await stagehand.page.extract({
      instruction:
        "extract whether the current page is a dashboard or main app page. Look for navigation, sidebar, or dashboard content.",
      schema: z.object({
        isDashboard: z.boolean(),
        pageUrl: z.string(),
      }),
    });
    expect(result.isDashboard).toBe(true);
  }, 45_000);
});
