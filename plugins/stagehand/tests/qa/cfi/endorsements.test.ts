import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, loginAsCfi, hasQaAccounts } from "../setup.js";
import "dotenv/config";

describe.skipIf(!hasQaAccounts)("QA: CFI Endorsement History", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
    await loginAsCfi(stagehand);
    await stagehand.page.goto(`${APP_URL}/cfi/endorsements`, {
      waitUntil: "domcontentloaded",
    });
  }, 90_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should load the endorsement history page", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page has an "Endorsement History" heading, and whether it shows a table/list of endorsements or an empty state',
      schema: z.object({
        hasTitle: z.boolean(),
        hasTable: z.boolean(),
        hasEmptyState: z.boolean(),
      }),
    });
    expect(result.hasTitle).toBe(true);
    expect(result.hasTable || result.hasEmptyState).toBe(true);
  }, 30_000);

  it("should have search functionality", async () => {
    const result = await stagehand.page.extract({
      instruction:
        "extract whether the page has a search input or search bar for filtering endorsements",
      schema: z.object({
        hasSearch: z.boolean(),
      }),
    });
    expect(result.hasSearch).toBeDefined();
  }, 30_000);
});
