import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import chalk from 'chalk';
import ora from 'ora';
import { stringify } from 'yaml';

export function registerInitCommand(program) {
  program
    .command('init <name>')
    .description('Initialize a new Odin project')
    .action(async (name) => {
      const projectDir = join(process.cwd(), name);

      // Check if directory already exists
      if (existsSync(projectDir)) {
        console.error(
          chalk.red(`Error: Directory "${name}" already exists.`)
        );
        process.exit(1);
      }

      const spinner = ora(`Creating project "${name}"...`).start();

      try {
        // Create project root
        mkdirSync(projectDir, { recursive: true });

        // Create subdirectories with .gitkeep files
        const dirs = ['agents', 'skills', 'templates', 'output'];
        for (const dir of dirs) {
          const dirPath = join(projectDir, dir);
          mkdirSync(dirPath, { recursive: true });
          writeFileSync(join(dirPath, '.gitkeep'), '');
        }

        // Create odin.yaml
        const config = {
          name,
          version: '0.1.0',
          llm: {
            provider: 'openai',
            model: 'gpt-4o',
            api_key: '',
          },
          browser: {
            headless: true,
            port: 9227,
          },
          agents: [],
          templates: [],
        };
        writeFileSync(join(projectDir, 'odin.yaml'), stringify(config));

        // Create .gitignore
        const gitignore = [
          'output/',
          '.odin/',
          'node_modules/',
          '*.log',
          '',
        ].join('\n');
        writeFileSync(join(projectDir, '.gitignore'), gitignore);

        spinner.succeed(`Created project "${name}"`);

        // Print next steps
        console.log('');
        console.log(chalk.bold('  Next steps:'));
        console.log(chalk.cyan(`    cd ${name}`));
        console.log(
          chalk.cyan('    odin config set llm.api_key <your-key>')
        );
        console.log(chalk.cyan('    odin template add lead-gen'));
        console.log(chalk.cyan('    odin run lead-gen'));
        console.log('');
      } catch (err) {
        spinner.fail('Failed to create project');
        console.error(chalk.red(err.message));
        process.exit(1);
      }
    });
}
