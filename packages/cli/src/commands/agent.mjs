import chalk from 'chalk';

export function registerAgentCommand(program) {
  program
    .command('agent <action>')
    .description('Manage agents')
    .action((action) => {
      console.log(chalk.yellow(`odin agent "${action}" — coming soon`));
    });
}
