import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import chalk from 'chalk';
import { findProjectRoot } from '../lib/config.mjs';

/**
 * Generate the agent prompt markdown from a name and optional prompt text.
 */
function agentTemplate(name, prompt) {
  const capitalized = name.charAt(0).toUpperCase() + name.slice(1);
  const role = prompt || 'Describe this agent\'s role and responsibilities.';

  return `# ${capitalized} Agent

## Role
${role}

## Capabilities
- List what this agent can do

## Workflow
1. Read the task
2. Execute the work
3. Report results

## Constraints
- List limitations and guardrails
`;
}

export function registerAgentCommand(program) {
  const agentCmd = program
    .command('agent')
    .description('Manage agents');

  agentCmd
    .command('create <name>')
    .description('Create a new agent prompt file')
    .option('--prompt <prompt>', 'Agent role description')
    .action((name, opts) => {
      try {
        const root = findProjectRoot();
        const agentsDir = join(root, 'agents');

        // Ensure agents/ directory exists
        if (!existsSync(agentsDir)) {
          mkdirSync(agentsDir, { recursive: true });
        }

        const filePath = join(agentsDir, `${name}.md`);

        if (existsSync(filePath)) {
          console.error(chalk.red(`Error: Agent "${name}" already exists.`));
          process.exit(1);
        }

        const content = agentTemplate(name, opts.prompt);
        writeFileSync(filePath, content, 'utf8');

        console.log(chalk.green(`Created agent "${name}" at agents/${name}.md`));
      } catch (err) {
        console.error(chalk.red(err.message));
        process.exit(1);
      }
    });

  agentCmd
    .command('list')
    .description('List all agents in the project')
    .action(() => {
      try {
        const root = findProjectRoot();
        const agentsDir = join(root, 'agents');

        if (!existsSync(agentsDir)) {
          console.log(chalk.yellow('No agents directory found.'));
          return;
        }

        const files = readdirSync(agentsDir)
          .filter((f) => f.endsWith('.md'))
          .map((f) => f.replace(/\.md$/, ''));

        if (files.length === 0) {
          console.log(chalk.yellow('No agents found.'));
          return;
        }

        console.log(chalk.bold('Agents:'));
        for (const name of files) {
          console.log(chalk.cyan(`  ${name}`));
        }
      } catch (err) {
        console.error(chalk.red(err.message));
        process.exit(1);
      }
    });

  agentCmd
    .command('show <name>')
    .description('Show an agent prompt file')
    .action((name) => {
      try {
        const root = findProjectRoot();
        const filePath = join(root, 'agents', `${name}.md`);

        if (!existsSync(filePath)) {
          console.error(chalk.red(`Error: Agent "${name}" not found.`));
          process.exit(1);
        }

        const content = readFileSync(filePath, 'utf8');
        console.log(content);
      } catch (err) {
        console.error(chalk.red(err.message));
        process.exit(1);
      }
    });
}
