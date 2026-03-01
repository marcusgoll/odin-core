import type { Stagehand } from "@browserbasehq/stagehand";

export interface NavigateInput {
  url: string;
}

export interface NavigateResult {
  url: string;
  title: string;
}

export async function handleNavigate(
  stagehand: Stagehand,
  input: NavigateInput,
): Promise<NavigateResult> {
  const page = stagehand.page;
  await page.goto(input.url, { waitUntil: "domcontentloaded" });
  const title = await page.title();
  return { url: page.url(), title };
}
