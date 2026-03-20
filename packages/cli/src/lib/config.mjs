import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import YAML from 'yaml';

const CONFIG_FILE = 'odin.yaml';

/**
 * Walk up directories from startDir looking for odin.yaml.
 * Returns the directory path containing odin.yaml.
 * Throws if not found.
 */
export function findProjectRoot(startDir = process.cwd()) {
  let dir = startDir;
  while (true) {
    if (existsSync(join(dir, CONFIG_FILE))) {
      return dir;
    }
    const parent = dirname(dir);
    if (parent === dir) {
      throw new Error("Not in an Odin project. Run 'odin init' first.");
    }
    dir = parent;
  }
}

/**
 * Read and parse odin.yaml from the given project root.
 * Returns a JS object.
 */
export function loadConfig(projectRoot) {
  const configPath = join(projectRoot, CONFIG_FILE);
  const raw = readFileSync(configPath, 'utf8');
  return YAML.parse(raw) || {};
}

/**
 * Write config object back to odin.yaml.
 */
export function saveConfig(projectRoot, config) {
  const configPath = join(projectRoot, CONFIG_FILE);
  writeFileSync(configPath, YAML.stringify(config), 'utf8');
}

/**
 * Get a nested value by dot path (e.g., "llm.api_key").
 * Returns undefined if the path doesn't exist.
 */
export function getConfigValue(obj, dotPath) {
  const keys = dotPath.split('.');
  let current = obj;
  for (const key of keys) {
    if (current == null || typeof current !== 'object') {
      return undefined;
    }
    current = current[key];
  }
  return current;
}

/**
 * Set a nested value by dot path. Creates intermediate objects as needed.
 */
export function setConfigValue(obj, dotPath, value) {
  const keys = dotPath.split('.');
  let current = obj;
  for (let i = 0; i < keys.length - 1; i++) {
    const key = keys[i];
    if (current[key] == null || typeof current[key] !== 'object') {
      current[key] = {};
    }
    current = current[key];
  }
  current[keys[keys.length - 1]] = value;
}

/** Patterns that indicate a secret value */
const SECRET_PATTERNS = ['key', 'secret', 'token', 'password'];

/**
 * Check if a key name looks like it holds a secret.
 */
function isSecretKey(key) {
  const lower = key.toLowerCase();
  return SECRET_PATTERNS.some((pat) => lower.includes(pat));
}

/**
 * Redact a single value: show first 3 chars + "...****"
 */
function redactValue(val) {
  const str = String(val);
  if (str.length <= 3) {
    return '...****';
  }
  return str.slice(0, 3) + '...****';
}

/**
 * Deep clone a config object, replacing any key containing
 * "key", "secret", "token", or "password" with a redacted version.
 */
export function redactSecrets(config) {
  if (config == null || typeof config !== 'object') {
    return config;
  }

  if (Array.isArray(config)) {
    return config.map((item) => redactSecrets(item));
  }

  const result = {};
  for (const [key, value] of Object.entries(config)) {
    if (isSecretKey(key) && typeof value === 'string') {
      result[key] = redactValue(value);
    } else if (typeof value === 'object' && value !== null) {
      result[key] = redactSecrets(value);
    } else {
      result[key] = value;
    }
  }
  return result;
}
