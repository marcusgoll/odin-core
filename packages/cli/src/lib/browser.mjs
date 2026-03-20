/**
 * Browser lifecycle management.
 *
 * Provides helpers to check browser server health, ensure it's running,
 * and stop it. The browser server is the Huginn Playwright server that
 * templates use for web automation.
 *
 * When the server is not already running, ensureBrowser() will auto-start
 * the bundled Huginn server as a background child process.
 */

import { spawn, execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

/** Reference to the child process we spawned (if any). */
let serverProcess = null;

/**
 * Check if the browser server is healthy on the given port.
 * Returns true if the /health endpoint responds successfully.
 */
export async function browserHealth(port) {
  try {
    const res = await fetch(`http://127.0.0.1:${port}/health`, {
      signal: AbortSignal.timeout(3000),
    });
    return res.ok;
  } catch {
    return false;
  }
}

/**
 * Check whether Playwright browsers are installed.
 * Returns true if chromium is available.
 */
function isPlaywrightInstalled() {
  try {
    // Playwright stores browsers in a well-known cache dir.
    // The quickest check: try to resolve playwright and let it tell us.
    execSync('npx playwright install --dry-run chromium 2>&1', {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      timeout: 10000,
    });
    return true;
  } catch {
    // If dry-run fails, check if playwright module at least exists
    try {
      require.resolve('playwright');
      return true;
    } catch {
      return false;
    }
  }
}

/**
 * Check if playwright npm package can be imported.
 */
async function canImportPlaywright() {
  try {
    await import('playwright');
    return true;
  } catch {
    return false;
  }
}

/**
 * Start the bundled Huginn server as a background child process.
 * Returns a Promise that resolves once /health responds OK.
 *
 * @param {number} port - Port to start on
 * @param {number} [timeoutMs=15000] - How long to wait for startup
 */
async function startBundledServer(port, timeoutMs = 15000) {
  const serverPath = join(__dirname, 'huginn-server.mjs');

  const env = { ...process.env, ODIN_BROWSER_PORT: String(port) };

  serverProcess = spawn(process.execPath, [serverPath], {
    env,
    stdio: ['ignore', 'pipe', 'pipe'],
    detached: false,
  });

  // Log server output to stderr for debugging
  serverProcess.stderr.on('data', (data) => {
    const msg = data.toString().trim();
    if (msg) console.error(`[huginn] ${msg}`);
  });
  serverProcess.stdout.on('data', (data) => {
    const msg = data.toString().trim();
    if (msg) console.error(`[huginn] ${msg}`);
  });

  serverProcess.on('error', (err) => {
    console.error(`[huginn] Process error: ${err.message}`);
    serverProcess = null;
  });

  serverProcess.on('exit', (code) => {
    if (code !== 0 && code !== null) {
      console.error(`[huginn] Process exited with code ${code}`);
    }
    serverProcess = null;
  });

  // Prevent the child from keeping the parent alive if the parent exits
  serverProcess.unref();

  // Poll /health until it responds
  const startedAt = Date.now();
  const pollInterval = 500;

  while (Date.now() - startedAt < timeoutMs) {
    const healthy = await browserHealth(port);
    if (healthy) return;
    await new Promise((r) => setTimeout(r, pollInterval));
  }

  // Timed out
  if (serverProcess) {
    serverProcess.kill();
    serverProcess = null;
  }
  throw new Error(
    `Huginn server failed to start within ${timeoutMs / 1000}s on port ${port}`
  );
}

/**
 * Ensure the browser server is available.
 *
 * 1. Check if server is already running (GET /health)
 * 2. If not, verify Playwright is available
 * 3. Start the bundled server as a background child process
 * 4. Wait for /health to return ok
 *
 * @param {object} config - Project config (from odin.yaml)
 * @returns {number} The browser port
 */
export async function ensureBrowser(config) {
  const port = config?.browser?.port || 9227;

  // Already running? Great.
  const healthy = await browserHealth(port);
  if (healthy) return port;

  const chalk = (await import('chalk')).default;

  // Check if playwright is available
  const pwAvailable = await canImportPlaywright();
  if (!pwAvailable) {
    console.log('');
    console.log(
      chalk.yellow(
        '  Browser automation requires Playwright, which is not installed.'
      )
    );
    console.log(
      chalk.dim(
        '  Install it with: npm install playwright && npx playwright install chromium'
      )
    );
    console.log(
      chalk.dim(
        '  Templates that require a browser will fail until Playwright is installed.'
      )
    );
    console.log('');
    return port;
  }

  // Start the bundled server
  console.log(chalk.dim('  Starting browser server...'));

  try {
    await startBundledServer(port);
    console.log(chalk.green(`  Browser server started on port ${port}`));
  } catch (err) {
    console.log('');
    console.log(
      chalk.yellow(`  Warning: Failed to start browser server: ${err.message}`)
    );
    console.log(
      chalk.dim('  Templates that require a browser may fail.')
    );
    console.log('');
  }

  return port;
}

/**
 * Stop the browser server on the given port.
 * If we started the server, also kill the child process.
 * Returns true if the stop request succeeded.
 */
export async function stopBrowser(port) {
  try {
    const res = await fetch(`http://127.0.0.1:${port}/stop`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      signal: AbortSignal.timeout(5000),
    });

    // Give the process a moment to exit
    if (serverProcess) {
      await new Promise((r) => setTimeout(r, 200));
      if (serverProcess) {
        serverProcess.kill();
        serverProcess = null;
      }
    }

    return res.ok;
  } catch {
    // If fetch failed but we have a child process, kill it directly
    if (serverProcess) {
      serverProcess.kill();
      serverProcess = null;
    }
    return false;
  }
}
