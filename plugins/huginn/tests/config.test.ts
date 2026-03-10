import { describe, it, expect, afterEach } from "vitest";
import {
  isDomainAllowed,
  loadConfig,
  normalizeObserveTarget,
} from "../src/config.js";

describe("Config", () => {
  const origEnv = { ...process.env };

  afterEach(() => {
    process.env = { ...origEnv };
  });

  it("loads defaults when no env vars set", () => {
    delete process.env.HUGINN_SERVER_URL;
    delete process.env.HUGINN_HEADLESS;
    delete process.env.HUGINN_ALLOWED_DOMAINS;
    const cfg = loadConfig();
    expect(cfg.serverUrl).toBe("http://127.0.0.1:9227");
    expect(cfg.headless).toBe(true);
    expect(cfg.allowlistedDomains).toContain("cfipros.com");
  });

  it("respects HUGINN_HEADLESS=false", () => {
    process.env.HUGINN_HEADLESS = "false";
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

describe("normalizeObserveTarget", () => {
  it("passes through url input", () => {
    expect(
      normalizeObserveTarget({ url: "https://cfipros.com/pricing" }),
    ).toBe("https://cfipros.com/pricing");
  });

  it("normalizes a bare domain to https", () => {
    expect(normalizeObserveTarget({ domain: "cfipros.com" })).toBe(
      "https://cfipros.com",
    );
  });
});
