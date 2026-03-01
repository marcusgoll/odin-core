import { describe, it, expect, afterEach } from "vitest";
import { loadConfig } from "../src/config.js";

describe("loadConfig", () => {
  const origEnv = { ...process.env };

  afterEach(() => {
    process.env = { ...origEnv };
  });

  it("should return defaults when no env vars set", () => {
    delete process.env.GMAIL_RULES_PATH;
    delete process.env.GMAIL_STATE_DIR;
    const cfg = loadConfig();
    expect(cfg.rulesPath).toBe("/var/odin/config/gmail-rules.yaml");
    expect(cfg.stateDir).toBe("/var/odin/state/gmail");
    expect(cfg.account).toBe("personal");
  });

  it("should read env overrides", () => {
    process.env.GMAIL_RULES_PATH = "/tmp/rules.yaml";
    process.env.GMAIL_STATE_DIR = "/tmp/state";
    process.env.GMAIL_ACCOUNT = "work";
    const cfg = loadConfig();
    expect(cfg.rulesPath).toBe("/tmp/rules.yaml");
    expect(cfg.stateDir).toBe("/tmp/state");
    expect(cfg.account).toBe("work");
  });
});
