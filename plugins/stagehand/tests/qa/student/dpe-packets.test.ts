import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, loginAsStudent, hasQaAccounts } from "../setup.js";
import "dotenv/config";

describe.skipIf(!hasQaAccounts)("QA: Student DPE Packets", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
    await loginAsStudent(stagehand);
    await stagehand.page.goto(`${APP_URL}/dashboard/packets`, {
      waitUntil: "domcontentloaded",
    });
  }, 90_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should load the DPE packets page", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page has a "DPE Packets" heading and a "Create New Packet" button, or an empty state with a "Create Packet" prompt',
      schema: z.object({
        hasTitle: z.boolean(),
        hasCreateButton: z.boolean(),
      }),
    });
    expect(result.hasTitle).toBe(true);
    expect(result.hasCreateButton).toBe(true);
  }, 30_000);

  it("should navigate to the create packet form", async () => {
    await stagehand.page.act({
      action:
        'Click the "Create New Packet" or "Create Packet" button',
    });
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page now shows a "Create DPE Packet" form with fields for Audit Report ID and Certificate Type',
      schema: z.object({
        hasCreateForm: z.boolean(),
        hasAuditIdField: z.boolean(),
        hasCertTypeField: z.boolean(),
      }),
    });
    expect(result.hasCreateForm).toBe(true);
  }, 30_000);

  it("should show certificate type options", async () => {
    const result = await stagehand.page.extract({
      instruction:
        "extract the available certificate type options from the dropdown. Look for Private Pilot, Instrument Rating, Commercial Pilot, etc.",
      schema: z.object({
        options: z.array(z.string()).describe("Certificate type options"),
      }),
    });
    expect(result.options.length).toBeGreaterThan(0);
  }, 30_000);
});
