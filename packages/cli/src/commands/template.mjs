import chalk from 'chalk';

export function registerTemplateCommand(program) {
  program
    .command('template <action>')
    .description('Manage templates')
    .action((action) => {
      console.log(chalk.yellow(`odin template "${action}" — coming soon`));
    });
}
