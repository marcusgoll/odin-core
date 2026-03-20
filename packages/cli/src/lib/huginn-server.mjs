/**
 * Minimal Huginn browser server for Odin CLI.
 *
 * A stripped-down version of the full Huginn server
 * (odin-orchestrator/scripts/odin/lib/odin-huginn-server.js).
 * Provides the core endpoints needed for template automation:
 *   GET  /health     — Server health check
 *   POST /launch     — Launch browser and navigate to URL
 *   POST /navigate   — Navigate current page
 *   GET  /snapshot   — Accessibility tree with [ref=eN] markers
 *   POST /evaluate   — Evaluate JS in page context
 *   POST /act        — Interact with elements (click, type, press)
 *   GET  /screenshot  — Base64 PNG screenshot
 *   POST /stop       — Close browser and stop server
 *
 * Binds to 127.0.0.1 only (not exposed to network).
 * Configurable via ODIN_BROWSER_PORT (default 9227).
 * Optional auth via ODIN_BROWSER_TOKEN env var.
 */

import { createServer } from 'node:http';

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------
const DEFAULT_PORT = 9227;
const HOST = '127.0.0.1';
const AUTH_TOKEN = (process.env.ODIN_BROWSER_TOKEN || '').trim();

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------
let browser = null;
let context = null;
let page = null;
let knownRefs = new Set();
let httpServer = null;

function resetState() {
  browser = null;
  context = null;
  page = null;
  knownRefs.clear();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function badRequest(msg) {
  return Object.assign(new Error(msg), { status: 400 });
}

function refToLocator(ref) {
  if (!page) throw badRequest('No page open');
  if (!knownRefs.has(ref)) {
    throw badRequest(`Unknown ref "${ref}". Take a new snapshot to get current refs.`);
  }
  return page.locator(`aria-ref=${ref}`);
}

function readBody(req, maxBytes = 10 * 1024 * 1024) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let size = 0;
    req.on('data', (c) => {
      size += c.length;
      if (size > maxBytes) {
        req.destroy();
        return reject(new Error('Request body too large'));
      }
      chunks.push(c);
    });
    req.on('end', () => {
      const raw = Buffer.concat(chunks).toString();
      if (!raw) return resolve({});
      try {
        resolve(JSON.parse(raw));
      } catch {
        reject(new Error('Invalid JSON body'));
      }
    });
    req.on('error', reject);
  });
}

function parseQuery(url) {
  const idx = url.indexOf('?');
  if (idx === -1) return {};
  const params = new URLSearchParams(url.slice(idx));
  const out = {};
  for (const [k, v] of params) out[k] = v;
  return out;
}

function jsonResponse(res, status, body) {
  const payload = JSON.stringify(body);
  res.writeHead(status, {
    'Content-Type': 'application/json',
    'Content-Length': Buffer.byteLength(payload),
  });
  res.end(payload);
}

function pathname(url) {
  const idx = url.indexOf('?');
  return idx === -1 ? url : url.slice(0, idx);
}

function isBrowserLive(candidate) {
  if (!candidate) return false;
  try {
    return typeof candidate.isConnected === 'function'
      ? candidate.isConnected()
      : true;
  } catch {
    return false;
  }
}

function isPageLive(candidate) {
  if (!candidate) return false;
  try {
    return typeof candidate.isClosed === 'function' ? !candidate.isClosed() : true;
  } catch {
    return false;
  }
}

function isAuthorized(req, path) {
  if (!AUTH_TOKEN || path === '/health') return true;
  return req.headers.authorization === `Bearer ${AUTH_TOKEN}`;
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

async function handleHealth() {
  return {
    ok: true,
    browser: isBrowserLive(browser),
    url: isPageLive(page) ? page.url() : null,
  };
}

async function handleLaunch(body) {
  // Tear down any existing browser
  if (browser) {
    try {
      await browser.close();
    } catch { /* ignore */ }
    resetState();
  }

  let chromium;
  try {
    const pw = await import('playwright');
    chromium = pw.chromium;
  } catch {
    throw Object.assign(
      new Error(
        'Playwright is not installed. Run: npm install playwright && npx playwright install chromium'
      ),
      { status: 500 }
    );
  }

  const headless = body.headless !== false;
  browser = await chromium.launch({
    headless,
    args: ['--no-sandbox', '--disable-blink-features=AutomationControlled'],
  });

  const ctxOpts = {
    viewport: body.viewport || { width: 1920, height: 1080 },
  };
  if (body.userAgent) ctxOpts.userAgent = body.userAgent;

  context = await browser.newContext(ctxOpts);
  page = await context.newPage();

  let url = null;
  if (body.url) {
    await page.goto(body.url, { waitUntil: 'domcontentloaded' });
    try {
      await page.waitForLoadState('load', { timeout: 5000 });
    } catch { /* timeout ok */ }
    url = page.url();
  }

  return { ok: true, ...(url ? { url } : {}) };
}

async function handleNavigate(body) {
  if (!page || !isPageLive(page)) throw badRequest('No page open');

  if (body.url) {
    await page.goto(body.url, { waitUntil: 'domcontentloaded' });
    try {
      await page.waitForLoadState('load', { timeout: 5000 });
    } catch { /* timeout ok */ }
    knownRefs.clear();
  } else if (body.action) {
    switch (body.action) {
      case 'back':
        await page.goBack();
        break;
      case 'forward':
        await page.goForward();
        break;
      case 'reload':
        await page.reload();
        break;
      default:
        throw badRequest(`Unknown action: ${body.action}`);
    }
    knownRefs.clear();
  } else {
    throw badRequest('url or action is required');
  }

  let title = '';
  try {
    title = await page.title();
  } catch { /* ignore */ }

  return { ok: true, url: page.url(), title };
}

async function handleSnapshot(query) {
  if (!page || !isPageLive(page)) throw badRequest('No page open');

  let snapshotText = '';

  // Prefer _snapshotForAI (Playwright 1.50+), fallback to accessibility.snapshot()
  if (typeof page._snapshotForAI === 'function') {
    const result = await page._snapshotForAI();
    snapshotText = result.full || '';
  } else {
    // Fallback: accessibility tree
    const tree = await page.accessibility.snapshot();
    snapshotText = tree ? JSON.stringify(tree, null, 2) : '';
  }

  // Extract refs from snapshot
  knownRefs = new Set();
  const refs = {};
  const lines = snapshotText.split('\n');
  for (const line of lines) {
    const lineRefs = [...line.matchAll(/\[ref=(e\d+)\]/g)];
    for (const lr of lineRefs) {
      knownRefs.add(lr[1]);
      refs[lr[1]] = line.trim();
    }
  }

  const response = {
    snapshot: snapshotText,
    refs,
    stats: {
      totalRefs: knownRefs.size,
      interactiveRefs: Object.keys(refs).length,
    },
  };

  // Compact mode: only lines with refs or blank lines
  if (query.compact === 'true' || query.compact === '1') {
    const compactLines = lines.filter(
      (l) => /\[ref=e\d+\]/.test(l) || l.trim().length === 0
    );
    response.snapshot = compactLines.join('\n');
  }

  return response;
}

async function handleEvaluate(body) {
  if (!page || !isPageLive(page)) throw badRequest('No page open');
  if (!body.fn) throw badRequest('fn is required');

  // body.fn is a string like '() => document.title' or 'document.title'
  // If it's a function expression, wrap in parens and invoke.
  let fn = body.fn.trim();
  let result;

  if (fn.startsWith('(') || fn.startsWith('function') || fn.startsWith('async')) {
    result = await page.evaluate(`(${fn})()`);
  } else {
    result = await page.evaluate(fn);
  }

  return { ok: true, result };
}

async function handleAct(body) {
  if (!page || !isPageLive(page)) throw badRequest('No page open');
  if (!body.kind) throw badRequest('kind is required');

  const { kind, ref, selector, text, submit, key } = body;

  switch (kind) {
    case 'click': {
      if (ref !== undefined) {
        const locator = refToLocator(ref);
        await locator.click();
      } else if (selector) {
        await page.locator(selector).click();
      } else {
        throw badRequest('ref or selector is required for click');
      }
      break;
    }

    case 'type': {
      let locator;
      if (ref !== undefined) {
        locator = refToLocator(ref);
      } else if (selector) {
        locator = page.locator(selector);
      } else {
        throw badRequest('ref or selector is required for type');
      }

      const typeText = text || '';
      await locator.click();
      await locator.fill('');
      await locator.fill(typeText);

      if (submit) {
        await locator.press('Enter');
      }
      break;
    }

    case 'press': {
      if (!key) throw badRequest('key is required for press');
      await page.keyboard.press(key);
      break;
    }

    case 'hover': {
      if (ref !== undefined) {
        const locator = refToLocator(ref);
        await locator.hover();
      } else if (selector) {
        await page.locator(selector).hover();
      } else {
        throw badRequest('ref or selector is required for hover');
      }
      break;
    }

    default:
      throw badRequest(`Unknown action kind: ${kind}`);
  }

  return { ok: true, url: page.url() };
}

async function handleScreenshot(query) {
  if (!page || !isPageLive(page)) throw badRequest('No page open');

  const opts = {};
  if (query.fullPage === 'true' || query.fullPage === '1') {
    opts.fullPage = true;
  }

  let buf;
  if (query.ref !== undefined) {
    const locator = refToLocator(query.ref);
    buf = await locator.screenshot(opts);
  } else {
    buf = await page.screenshot(opts);
  }

  return { ok: true, data: buf.toString('base64') };
}

async function handleStop() {
  if (browser) {
    try {
      await browser.close();
    } catch { /* ignore */ }
    resetState();
  }

  // Schedule server close after response is sent
  if (httpServer) {
    setTimeout(() => {
      httpServer.close();
      process.exit(0);
    }, 100);
  }

  return { ok: true };
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

const routes = {
  'GET /health': () => handleHealth(),
  'POST /launch': (body) => handleLaunch(body),
  'POST /navigate': (body) => handleNavigate(body),
  'GET /snapshot': (_body, query) => handleSnapshot(query),
  'POST /evaluate': (body) => handleEvaluate(body),
  'POST /act': (body) => handleAct(body),
  'GET /screenshot': (_body, query) => handleScreenshot(query),
  'POST /stop': () => handleStop(),
};

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/**
 * Start the Huginn browser server on the given port.
 * Returns a Promise that resolves with the http.Server once listening.
 *
 * @param {number} [port] - Port to listen on (default from env or 9227)
 * @returns {Promise<import('node:http').Server>}
 */
export function startServer(port) {
  const listenPort = port || parseInt(process.env.ODIN_BROWSER_PORT || String(DEFAULT_PORT), 10);

  return new Promise((resolve, reject) => {
    httpServer = createServer(async (req, res) => {
      const path = pathname(req.url);
      const method = (req.method || 'GET').toUpperCase();
      const key = `${method} ${path}`;

      const handler = routes[key];
      if (!handler) {
        return jsonResponse(res, 404, { error: `Not found: ${method} ${path}` });
      }

      if (!isAuthorized(req, path)) {
        return jsonResponse(res, 401, { error: 'Unauthorized' });
      }

      try {
        const body = method === 'GET' ? {} : await readBody(req);
        const query = parseQuery(req.url);
        const result = await handler(body, query);
        jsonResponse(res, 200, result);
      } catch (err) {
        const status = err.status || 500;
        const message = err.message || 'Internal server error';
        if (status === 500) console.error('[huginn]', err);
        jsonResponse(res, status, { error: message });
      }
    });

    httpServer.on('error', reject);

    httpServer.listen(listenPort, HOST, () => {
      console.log(`huginn-server listening on ${HOST}:${listenPort}`);
      resolve(httpServer);
    });
  });
}

// ---------------------------------------------------------------------------
// Graceful shutdown
// ---------------------------------------------------------------------------

async function shutdown(signal) {
  console.log(`\n[${signal}] shutting down...`);
  if (browser) {
    try {
      await browser.close();
    } catch { /* ignore */ }
    resetState();
  }
  if (httpServer) {
    httpServer.close(() => process.exit(0));
    setTimeout(() => process.exit(1), 5000).unref();
  } else {
    process.exit(0);
  }
}

process.on('SIGINT', () => shutdown('SIGINT'));
process.on('SIGTERM', () => shutdown('SIGTERM'));

// ---------------------------------------------------------------------------
// Direct execution: node huginn-server.mjs
// ---------------------------------------------------------------------------
// When run directly (not imported), auto-start the server.
const isDirectRun =
  process.argv[1] &&
  (process.argv[1].endsWith('huginn-server.mjs') ||
    process.argv[1].endsWith('huginn-server'));

if (isDirectRun) {
  startServer().catch((err) => {
    console.error('Failed to start huginn-server:', err.message);
    process.exit(1);
  });
}
