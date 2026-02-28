import { Stagehand } from "@browserbasehq/stagehand";
import { type StagehandPluginConfig } from "./config.js";

let instance: Stagehand | null = null;
let idleTimer: ReturnType<typeof setTimeout> | null = null;

export async function getStagehand(config: StagehandPluginConfig): Promise<Stagehand> {
  resetIdleTimer(config);

  if (instance) return instance;

  const stagehand = new Stagehand({
    env: "LOCAL",
    modelName: config.primaryModel,
    localBrowserLaunchOptions: {
      headless: config.headless,
      ...(config.chromePath ? { executablePath: config.chromePath } : {}),
    },
    domSettleTimeoutMs: config.domSettleTimeout,
    selfHeal: true,
    verbose: 0,
  });

  await stagehand.init();
  instance = stagehand;
  return stagehand;
}

export async function shutdownBrowser(): Promise<void> {
  if (idleTimer) {
    clearTimeout(idleTimer);
    idleTimer = null;
  }
  if (instance) {
    await instance.close();
    instance = null;
  }
}

function resetIdleTimer(config: StagehandPluginConfig): void {
  if (idleTimer) clearTimeout(idleTimer);
  idleTimer = setTimeout(async () => {
    await shutdownBrowser();
  }, config.idleTimeoutMs);
}
