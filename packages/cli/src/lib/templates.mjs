import { readFileSync, readdirSync, existsSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import YAML from 'yaml';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

/**
 * Returns the absolute path to the bundled templates directory
 * (packages/cli/templates/).
 */
export function getBundledTemplatesDir() {
  return join(__dirname, '..', '..', 'templates');
}

/**
 * Returns the absolute path to the project's templates directory.
 */
export function getProjectTemplatesDir(projectRoot) {
  return join(projectRoot, 'templates');
}

/**
 * Read a single manifest.yaml from a template directory.
 * Returns the parsed object with an added `path` field, or null on failure.
 */
function readManifest(templateDir) {
  const manifestPath = join(templateDir, 'manifest.yaml');
  if (!existsSync(manifestPath)) {
    return null;
  }
  try {
    const raw = readFileSync(manifestPath, 'utf8');
    const manifest = YAML.parse(raw) || {};
    manifest._path = templateDir;
    return manifest;
  } catch {
    return null;
  }
}

/**
 * Scan a templates directory and return an array of parsed manifests.
 * Each entry includes a `_path` field pointing to its directory.
 */
function scanTemplatesDir(templatesDir) {
  if (!existsSync(templatesDir)) {
    return [];
  }

  const entries = readdirSync(templatesDir);
  const manifests = [];

  for (const entry of entries) {
    const entryPath = join(templatesDir, entry);
    if (!statSync(entryPath).isDirectory()) {
      continue;
    }
    const manifest = readManifest(entryPath);
    if (manifest) {
      manifests.push(manifest);
    }
  }

  return manifests;
}

/**
 * List all bundled templates (shipped with the CLI).
 * Returns an array of manifest objects.
 */
export function listBundledTemplates() {
  return scanTemplatesDir(getBundledTemplatesDir());
}

/**
 * List all project-local templates.
 * Returns an array of manifest objects.
 */
export function listProjectTemplates(projectRoot) {
  return scanTemplatesDir(getProjectTemplatesDir(projectRoot));
}

/**
 * Find a template by name. Checks the project directory first,
 * then falls back to bundled templates.
 * Returns the manifest object (with `_path`) or null.
 */
export function getTemplate(name, projectRoot) {
  // Check project templates first
  if (projectRoot) {
    const projectDir = join(getProjectTemplatesDir(projectRoot), name);
    if (existsSync(projectDir)) {
      const manifest = readManifest(projectDir);
      if (manifest) {
        manifest._source = 'project';
        return manifest;
      }
    }
  }

  // Fall back to bundled templates
  const bundledDir = join(getBundledTemplatesDir(), name);
  if (existsSync(bundledDir)) {
    const manifest = readManifest(bundledDir);
    if (manifest) {
      manifest._source = 'bundled';
      return manifest;
    }
  }

  return null;
}
