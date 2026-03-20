import chalk from 'chalk';

export function registerInitCommand(program) {
  program
    .command('init <name>')
    .description('Initialize a new Odin project')
    .action((name) => {
      console.log(chalk.yellow(`odin init "${name}" — coming soon`));
    });
}
