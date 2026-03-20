import chalk from 'chalk';
import YAML from 'yaml';
import {
  findProjectRoot,
  loadConfig,
  saveConfig,
  getConfigValue,
  setConfigValue,
  redactSecrets,
} from '../lib/config.mjs';

/** Check if a key name looks like it holds a secret */
function isSecretKey(key) {
  const lower = key.toLowerCase();
  return ['key', 'secret', 'token', 'password'].some((pat) => lower.includes(pat));
}

/** Redact a value for display */
function redactValue(val) {
  const str = String(val);
  if (str.length <= 3) return '...****';
  return str.slice(0, 3) + '...****';
}

export function registerConfigCommand(program) {
  const configCmd = program
    .command('config')
    .description('Manage project configuration');

  configCmd
    .command('set <key> <value>')
    .description('Set a configuration value by dot path')
    .action((key, value) => {
      try {
        const root = findProjectRoot();
        const config = loadConfig(root);
        setConfigValue(config, key, value);
        saveConfig(root, config);

        const display = isSecretKey(key) ? redactValue(value) : value;
        console.log(chalk.green(`Set ${key} = ${display}`));
      } catch (err) {
        console.error(chalk.red(err.message));
        process.exit(1);
      }
    });

  configCmd
    .command('get <key>')
    .description('Get a configuration value by dot path')
    .action((key) => {
      try {
        const root = findProjectRoot();
        const config = loadConfig(root);
        const value = getConfigValue(config, key);

        if (value === undefined) {
          console.error(chalk.red(`Key "${key}" not found in config.`));
          process.exit(1);
        }

        if (typeof value === 'object' && value !== null) {
          console.log(YAML.stringify(redactSecrets({ [key]: value })).trim());
        } else {
          const display = isSecretKey(key) ? redactValue(value) : String(value);
          console.log(display);
        }
      } catch (err) {
        console.error(chalk.red(err.message));
        process.exit(1);
      }
    });

  configCmd
    .command('list')
    .description('List all configuration values (secrets redacted)')
    .action(() => {
      try {
        const root = findProjectRoot();
        const config = loadConfig(root);
        const redacted = redactSecrets(config);
        console.log(YAML.stringify(redacted).trim());
      } catch (err) {
        console.error(chalk.red(err.message));
        process.exit(1);
      }
    });
}
