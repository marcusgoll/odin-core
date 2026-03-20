# Workflow Automation Template

Demonstrates end-to-end business workflow automation using n8n. Includes a production-ready lead enrichment pipeline (webhook intake, validation, website enrichment, scoring, and routing) with test scenarios and ROI analysis.

## What It Does

1. **Workflow Validation** -- Verifies the n8n workflow JSON definition, lists nodes and connections
2. **Documentation** -- Displays the workflow architecture and import instructions
3. **Test Scenarios** -- Prints 4 simulated test cases with expected scoring breakdowns
4. **ROI Analysis** -- Shows build-time comparison and monthly operational savings

## Prerequisites

- `jq` installed
- n8n instance (for importing the workflow -- not required to run the demo)

## Usage

```bash
# Run the demo (prints workflow docs, test scenarios, ROI analysis)
odin run workflow-automation
```

No browser or LLM required -- this template outputs documentation and test scenarios to stdout.

## Included Files

- `run.sh` -- Demo script with validation, test scenarios, and ROI analysis
- `lead-enrichment.json` -- Production-ready n8n workflow (11 nodes, ready to import)

## n8n Workflow: Lead Enrichment Pipeline

```
POST /webhook/lead-intake
        |
   [Validate & Normalize]
        |
   [Valid Lead?] ----NO----> [400: Missing Fields]
        |
       YES
        |
   [Fetch Company Website]
        |
   [Score Lead (1-100)]
        |
   [Hot Lead?] ----NO----> [Queue for Nurture] --> [200: Nurture Response]
        |
       YES
        |
   [Telegram Alert] --> [200: Hot Lead Response]
```

## Scoring Signals

| Signal | Points | Description |
|--------|--------|-------------|
| Base (valid data) | +30 | Lead has name, email, and company |
| Professional email | +15 | Not gmail/yahoo/hotmail/outlook |
| Free email provider | -5 | Using a consumer email service |
| Substantial website | +10 | Website content > 500 characters |
| High-value industry (2+) | +20 | SaaS, AI, fintech, healthcare, etc. |
| Company size indicators | +10 | Mentions team size, offices, global |
| Buying intent signals | +10 | Pricing pages, demo CTAs, contact sales |

## Importing into n8n

```bash
# Via n8n REST API
cat lead-enrichment.json \
  | jq 'del(.tags, .meta, .id, .createdAt, .updatedAt, .active, .versionId)' \
  | curl -s -X POST "${N8N_API_URL}/workflows" \
    -H "X-N8N-API-KEY: ${N8N_API_KEY}" \
    -H "Content-Type: application/json" \
    -d @-
```

Or import via the n8n UI: Add workflow > Import from file > select `lead-enrichment.json`.
