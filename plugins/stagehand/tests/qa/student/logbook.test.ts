import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";
import { createTestStagehand } from "../../helpers.js";
import { APP_URL, loginAsStudent, hasQaAccounts } from "../setup.js";
import "dotenv/config";

describe.skipIf(!hasQaAccounts)("QA: Student Logbook", () => {
  let stagehand: Stagehand;

  beforeAll(async () => {
    stagehand = createTestStagehand();
    await stagehand.init();
    await loginAsStudent(stagehand);
    await stagehand.page.goto(`${APP_URL}/logbook`, {
      waitUntil: "domcontentloaded",
    });
  }, 90_000);

  afterAll(async () => {
    await stagehand.close();
  });

  it("should load the logbook page with title and controls", async () => {
    const result = await stagehand.page.extract({
      instruction:
        'extract whether the page has: a "Logbook" heading, filter/search controls, and an Add button or floating action button for adding flights',
      schema: z.object({
        hasLogbookTitle: z.boolean(),
        hasFilters: z.boolean(),
        hasAddButton: z.boolean(),
      }),
    });
    expect(result.hasLogbookTitle).toBe(true);
    expect(result.hasAddButton).toBe(true);
  }, 30_000);

  it("should open the add flight form", async () => {
    await stagehand.page.act({
      action:
        'Click the add flight button or the "+" floating action button to open the add flight form',
    });
    const result = await stagehand.page.extract({
      instruction:
        'extract whether an "Add Flight" form or sheet is now visible. Look for fields like Date, Aircraft Type, Total Hours.',
      schema: z.object({
        hasAddFlightForm: z.boolean(),
        visibleFields: z
          .array(z.string())
          .describe("Field labels visible in the form"),
      }),
    });
    expect(result.hasAddFlightForm).toBe(true);
  }, 30_000);

  it("should fill and submit a flight entry", async () => {
    // Fill minimum required fields
    await stagehand.page.act({
      action: "Click the date picker and select today's date",
    });
    await stagehand.page.act({
      action: 'Type "C172" into the Aircraft Type field',
    });
    await stagehand.page.act({
      action: 'Type "1.5" into the Total Hours field',
    });
    await stagehand.page.act({
      action: 'Click the "Save Flight" button',
    });

    // Verify the flight appears in the list
    const result = await stagehand.page.extract({
      instruction:
        "extract whether the flight was saved successfully. Look for the flight entry in the list with C172 and 1.5 hours, or a success message.",
      schema: z.object({
        flightSaved: z.boolean(),
      }),
    });
    expect(result.flightSaved).toBe(true);
  }, 60_000);

  it("should show the flight in the logbook list", async () => {
    const result = await stagehand.page.extract({
      instruction:
        "extract the first flight entry visible in the logbook list. Include aircraft type and total hours if visible.",
      schema: z.object({
        hasFlights: z.boolean(),
        firstFlight: z
          .object({
            aircraftType: z.string().optional(),
            totalHours: z.string().optional(),
          })
          .optional(),
      }),
    });
    expect(result.hasFlights).toBe(true);
  }, 30_000);
});
