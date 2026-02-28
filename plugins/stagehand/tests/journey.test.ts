import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { loadConfig } from "../src/config.js";
import "dotenv/config";

const testEmail = process.env.CFIPROS_TEST_EMAIL;
const testPassword = process.env.CFIPROS_TEST_PASSWORD;
const hasCredentials = !!(testEmail && testPassword);

describe.skipIf(!hasCredentials)(
  "Phase 3: Authenticated User Journey — app.cfipros.com",
  () => {
    let stagehand: Stagehand;
    const config = loadConfig();

    beforeAll(async () => {
      if (!hasCredentials) {
        console.warn(
          "Skipping journey tests: CFIPROS_TEST_EMAIL and CFIPROS_TEST_PASSWORD not set",
        );
        return;
      }

      stagehand = new Stagehand({
        env: "LOCAL",
        modelName: config.primaryModel,
        localBrowserLaunchOptions: {
          headless: config.headless,
          ...(config.chromePath ? { executablePath: config.chromePath } : {}),
          args: ["--no-sandbox", "--disable-setuid-sandbox"],
        },
        domSettleTimeoutMs: config.domSettleTimeout,
        selfHeal: true,
        verbose: 0,
      });
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
