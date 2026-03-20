import chalk from 'chalk';
import { findProjectRoot, loadConfig } from '../lib/config.mjs';
import { getTemplate } from '../lib/templates.mjs';
import { ensureBrowser } from '../lib/browser.mjs';
import { runTemplate, listOutputFiles } from '../lib/runner.mjs';

export function registerRunCommand(program) {
  program
    .command('run <template>')
    .description('Run an automation template')
    .allowUnknownOption(true)
    .action(async (template, _opts, cmd) => {
      // 1. Find project root and load config
      let projectRoot;
      let config;
      try {
        projectRoot = findProjectRoot();
        config = loadConfig(projectRoot);
      } catch (err) {
        console.error(chalk.red(err.message));
        process.exit(1);
      }

      // 2. Find the template (project dir first, then bundled)
      const manifest = getTemplate(template, projectRoot);
      if (!manifest) {
        console.error(
          chalk.red(
            `Template "${template}" not found. Run "odin template list" to see available templates.`
          )
        );
        process.exit(1);
      }

      const requires = manifest.requires || [];

      // 3. Check requirements
      if (requires.includes('browser')) {
        await ensureBrowser(config);
      }

      if (requires.includes('llm')) {
        const apiKey = config?.llm?.api_key || process.env.ODIN_LLM_API_KEY;
        if (!apiKey) {
          console.log('');
          console.log(
            chalk.yellow(
              '  Warning: No LLM API key configured.'
            )
          );
          console.log(
            chalk.yellow(
              '  Templates that require an LLM may fail.'
            )
          );
          console.log(
            chalk.dim(
              '  Set it with: odin config set llm.api_key <your-key>'
            )
          );
          console.log('');
        }
      }

      // 4. Collect args passed after --
      // Commander puts everything after -- into cmd.args
      const templateArgs = cmd.args.slice(1); // first element is the template name

      // 5. Run the template
      console.log('');
      console.log(
        chalk.bold(`Running template: ${chalk.cyan(manifest.name)}`)
      );
      if (manifest.description) {
        console.log(chalk.dim(`  ${manifest.description}`));
      }
      console.log('');

      try {
        const exitCode = await runTemplate(
          manifest._path,
          templateArgs,
          config,
          projectRoot
        );

        console.log('');

        if (exitCode === 0) {
          // Print output summary
          const outputFiles = listOutputFiles(projectRoot);
          if (outputFiles.length > 0) {
            console.log(chalk.bold('Template completed. Output files:'));
            for (const file of outputFiles) {
              console.log(chalk.cyan(`  output/${file}`));
            }
          } else {
            console.log(chalk.bold('Template completed.'));
          }
          console.log('');
          console.log(chalk.green('Done.'));
        } else {
          console.log(
            chalk.red(`Template failed with exit code ${exitCode}.`)
          );
          process.exit(exitCode);
        }
      } catch (err) {
        console.error('');
        console.error(chalk.red(`Error: ${err.message}`));
        process.exit(1);
      }
    });
}
