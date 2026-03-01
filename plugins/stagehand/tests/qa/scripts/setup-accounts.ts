import pg from "pg";
import argon2 from "argon2";
import "dotenv/config";

const DATABASE_URL = process.env.QA_DATABASE_URL;
if (!DATABASE_URL) {
  console.error("QA_DATABASE_URL is required in .env");
  process.exit(1);
}

interface QaAccount {
  email: string;
  password: string;
  fullName: string;
  roleName: string;
}

const accounts: QaAccount[] = [
  {
    email: process.env.QA_STUDENT_EMAIL || "qa-student@cfipros.com",
    password: process.env.QA_STUDENT_PASSWORD || "QaTest2026!Secure",
    fullName: "QA Student",
    roleName: "student",
  },
  {
    email: process.env.QA_CFI_EMAIL || "qa-cfi@cfipros.com",
    password: process.env.QA_CFI_PASSWORD || "QaTest2026!CfiAcct",
    fullName: "QA Instructor",
    roleName: "cfi",
  },
];

async function setupAccount(
  client: pg.Client,
  account: QaAccount,
): Promise<void> {
  const hash = await argon2.hash(account.password, { type: argon2.argon2id });

  // Get role ID
  const roleResult = await client.query(
    `SELECT id FROM role WHERE name = $1 LIMIT 1`,
    [account.roleName],
  );
  if (roleResult.rows.length === 0) {
    console.error(`Role "${account.roleName}" not found in database`);
    return;
  }
  const roleId = roleResult.rows[0].id;

  // Check if user exists
  const existing = await client.query(
    `SELECT id FROM "user" WHERE email = $1`,
    [account.email],
  );

  if (existing.rows.length > 0) {
    // Update existing user
    await client.query(
      `UPDATE "user" SET
        email_verified = true,
        email_verified_at = COALESCE(email_verified_at, NOW()),
        password_hash = $1,
        full_name = $2,
        primary_role_id = $3,
        onboarding_completed_at = COALESCE(onboarding_completed_at, NOW()),
        onboarding_step = 'completed'
      WHERE email = $4`,
      [hash, account.fullName, roleId, account.email],
    );
    console.log(`Updated: ${account.email} (${account.roleName})`);
  } else {
    // Insert new user
    await client.query(
      `INSERT INTO "user" (
        id, email, email_verified, email_verified_at,
        password_hash, full_name, primary_role_id,
        onboarding_completed_at, onboarding_step,
        subscription_tier, failed_login_count, breach_nag_count,
        created_at, updated_at
      ) VALUES (
        gen_random_uuid(), $1, true, NOW(),
        $2, $3, $4,
        NOW(), 'completed',
        'free', 0, 0,
        NOW(), NOW()
      )`,
      [account.email, hash, account.fullName, roleId],
    );
    console.log(`Created: ${account.email} (${account.roleName})`);
  }
}

async function main(): Promise<void> {
  const client = new pg.Client({ connectionString: DATABASE_URL });
  await client.connect();
  console.log("Connected to database");

  try {
    for (const account of accounts) {
      await setupAccount(client, account);
    }
    console.log("\nQA accounts ready. You can now run: npm run test:qa");
  } finally {
    await client.end();
  }
}

main().catch((err) => {
  console.error("Setup failed:", err.message);
  process.exit(1);
});
