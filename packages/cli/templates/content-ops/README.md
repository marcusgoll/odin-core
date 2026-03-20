# Content Operations Template

Automates content research, social media post drafting, and weekly calendar building. Scrapes industry articles, generates X (Twitter) and LinkedIn posts via LLM, and outputs a structured content calendar.

## What It Does

1. **Research** -- Scrapes article links from an industry site, then visits each article to extract title, summary, and key takeaways
2. **Post Drafting** -- Uses an LLM to generate X posts (under 280 chars) and LinkedIn posts (2-3 paragraphs) for each article
3. **Calendar Building** -- Maps posts to a Monday-Friday content schedule
4. **Validation** -- Outputs a formatted report with article verification and sample posts

## Prerequisites

- Odin Huginn browser server running (`odin agent start`)
- LLM API access (Ollama local, or set `ODIN_LLM_API_KEY` for cloud)
- `jq` installed
- `curl` installed

## Usage

```bash
# Default: scrape 3 articles from searchengineland.com
odin run content-ops

# Custom source and count
odin run content-ops --source "blog.hubspot.com/marketing" --count 5
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ODIN_BROWSER_URL` | `http://127.0.0.1:9227` | Huginn browser server URL |
| `ODIN_BROWSER_TOKEN` | (none) | Optional auth token for browser server |
| `ODIN_LLM_URL` | `http://localhost:11434/api/chat` | LLM API endpoint (Ollama-compatible) |
| `ODIN_LLM_MODEL` | `qwen3.5:4b` | LLM model name |
| `ODIN_LLM_API_KEY` | (none) | API key for cloud LLM providers |
| `ODIN_OUTPUT_DIR` | `./output` | Directory for output files |

## Output Files

- `content-calendar.json` -- Full report with articles, generated posts, and weekly calendar

## Content Calendar Layout

| Day | Platform | Content |
|-----|----------|---------|
| Monday | X (Twitter) | Article 1 post |
| Tuesday | LinkedIn | Article 1 post |
| Wednesday | X (Twitter) | Article 2 post |
| Thursday | LinkedIn | Article 2 post |
| Friday | X (Twitter) | Article 3 post |
