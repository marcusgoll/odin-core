# Lead Generation Template

Scrapes business directories (Clutch.co) for agency listings, enriches each lead by visiting their profile and company website, and outputs structured JSON + CSV reports.

## What It Does

1. **Directory Scrape** -- Extracts agency cards from Clutch.co (name, rating, location, services, reviews)
2. **Profile Enrichment** -- Visits each Clutch profile page to extract website URL, employee count, and min project size
3. **Website Enrichment** -- Visits company websites to extract descriptions, contact emails, and founder/CEO names
4. **Report Generation** -- Outputs JSON and CSV files with all collected data

## Prerequisites

- Odin Huginn browser server running (`odin agent start`)
- `jq` installed
- `curl` installed

## Usage

```bash
# Default: scrape digital marketing agencies in Texas
odin run lead-gen

# Custom target (currently uses Clutch.co Texas digital marketing)
odin run lead-gen --target "marketing agencies in Texas"

# Limit enrichment depth
odin run lead-gen --profile-limit 5 --website-limit 3
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ODIN_BROWSER_URL` | `http://127.0.0.1:9227` | Huginn browser server URL |
| `ODIN_BROWSER_TOKEN` | (none) | Optional auth token for browser server |
| `ODIN_OUTPUT_DIR` | `./output` | Directory for output files |

## Output Files

- `leads.json` -- Full structured data for all scraped leads
- `leads.csv` -- CSV export with key fields (name, location, rating, website, email, founder)
