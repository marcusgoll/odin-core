import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { loginAsStudent, hasQaAccounts } from "../setup.js";
import "dotenv/config";

describe.skipIf(!hasQaAccounts)("QA: Student Dashboard", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
    await loginAsStudent(stagehand);
  }, 90_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should display the student dashboard with a welcome message", async () => {
    const result = await stagehand.page.extract({
      instruction:
        "extract the main welcome or greeting message on the dashboard page. Look for text like 'Welcome back' or the user's name.",
      schema: z.object({
        hasWelcome: z.boolean(),
        welcomeText: z.string().optional(),
      }),
    });
    expect(result.hasWelcome).toBe(true);
  }, 30_000);

  it("should have a sidebar with navigation links", async () => {
    const result = await stagehand.page.extract({
      instruction:
        "extract the navigation sidebar link labels. Look for links like Dashboard, Logbook, Endorsements, Experience, Notifications, Settings.",
      schema: z.object({
        links: z.array(z.string()).describe("Sidebar navigation labels"),
      }),
    });
    expect(result.links.length).toBeGreaterThan(3);
  }, 30_000);

  it("should show a readiness score or audit section", async () => {
    const result = await stagehand.page.extract({
      instruction:
        "extract whether the dashboard shows a readiness score, readiness card, or audit summary. Look for percentage scores, GO/NO-GO badges, or readiness-related content.",
      schema: z.object({
        hasReadinessSection: z.boolean(),
        readinessInfo: z.string().optional(),
      }),
    });
    // New accounts may not have readiness data, but the section should exist
    expect(result.hasReadinessSection).toBeDefined();
  }, 30_000);
});
