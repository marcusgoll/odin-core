import chalk from 'chalk';

export function registerRunCommand(program) {
  program
    .command('run <template>')
    .description('Run an automation template')
    .action((template) => {
      console.log(chalk.yellow(`odin run "${template}" — coming soon`));
    });
}
