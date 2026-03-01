import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, loginAsStudent, hasQaAccounts } from "../setup.js";
import "dotenv/config";

const maneuverEnabled = process.env.QA_MANEUVER_UI_ENABLED === "true";

describe.skipIf(!maneuverEnabled || !hasQaAccounts)(
  "QA: Maneuver Progress Charts",
  () => {
    let stagehand: Stagehand;

    beforeAll(async () => {
      stagehand = createTestStagehand();
      await stagehand.init();
      await loginAsStudent(stagehand);
      await stagehand.page.goto(`${APP_URL}/dashboard/maneuvers`, {
        waitUntil: "domcontentloaded",
      });
    }, 90_000);

    afterAll(async () => {
      await stagehand.close();
    });

    it("should display progress charts or coverage percentage", async () => {
      const result = await stagehand.page.extract({
        instruction:
          "extract whether the page shows maneuver progress charts, coverage percentages, or trend visualizations. Look for chart elements, progress bars, or percentage indicators.",
        schema: z.object({
          hasProgressDisplay: z.boolean(),
          coveragePercent: z.string().optional(),
        }),
      });
      expect(result.hasProgressDisplay).toBe(true);
    }, 30_000);
  },
);
