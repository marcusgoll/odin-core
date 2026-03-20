# CLI Quickstart Guide

This guide walks through installing the Odin CLI, creating a project, and running your first automation.

## Prerequisites

- **Node.js 20+** -- check with `node --version`
- **npm** -- ships with Node.js
- **Playwright** (optional) -- auto-installed when you first run a browser template

If you plan to use LLM-powered templates (like `content-ops`), you need an API key from one of the supported providers: OpenAI, Anthropic, or a local Ollama instance.

## Installation

```bash
npm install -g @odin-ai/cli
```

Verify the installation:

```bash
odin --version
```

## Creating a Project

```bash
odin init my-automations
cd my-automations
```

This creates the following project structure:

```
my-automations/
  odin.yaml        # Project configuration
  agents/          # Agent prompt files
  skills/          # Custom skills
  templates/       # Installed templates
  output/          # Template output files
  .gitignore       # Ignores output/, .odin/, node_modules/
```

### odin.yaml structure

The generated `odin.yaml` is the central config file:

```yaml
name: my-automations
version: 0.1.0
llm:
  provider: openai
  model: gpt-4o
  api_key: ""
browser:
  headless: true
  port: 9227
agents: []
templates: []
```

You can edit this file directly, or use `odin config set` to modify values from the command line.

## Configuring LLM Providers

### Anthropic (Claude)

```bash
odin config set llm.provider anthropic
odin config set llm.model claude-sonnet-4-20250514
odin config set llm.api_key sk-ant-your-key
```

### OpenAI

```bash
odin config set llm.provider openai
odin config set llm.model gpt-4o
odin config set llm.api_key sk-your-key
```

### Ollama (local, no API key needed)

```bash
odin config set llm.provider ollama
odin config set llm.model qwen3
odin config set llm.api_key local
```

Ollama must be running locally on port 11434 (default).

### Environment variable

Instead of storing the key in `odin.yaml`, you can set the `ODIN_LLM_API_KEY` environment variable. The CLI checks both locations.

## Running Your First Template (No Dependencies)

The `workflow-automation` template has zero external requirements -- no browser, no LLM key. It generates a ready-to-import n8n workflow definition.

```bash
odin template add workflow-automation
odin run workflow-automation
```

Output is written to `output/lead-enrichment.json`.

Another zero-dependency option is `hello-world`:

```bash
odin template add hello-world
odin run hello-world -- --name "Odin"
```

## Running a Browser Template

Browser templates use Playwright to automate a headless Chromium instance. Playwright is listed as an optional dependency and is installed automatically the first time you run a template that requires it.

### lead-gen

Scrapes business directories and enriches leads with data from company websites:

```bash
odin template add lead-gen
odin run lead-gen -- --target "marketing agencies in Austin"
```

Optional flags:

- `--profile_limit N` -- max number of profile pages to enrich (default: 10)
- `--website_limit N` -- max company websites to visit (default: 5)

Outputs: `output/leads.json` and `output/leads.csv`

### price-monitor

Monitors competitor prices across multiple retailers:

```bash
odin template add price-monitor
odin run price-monitor
```

Pass a custom product config file:

```bash
odin run price-monitor -- --config path/to/products.json
```

Outputs: `output/prices.json` and `output/price-history.json`

### content-ops

Researches articles, generates social media posts, and builds a content calendar. Requires both a browser and an LLM API key.

```bash
odin template add content-ops
odin run content-ops -- --source "techcrunch.com" --count 5
```

Output: `output/content-calendar.json`

## Browsing Templates

List all available templates (bundled and project-local):

```bash
odin template list
```

Get detailed information about a specific template, including its inputs, outputs, and README:

```bash
odin template info lead-gen
```

## Creating Custom Agents

Agents are markdown prompt files that define roles and capabilities for AI agents in your project.

```bash
odin agent create researcher --prompt "Research companies and compile competitive intelligence"
```

This creates `agents/researcher.md` with a structured prompt template you can customize.

List and inspect agents:

```bash
odin agent list
odin agent show researcher
```

## Project Structure Explained

| Path | Purpose |
|------|---------|
| `odin.yaml` | Central configuration -- LLM provider, browser settings, registered agents and templates |
| `agents/` | Agent prompt files (markdown). Each file defines one agent's role, capabilities, workflow, and constraints |
| `templates/` | Installed automation templates. Each has a `manifest.yaml`, `run.sh`, and optional `README.md` |
| `skills/` | Custom reusable skills (reserved for future use) |
| `output/` | All template output is written here. Ignored by `.gitignore` |
| `.odin/` | Internal state directory (created at runtime). Ignored by `.gitignore` |

## Troubleshooting

### "Not in an Odin project"

The CLI looks for `odin.yaml` in the current directory and walks up the tree. Make sure you are inside a project created with `odin init`.

### Playwright fails to install

If the automatic Playwright install hangs or fails behind a corporate proxy:

```bash
npx playwright install chromium
```

This downloads the Chromium binary manually. The CLI will detect it on the next run.

### "No LLM API key configured" warning

Templates that require an LLM (like `content-ops`) check for a key in two places:

1. `llm.api_key` in `odin.yaml`
2. The `ODIN_LLM_API_KEY` environment variable

Set either one:

```bash
odin config set llm.api_key sk-your-key
# or
export ODIN_LLM_API_KEY=sk-your-key
```

### Template not found

If `odin run my-template` fails with "not found", make sure you have added it first:

```bash
odin template add my-template
```

Run `odin template list` to see all available bundled templates.

### Browser template shows blank output

- Check that `browser.headless` is `true` in `odin.yaml` (default)
- Try running with `--website_limit 1` to isolate issues
- Check for network connectivity -- browser templates need internet access to scrape sites

### Port 9227 already in use

If another process is using port 9227, change the port in `odin.yaml`:

```bash
odin config set browser.port 9228
```
