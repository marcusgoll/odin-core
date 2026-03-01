import type { Stagehand } from "@browserbasehq/stagehand";

export interface ActInput {
  instruction: string;
  variables?: Record<string, string>;
}

export interface ActResult {
  success: boolean;
  message: string;
  action: string;
}

export async function handleAct(
  stagehand: Stagehand,
  input: ActInput,
): Promise<ActResult> {
  const page = stagehand.page;
  const result = await page.act({
    action: input.instruction,
    ...(input.variables ? { variables: input.variables } : {}),
  });
  return { success: result.success, message: result.message, action: result.action };
}
