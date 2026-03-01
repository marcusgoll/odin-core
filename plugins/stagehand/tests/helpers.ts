import { Stagehand } from "@browserbasehq/stagehand";
import { loadConfig } from "../src/config.js";

export function createTestStagehand(): Stagehand {
  const config = loadConfig();
  return new Stagehand({
    env: "LOCAL",
    modelName: config.primaryModel,
    localBrowserLaunchOptions: {
      headless: config.headless,
      ...(config.chromePath ? { executablePath: config.chromePath } : {}),
      args: ["--no-sandbox", "--disable-setuid-sandbox"],
    },
    domSettleTimeoutMs: config.domSettleTimeout,
    selfHeal: true,
    verbose: 0,
  });
}
