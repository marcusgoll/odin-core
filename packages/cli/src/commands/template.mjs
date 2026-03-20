import { existsSync, cpSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import chalk from 'chalk';
import {
  listBundledTemplates,
  listProjectTemplates,
  getTemplate,
  getBundledTemplatesDir,
  getProjectTemplatesDir,
} from '../lib/templates.mjs';
import { findProjectRoot } from '../lib/config.mjs';

/**
 * Pad a string to a fixed width, truncating if needed.
 */
function pad(str, width) {
  if (str.length > width) {
    return str.slice(0, width - 1) + '\u2026';
  }
  return str.padEnd(width);
}

export function registerTemplateCommand(program) {
  const templateCmd = program
    .command('template')
    .description('Manage automation templates');

  // --- odin template list ---
  templateCmd
    .command('list')
    .description('List available templates')
    .action(() => {
      const bundled = listBundledTemplates();

      let project = [];
      let projectRoot = null;
      try {
        projectRoot = findProjectRoot();
        project = listProjectTemplates(projectRoot);
      } catch {
        // Not in a project — that's fine, just show bundled
      }

      // Deduplicate: project templates override bundled ones with the same name
      const byName = new Map();
      for (const t of bundled) {
        byName.set(t.name, { ...t, _source: 'bundled' });
      }
      for (const t of project) {
        byName.set(t.name, { ...t, _source: 'project' });
      }

      const all = [...byName.values()];

      if (all.length === 0) {
        console.log(chalk.yellow('No templates found.'));
        return;
      }

      // Table header
      const cols = { name: 20, desc: 40, ver: 10, source: 10 };
      console.log('');
      console.log(
        chalk.bold(
          pad('Name', cols.name) +
            pad('Description', cols.desc) +
            pad('Version', cols.ver) +
            pad('Source', cols.source)
        )
      );
      console.log(
        chalk.dim(
          '\u2500'.repeat(cols.name + cols.desc + cols.ver + cols.source)
        )
      );

      for (const t of all) {
        const requires =
          t.requires && t.requires.length > 0
            ? ` [${t.requires.join(', ')}]`
            : '';
        console.log(
          chalk.cyan(pad(t.name || '?', cols.name)) +
            pad((t.description || '') + requires, cols.desc) +
            pad(t.version || '-', cols.ver) +
            chalk.dim(pad(t._source || '-', cols.source))
        );
      }
      console.log('');
    });

  // --- odin template add <name> ---
  templateCmd
    .command('add <name>')
    .description('Add a bundled template to the current project')
    .action((name) => {
      // Must be in a project
      let projectRoot;
      try {
        projectRoot = findProjectRoot();
      } catch (err) {
        console.error(chalk.red(err.message));
        process.exit(1);
      }

      // Check if already installed in project
      const destDir = join(getProjectTemplatesDir(projectRoot), name);
      if (existsSync(destDir)) {
        console.error(
          chalk.red(
            `Template "${name}" already exists in this project at ${destDir}`
          )
        );
        process.exit(1);
      }

      // Find in bundled templates
      const srcDir = join(getBundledTemplatesDir(), name);
      if (!existsSync(srcDir)) {
        console.error(
          chalk.red(
            `Template "${name}" not found in bundled templates. Run "odin template list" to see available templates.`
          )
        );
        process.exit(1);
      }

      // Copy the template directory
      try {
        cpSync(srcDir, destDir, { recursive: true });
        console.log(
          chalk.green(`\u2713 Added template "${name}" to ${destDir}`)
        );
      } catch (err) {
        console.error(chalk.red(`Failed to add template: ${err.message}`));
        process.exit(1);
      }
    });

  // --- odin template info <name> ---
  templateCmd
    .command('info <name>')
    .description('Show detailed information about a template')
    .action((name) => {
      let projectRoot = null;
      try {
        projectRoot = findProjectRoot();
      } catch {
        // Not in a project — only search bundled
      }

      const template = getTemplate(name, projectRoot);
      if (!template) {
        console.error(
          chalk.red(
            `Template "${name}" not found. Run "odin template list" to see available templates.`
          )
        );
        process.exit(1);
      }

      // Print manifest details
      console.log('');
      console.log(chalk.bold.cyan(template.name || name));
      console.log(chalk.dim(`Source: ${template._source || 'unknown'}`));
      console.log('');

      if (template.description) {
        console.log(chalk.bold('Description'));
        console.log(`  ${template.description}`);
        console.log('');
      }

      if (template.version) {
        console.log(chalk.bold('Version'));
        console.log(`  ${template.version}`);
        console.log('');
      }

      if (template.requires && template.requires.length > 0) {
        console.log(chalk.bold('Requirements'));
        for (const req of template.requires) {
          console.log(`  - ${req}`);
        }
        console.log('');
      }

      if (template.inputs && template.inputs.length > 0) {
        console.log(chalk.bold('Inputs'));
        for (const input of template.inputs) {
          const reqTag = input.required ? chalk.red(' (required)') : '';
          const defTag =
            input.default !== undefined
              ? chalk.dim(` [default: ${input.default}]`)
              : '';
          console.log(`  ${chalk.cyan(input.name)}${reqTag}${defTag}`);
          if (input.description) {
            console.log(`    ${input.description}`);
          }
        }
        console.log('');
      }

      if (template.outputs && template.outputs.length > 0) {
        console.log(chalk.bold('Outputs'));
        for (const output of template.outputs) {
          console.log(`  - ${output}`);
        }
        console.log('');
      }

      // Print README if available
      const readmePath = join(template._path, 'README.md');
      if (existsSync(readmePath)) {
        const readme = readFileSync(readmePath, 'utf8');
        console.log(chalk.bold('\u2500\u2500\u2500 README \u2500\u2500\u2500'));
        console.log(readme);
      }
    });
}
