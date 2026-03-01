import type { Stagehand } from "@browserbasehq/stagehand";

export interface ObserveInput {
  instruction?: string;
}

export async function handleObserve(
  stagehand: Stagehand,
  input: ObserveInput,
): Promise<unknown[]> {
  const page = stagehand.page;
  if (input.instruction) {
    return await page.observe(input.instruction);
  }
  return await page.observe();
}
