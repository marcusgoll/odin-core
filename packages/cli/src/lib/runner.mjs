/**
 * Template execution engine.
 *
 * Runs a template's run.sh script with the correct environment variables
 * and streams output to the terminal in real-time.
 */

import { spawn } from 'node:child_process';
import { join } from 'node:path';
import { chmodSync, existsSync, readdirSync, mkdirSync } from 'node:fs';

/**
 * Execute a template.
 *
 * @param {string} templateDir  - Absolute path to the template directory
 * @param {string[]} args       - Arguments to pass to run.sh (from after --)
 * @param {object} config       - Project config (from odin.yaml)
 * @param {string} projectRoot  - Absolute path to the project root
 * @returns {Promise<number>}   - Exit code from the template process
 */
export function runTemplate(templateDir, args, config, projectRoot) {
  return new Promise((resolve, reject) => {
    const runScript = join(templateDir, 'run.sh');

    if (!existsSync(runScript)) {
      reject(new Error(`Template is missing run.sh at ${templateDir}`));
      return;
    }

    // Ensure run.sh is executable
    try {
      chmodSync(runScript, 0o755);
    } catch {
      // Ignore chmod errors (e.g., on read-only filesystems)
    }

    // Set up output directory
    const outputDir = join(projectRoot, 'output');
    mkdirSync(outputDir, { recursive: true });

    // Build environment variables
    const env = { ...process.env };

    // LLM configuration
    if (config?.llm?.api_key) {
      env.ODIN_LLM_API_KEY = config.llm.api_key;
    }
    if (config?.llm?.provider) {
      env.ODIN_LLM_PROVIDER = config.llm.provider;
    }
    if (config?.llm?.model) {
      env.ODIN_LLM_MODEL = config.llm.model;
    }

    // Browser configuration
    const browserPort = config?.browser?.port || 9227;
    env.ODIN_BROWSER_URL = `http://127.0.0.1:${browserPort}`;
    if (process.env.ODIN_BROWSER_TOKEN) {
      env.ODIN_BROWSER_TOKEN = process.env.ODIN_BROWSER_TOKEN;
    }

    // Output directory
    env.ODIN_OUTPUT_DIR = outputDir;

    // Spawn bash run.sh with the template args
    const child = spawn('bash', [runScript, ...args], {
      cwd: templateDir,
      env,
      stdio: ['inherit', 'pipe', 'pipe'],
    });

    // Stream stdout to terminal in real-time
    child.stdout.on('data', (data) => {
      process.stdout.write(data);
    });

    // Stream stderr to terminal in real-time
    child.stderr.on('data', (data) => {
      process.stderr.write(data);
    });

    child.on('error', (err) => {
      reject(new Error(`Failed to execute template: ${err.message}`));
    });

    child.on('close', (code) => {
      resolve(code ?? 1);
    });
  });
}

/**
 * List files in the output directory for the post-run summary.
 * Returns an array of filenames (not full paths).
 */
export function listOutputFiles(projectRoot) {
  const outputDir = join(projectRoot, 'output');
  if (!existsSync(outputDir)) {
    return [];
  }

  try {
    return readdirSync(outputDir).filter((f) => f !== '.gitkeep');
  } catch {
    return [];
  }
}
