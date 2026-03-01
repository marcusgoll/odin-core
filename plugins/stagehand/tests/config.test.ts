import { describe, it, expect, afterEach } from "vitest";
import { loadConfig, isDomainAllowed } from "../src/config.js";

describe("Config", () => {
  const origEnv = { ...process.env };

  afterEach(() => {
    process.env = { ...origEnv };
  });

  it("loads defaults when no env vars set", () => {
    delete process.env.STAGEHAND_HEADLESS;
    delete process.env.STAGEHAND_PRIMARY_MODEL;
    delete process.env.STAGEHAND_ALLOWED_DOMAINS;
    const cfg = loadConfig();
    expect(cfg.headless).toBe(true);
    expect(cfg.primaryModel).toBe("anthropic/claude-sonnet-4-6");
    expect(cfg.allowlistedDomains).toContain("cfipros.com");
  });

  it("respects STAGEHAND_HEADLESS=false", () => {
    process.env.STAGEHAND_HEADLESS = "false";
    const cfg = loadConfig();
    expect(cfg.headless).toBe(false);
  });
});

describe("isDomainAllowed", () => {
  const cfg = loadConfig();

  it("allows cfipros.com", () => {
    expect(isDomainAllowed("https://cfipros.com/pricing", cfg)).toBe(true);
  });

  it("allows app.cfipros.com", () => {
    expect(isDomainAllowed("https://app.cfipros.com/dashboard", cfg)).toBe(true);
  });

  it("blocks unknown domains", () => {
    expect(isDomainAllowed("https://evil.com", cfg)).toBe(false);
  });

  it("handles invalid URLs", () => {
    expect(isDomainAllowed("not-a-url", cfg)).toBe(false);
  });
});
