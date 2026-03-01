import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, loginAsCfi, hasQaAccounts } from "../setup.js";
import "dotenv/config";

const maneuverEnabled = process.env.QA_MANEUVER_UI_ENABLED === "true";

describe.skipIf(!maneuverEnabled || !hasQaAccounts)(
  "QA: CFI Log Maneuvers",
  () => {
    let stagehand: Stagehand;

    beforeAll(async () => {
      stagehand = createTestStagehand();
      await stagehand.init();
      await loginAsCfi(stagehand);
    }, 90_000);

    afterAll(async () => {
      await stagehand.close();
    });

    it("should load the maneuver logging page for a student", async () => {
      // Navigate to a student's maneuver logging page
      await stagehand.page.goto(`${APP_URL}/cfi/students`, {
        waitUntil: "domcontentloaded",
      });
      await stagehand.page.act({
        action: "Click on the first student in the roster to view their detail page",
      });
      await stagehand.page.act({
        action: 'Click on "Log Maneuvers" or a link to log maneuvers for this student',
      });

      const result = await stagehand.page.extract({
        instruction:
          "extract whether the page shows a maneuver logging form with a flight selector and ACS task list",
        schema: z.object({
          hasFlightSelector: z.boolean(),
          hasTaskList: z.boolean(),
        }),
      });
      expect(result.hasFlightSelector).toBe(true);
      expect(result.hasTaskList).toBe(true);
    }, 60_000);

    it("should allow selecting ACS tasks and assigning grades", async () => {
      await stagehand.page.act({
        action: "Select the first available flight from the flight selector",
      });
      await stagehand.page.act({
        action:
          "Select the first ACS task from the task list (e.g., a Preflight Preparation task)",
      });

      const result = await stagehand.page.extract({
        instruction:
          'extract whether a grade picker is visible with options like "Unsatisfactory", "Needs Practice", "Satisfactory", "Proficient"',
        schema: z.object({
          hasGradePicker: z.boolean(),
          gradeOptions: z.array(z.string()),
        }),
      });
      expect(result.hasGradePicker).toBe(true);
      expect(result.gradeOptions.length).toBe(4);
    }, 45_000);

    it("should submit graded maneuvers", async () => {
      await stagehand.page.act({
        action: 'Select "Satisfactory" as the grade for the selected task',
      });
      await stagehand.page.act({
        action: 'Click the "Submit" or "Save" button to log the maneuver grades',
      });

      const result = await stagehand.page.extract({
        instruction:
          "extract whether the maneuver grades were saved successfully. Look for a success message or the grades appearing in a summary.",
        schema: z.object({
          saved: z.boolean(),
        }),
      });
      expect(result.saved).toBe(true);
    }, 45_000);
  },
);
