import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { createTestStagehand } from "./helpers.js";
import "dotenv/config";

const testEmail = process.env.CFIPROS_TEST_EMAIL;
const testPassword = process.env.CFIPROS_TEST_PASSWORD;
const hasCredentials = !!(testEmail && testPassword);

describe.skipIf(!hasCredentials)(
  "Phase 3: Authenticated User Journey — app.cfipros.com",
  () => {
    let stagehand: Stagehand;

    beforeAll(async () => {
      stagehand = createTestStagehand();
      await stagehand.init();
    }, 30_000);

    afterAll(async () => {
      if (stagehand) await stagehand.close();
    });

    it("should complete login flow with agent", async () => {
      const agent = stagehand.agent({
        provider: "openai",
        model: "gpt-4o",
      });
      // NOTE: Credentials are interpolated into the instruction string because
      // Stagehand v2's agent.execute() API does not support a variables parameter.
      // Only use dedicated test accounts with limited privileges for these tests.
      const result = await agent.execute({
        instruction: `Go to https://app.cfipros.com/login, enter the email ${testEmail} and password ${testPassword}, then click the login/sign in button. Wait for the dashboard to load.`,
        maxSteps: 15,
      });

      expect(result.completed).toBe(true);
    }, 120_000);

    it("should navigate the dashboard after login", async () => {
      const agent = stagehand.agent({
        provider: "openai",
        model: "gpt-4o",
      });
      const result = await agent.execute({
        instruction:
          "From the dashboard, find and click on any student record or the students section. Report what you see.",
        maxSteps: 10,
      });

      expect(result.completed).toBe(true);
    }, 120_000);
  },
);
