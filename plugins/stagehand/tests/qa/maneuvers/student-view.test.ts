import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, loginAsStudent, hasQaAccounts } from "../setup.js";
import "dotenv/config";

const maneuverEnabled = process.env.QA_MANEUVER_UI_ENABLED === "true";

describe.skipIf(!maneuverEnabled || !hasQaAccounts)(
  "QA: Student Maneuver View (Read-Only)",
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

    it("should display graded maneuvers in read-only mode", async () => {
      const result = await stagehand.page.extract({
        instruction:
          "extract whether the page shows maneuver grades from the CFI. Look for ACS task names with grade labels (Satisfactory, Proficient, etc.) displayed as read-only.",
        schema: z.object({
          hasManeuverGrades: z.boolean(),
          isReadOnly: z.boolean(),
        }),
      });
      expect(result.hasManeuverGrades).toBe(true);
      expect(result.isReadOnly).toBe(true);
    }, 30_000);
  },
);
