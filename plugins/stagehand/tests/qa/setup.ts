import type { Stagehand } from "@browserbasehq/stagehand";
import "dotenv/config";

export const APP_URL = process.env.QA_APP_URL || "https://app.cfipros.com";
export const SITE_URL = process.env.QA_SITE_URL || "https://cfipros.com";

export const QA_STUDENT = {
  email: process.env.QA_STUDENT_EMAIL || "qa-student@cfipros.com",
  password: process.env.QA_STUDENT_PASSWORD || "",
};

export const QA_CFI = {
  email: process.env.QA_CFI_EMAIL || "qa-cfi@cfipros.com",
  password: process.env.QA_CFI_PASSWORD || "",
};

export const hasQaAccounts = !!(QA_STUDENT.password && QA_CFI.password);

async function login(
  stagehand: Stagehand,
  email: string,
  password: string,
): Promise<void> {
  await stagehand.page.goto(`${APP_URL}/auth/login`, {
    waitUntil: "domcontentloaded",
  });
  await stagehand.page.act({ action: `Type "${email}" into the Email field` });
  await stagehand.page.act({
    action: `Type "${password}" into the Password field`,
  });
  await stagehand.page.act({ action: 'Click the "Sign in" button' });
  // Wait for navigation to complete after login
  await stagehand.page.waitForLoadState("domcontentloaded");
}

export async function loginAsStudent(stagehand: Stagehand): Promise<void> {
  await login(stagehand, QA_STUDENT.email, QA_STUDENT.password);
}

export async function loginAsCfi(stagehand: Stagehand): Promise<void> {
  await login(stagehand, QA_CFI.email, QA_CFI.password);
}
