import chalk from 'chalk';

export function registerConfigCommand(program) {
  program
    .command('config <action>')
    .description('Manage project configuration')
    .action((action) => {
      console.log(chalk.yellow(`odin config "${action}" — coming soon`));
    });
}
