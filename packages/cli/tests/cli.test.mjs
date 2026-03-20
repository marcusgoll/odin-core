import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { execSync } from 'node:child_process';
import { existsSync, readFileSync, mkdtempSync, rmSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const CLI = join(__dirname, '..', 'bin', 'odin.mjs');
const run = (args, opts = {}) =>
  execSync(`node ${CLI} ${args}`, { encoding: 'utf8', ...opts });

describe('odin --help', () => {
  it('shows all top-level commands', () => {
    const help = run('--help');
    assert.match(help, /init/);
    assert.match(help, /run/);
    assert.match(help, /config/);
    assert.match(help, /template/);
    assert.match(help, /agent/);
  });
});

describe('odin --version', () => {
  it('prints a semver version string', () => {
    const version = run('--version');
    assert.match(version, /\d+\.\d+\.\d+/);
  });
});

describe('odin init', () => {
  let tmpDir;

  before(() => {
    tmpDir = mkdtempSync(join(tmpdir(), 'odin-test-init-'));
  });

  after(() => {
    rmSync(tmpDir, { recursive: true, force: true });
  });

  it('creates project structure', () => {
    run('init test-project', { cwd: tmpDir });

    const projectDir = join(tmpDir, 'test-project');

    // odin.yaml exists and contains the project name
    const configPath = join(projectDir, 'odin.yaml');
    assert.ok(existsSync(configPath), 'odin.yaml should exist');
    const configContent = readFileSync(configPath, 'utf8');
    assert.ok(
      configContent.includes('test-project'),
      'odin.yaml should contain project name'
    );

    // Standard directories exist
    for (const dir of ['agents', 'skills', 'templates', 'output']) {
      assert.ok(
        existsSync(join(projectDir, dir)),
        `${dir}/ directory should exist`
      );
    }

    // .gitignore exists
    assert.ok(
      existsSync(join(projectDir, '.gitignore')),
      '.gitignore should exist'
    );
  });

  it('fails on existing directory', () => {
    // The first init already created test-project in tmpDir
    assert.throws(
      () => run('init test-project', { cwd: tmpDir }),
      (err) => {
        assert.ok(err.status !== 0, 'should exit with non-zero code');
        return true;
      }
    );
  });
});

describe('odin config', () => {
  let tmpDir;
  let projectDir;

  before(() => {
    tmpDir = mkdtempSync(join(tmpdir(), 'odin-test-config-'));
    run('init myproject', { cwd: tmpDir });
    projectDir = join(tmpDir, 'myproject');
  });

  after(() => {
    rmSync(tmpDir, { recursive: true, force: true });
  });

  it('set and get a secret value (redacted)', () => {
    run('config set llm.api_key sk-test12345', { cwd: projectDir });

    const output = run('config get llm.api_key', { cwd: projectDir });
    // Should be redacted: first 3 chars + ...****
    assert.match(output, /sk-\.\.\.\*\*\*\*/);
    // Must NOT contain the full key
    assert.ok(
      !output.includes('sk-test12345'),
      'full key should not be displayed'
    );
  });

  it('set and get a plain value', () => {
    run('config set llm.provider anthropic', { cwd: projectDir });

    const output = run('config get llm.provider', { cwd: projectDir });
    assert.match(output, /anthropic/);
  });

  it('list shows all config with secrets redacted', () => {
    const output = run('config list', { cwd: projectDir });
    assert.ok(output.includes('anthropic'), 'should contain provider');
    assert.match(output, /sk-\.\.\.\*\*\*\*/);
    assert.ok(
      !output.includes('sk-test12345'),
      'full key should not appear in list'
    );
  });
});

describe('odin template list', () => {
  it('shows bundled templates', () => {
    const output = run('template list');
    for (const name of [
      'lead-gen',
      'price-monitor',
      'content-ops',
      'workflow-automation',
      'hello-world',
    ]) {
      assert.ok(
        output.includes(name),
        `template list should contain "${name}"`
      );
    }
  });
});

describe('odin template add', () => {
  let tmpDir;
  let projectDir;

  before(() => {
    tmpDir = mkdtempSync(join(tmpdir(), 'odin-test-tmpladd-'));
    run('init tmplproj', { cwd: tmpDir });
    projectDir = join(tmpDir, 'tmplproj');
  });

  after(() => {
    rmSync(tmpDir, { recursive: true, force: true });
  });

  it('copies bundled template to project', () => {
    run('template add hello-world', { cwd: projectDir });

    const templateDir = join(projectDir, 'templates', 'hello-world');
    assert.ok(existsSync(templateDir), 'hello-world dir should exist');
    assert.ok(
      existsSync(join(templateDir, 'manifest.yaml')),
      'manifest.yaml should exist'
    );
    assert.ok(
      existsSync(join(templateDir, 'run.sh')),
      'run.sh should exist'
    );
  });
});

describe('odin agent', () => {
  let tmpDir;
  let projectDir;

  before(() => {
    tmpDir = mkdtempSync(join(tmpdir(), 'odin-test-agent-'));
    run('init agentproj', { cwd: tmpDir });
    projectDir = join(tmpDir, 'agentproj');
  });

  after(() => {
    rmSync(tmpDir, { recursive: true, force: true });
  });

  it('create writes agent prompt file', () => {
    run('agent create researcher --prompt "Research companies"', {
      cwd: projectDir,
    });

    const agentFile = join(projectDir, 'agents', 'researcher.md');
    assert.ok(existsSync(agentFile), 'researcher.md should exist');

    const content = readFileSync(agentFile, 'utf8');
    assert.ok(
      content.includes('Research companies'),
      'agent file should contain the prompt text'
    );
  });

  it('list shows created agent', () => {
    const output = run('agent list', { cwd: projectDir });
    assert.ok(
      output.includes('researcher'),
      'agent list should contain "researcher"'
    );
  });

  it('show displays agent prompt', () => {
    const output = run('agent show researcher', { cwd: projectDir });
    assert.ok(
      output.includes('Research companies'),
      'agent show should contain the prompt text'
    );
  });
});

describe('odin run', () => {
  let tmpDir;
  let projectDir;

  before(() => {
    tmpDir = mkdtempSync(join(tmpdir(), 'odin-test-run-'));
    run('init runproj', { cwd: tmpDir });
    projectDir = join(tmpDir, 'runproj');
    run('template add hello-world', { cwd: projectDir });
  });

  after(() => {
    rmSync(tmpDir, { recursive: true, force: true });
  });

  it('executes template and produces output file', () => {
    const output = run('run hello-world -- Marcus', { cwd: projectDir });

    // Check output file was created
    const greetingPath = join(projectDir, 'output', 'greeting.txt');
    assert.ok(existsSync(greetingPath), 'greeting.txt should exist');

    // Check content contains the name
    const content = readFileSync(greetingPath, 'utf8');
    assert.ok(
      content.includes('Marcus'),
      'greeting.txt should contain "Marcus"'
    );
  });
});
