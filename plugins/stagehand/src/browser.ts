import { Stagehand } from "@browserbasehq/stagehand";
import { type StagehandPluginConfig } from "./config.js";

let instance: Stagehand | null = null;
let idleTimer: ReturnType<typeof setTimeout> | null = null;
let closing = false;

export async function getStagehand(config: StagehandPluginConfig): Promise<Stagehand> {
  if (instance) {
    resetIdleTimer(config);
    return instance;
  }

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
  resetIdleTimer(config);
  return stagehand;
}

export async function shutdownBrowser(): Promise<void> {
  if (closing) return;
  closing = true;
  if (idleTimer) {
    clearTimeout(idleTimer);
    idleTimer = null;
  }
  if (instance) {
    await instance.close();
    instance = null;
  }
  closing = false;
}

function resetIdleTimer(config: StagehandPluginConfig): void {
  if (idleTimer) clearTimeout(idleTimer);
  idleTimer = setTimeout(async () => {
    await shutdownBrowser();
  }, config.idleTimeoutMs);
}
