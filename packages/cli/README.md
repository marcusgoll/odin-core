# @odin-ai/cli

AI agent orchestration platform. Automate anything with multi-agent systems, browser automation, and LLM routing.

## Install

```bash
npm install -g @odin-ai/cli
```

## Quick Start

```bash
# Create a project
odin init my-automations
cd my-automations

# Configure your LLM
odin config set llm.api_key sk-your-key
odin config set llm.provider anthropic

# Add and run a template
odin template add lead-gen
odin run lead-gen -- --target "marketing agencies in Austin"
```

## Templates

| Template | Description | Requirements |
|----------|-------------|--------------|
| lead-gen | Scrape business directories and enrich leads with website data | Browser |
| price-monitor | Monitor competitor prices across multiple retailers | Browser |
| content-ops | Research articles, generate social posts, and build a content calendar | Browser, LLM |
| workflow-automation | Generate n8n lead enrichment workflow with test scenarios and ROI analysis | None |
| hello-world | A simple test template that prints a greeting | None |

```bash
# Browse available templates
odin template list

# See full details for a template
odin template info lead-gen
```

## Commands

- `odin init <name>` -- Create a new project
- `odin run <template>` -- Run an automation template
- `odin config set <key> <value>` -- Set a configuration value
- `odin config get <key>` -- Get a configuration value
- `odin config list` -- List all configuration (secrets redacted)
- `odin template list` -- List available templates
- `odin template add <name>` -- Add a bundled template to the project
- `odin template info <name>` -- Show template details and README
- `odin agent create <name>` -- Create a new agent prompt file
- `odin agent list` -- List all agents in the project
- `odin agent show <name>` -- Display an agent prompt

## Requirements

- Node.js 20+
- Playwright is installed automatically when running browser-based templates

## License

MIT
