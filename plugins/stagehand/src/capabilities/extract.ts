import type { Stagehand } from "@browserbasehq/stagehand";
import { z } from "zod";

export interface ExtractInput {
  instruction: string;
  schema?: Record<string, unknown>;
  selector?: string;
}

export async function handleExtract(
  stagehand: Stagehand,
  input: ExtractInput,
): Promise<unknown> {
  const page = stagehand.page;

  if (input.schema) {
    const zodSchema = buildZodSchema(input.schema);
    return await page.extract({
      instruction: input.instruction,
      schema: zodSchema,
      ...(input.selector ? { selector: input.selector } : {}),
    });
  }

  return await page.extract(input.instruction);
}

function buildZodSchema(
  descriptor: Record<string, unknown>,
): z.ZodObject<Record<string, z.ZodTypeAny>> {
  const shape: Record<string, z.ZodTypeAny> = {};
  for (const [key, type] of Object.entries(descriptor)) {
    switch (type) {
      case "string":
        shape[key] = z.string();
        break;
      case "number":
        shape[key] = z.number();
        break;
      case "boolean":
        shape[key] = z.boolean();
        break;
      case "array":
        shape[key] = z.array(z.string());
        break;
      default:
        shape[key] = z.string();
    }
  }
  return z.object(shape);
}
