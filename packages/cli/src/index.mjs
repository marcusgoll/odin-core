import { createRequire } from 'node:module';
import { Command } from 'commander';
import { registerInitCommand } from './commands/init.mjs';
import { registerRunCommand } from './commands/run.mjs';
import { registerConfigCommand } from './commands/config.mjs';
import { registerTemplateCommand } from './commands/template.mjs';
import { registerAgentCommand } from './commands/agent.mjs';

const require = createRequire(import.meta.url);
const pkg = require('../package.json');

export function main(argv) {
  const program = new Command();

  program
    .name('odin')
    .description('Odin — AI agent orchestration platform')
    .version(pkg.version);

  registerInitCommand(program);
  registerRunCommand(program);
  registerConfigCommand(program);
  registerTemplateCommand(program);
  registerAgentCommand(program);

  program.parse(argv);
}
