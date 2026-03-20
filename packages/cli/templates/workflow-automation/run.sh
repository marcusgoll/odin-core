#!/usr/bin/env bash
# workflow-automation/run.sh — Workflow Automation Template
# Demonstrates end-to-end business workflow automation using n8n.
# Creates a "New Lead -> Enrich -> Score -> Route -> Notify" pipeline.
#
# Environment:
#   ODIN_OUTPUT_DIR — Output directory (default: ./output)
#
# Usage:
#   bash run.sh

set -euo pipefail

###############################################################################
# Usage
###############################################################################
usage() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Demonstrate n8n workflow automation with test scenarios and ROI analysis.

Options:
  -h, --help    Show this help message

Environment Variables:
  ODIN_OUTPUT_DIR    Output directory (default: ./output)

Output:
  lead-enrichment.json    n8n workflow definition (copied to output dir)
  Prints workflow docs, test scenarios, and ROI analysis to stdout
EOF
  exit 0
}

###############################################################################
# Parse Arguments
###############################################################################
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage ;;
    *)         echo "Unknown option: $1" >&2; usage ;;
  esac
done

###############################################################################
# Config
###############################################################################
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="${ODIN_OUTPUT_DIR:-./output}"
WORKFLOW_JSON="${SCRIPT_DIR}/lead-enrichment.json"

mkdir -p "$OUT_DIR"

###############################################################################
# Helpers
###############################################################################
log()     { printf '\033[1;34m[%s]\033[0m %s\n' "$(date '+%H:%M:%S')" "$*"; }
success() { printf '\033[1;32m  OK\033[0m  %s\n' "$*"; }
header()  { printf '\n\033[1;36m%s\033[0m\n' "--- $* ---"; }
dim()     { printf '\033[0;90m%s\033[0m\n' "$*"; }

###############################################################################
# Phase 1: Validate Workflow Definition
###############################################################################
header "Phase 1: Workflow Definition"

if [[ ! -f "${WORKFLOW_JSON}" ]]; then
    echo "ERROR: Workflow JSON not found at ${WORKFLOW_JSON}" >&2
    echo "  This file should be bundled with the template." >&2
    exit 1
fi

# Validate JSON structure
if ! jq empty "${WORKFLOW_JSON}" 2>/dev/null; then
    echo "ERROR: Invalid JSON in ${WORKFLOW_JSON}" >&2
    exit 1
fi

NODE_COUNT=$(jq '.nodes | length' "${WORKFLOW_JSON}")
CONNECTION_COUNT=$(jq '.connections | keys | length' "${WORKFLOW_JSON}")
WORKFLOW_NAME=$(jq -r '.name' "${WORKFLOW_JSON}")

success "Workflow: ${WORKFLOW_NAME}"
success "Nodes: ${NODE_COUNT}"
success "Connection groups: ${CONNECTION_COUNT}"

log "Node inventory:"
jq -r '.nodes[] | "  [\(.type | split(".") | last)] \(.name)"' "${WORKFLOW_JSON}"

log "Flow connections:"
jq -r '.connections | to_entries[] | .key as $from | .value.main[][]? | "  \($from) -> \(.node)"' "${WORKFLOW_JSON}"

# Copy workflow to output directory
cp "${WORKFLOW_JSON}" "$OUT_DIR/lead-enrichment.json"
success "Workflow copied to $OUT_DIR/lead-enrichment.json"

###############################################################################
# Phase 2: Documentation
###############################################################################
header "Phase 2: Workflow Architecture"

cat <<'ARCH'
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
ARCH

echo ""
log "Import instructions:"
echo "  Option A: n8n UI -> Add workflow -> Import from file -> select lead-enrichment.json"
echo "  Option B: n8n REST API (see README.md for curl commands)"

###############################################################################
# Phase 3: Simulated Test Scenarios
###############################################################################
header "Phase 3: Test Scenarios (Simulated)"

log "These curl commands demonstrate how to test the workflow once imported."
echo ""

# --- Test 1: Hot Lead ---
printf '\033[1;33mTest 1: Hot Lead (Professional email + tech company)\033[0m\n'
echo ""
dim "Command:"
cat <<'CURL1'
  curl -s -X POST https://YOUR_N8N_URL/webhook/lead-intake \
    -H 'Content-Type: application/json' \
    -d '{
      "name": "Sarah Chen",
      "email": "sarah@techstartup.com",
      "company": "TechStartup Inc",
      "website": "techstartup.com"
    }' | jq .
CURL1
echo ""

dim "Expected scoring breakdown:"
cat <<'SCORE1'
  Base score (valid data):           +30
  Professional email domain:         +15
  Website content (if substantial):  +10
  Tech industry keywords (SaaS/AI):  +10 to +20
  -----------------------------------------
  Estimated total:                   65-75 (HOT)
SCORE1
echo ""

dim "Expected response:"
cat <<'RESP1'
  {
    "status": "processed",
    "lead": {
      "name": "Sarah Chen",
      "score": 75,
      "tier": "hot",
      "action": "hot_lead_notified"
    }
  }
RESP1
echo ""

dim "Side effect: Telegram notification sent to sales team"
echo ""

# --- Test 2: Nurture Lead ---
printf '\033[1;33mTest 2: Nurture Lead (Free email + small business)\033[0m\n'
echo ""
dim "Command:"
cat <<'CURL2'
  curl -s -X POST https://YOUR_N8N_URL/webhook/lead-intake \
    -H 'Content-Type: application/json' \
    -d '{
      "name": "Bob Smith",
      "email": "bob@gmail.com",
      "company": "Bob Shop",
      "website": "smallshop.com"
    }' | jq .
CURL2
echo ""

dim "Expected scoring breakdown:"
cat <<'SCORE2'
  Base score (valid data):           +30
  Free email provider (gmail):        -5
  Website (likely minimal/empty):    +0 to +5
  No industry keywords:              +0
  -----------------------------------------
  Estimated total:                   25-30 (COLD)
SCORE2
echo ""

dim "Expected response:"
cat <<'RESP2'
  {
    "status": "processed",
    "lead": {
      "name": "Bob Smith",
      "score": 25,
      "tier": "cold",
      "action": "nurture_queued"
    }
  }
RESP2
echo ""

dim "Side effect: Lead logged to nurture queue for batch follow-up"
echo ""

# --- Test 3: Invalid Lead ---
printf '\033[1;33mTest 3: Invalid Lead (Missing required fields)\033[0m\n'
echo ""
dim "Command:"
cat <<'CURL3'
  curl -s -X POST https://YOUR_N8N_URL/webhook/lead-intake \
    -H 'Content-Type: application/json' \
    -d '{
      "name": "Alice",
      "email": ""
    }' | jq .
CURL3
echo ""

dim "Expected response (HTTP 400):"
cat <<'RESP3'
  {
    "status": "error",
    "message": "Missing required fields. Please provide: name, email, company",
    "received": {
      "name": "Alice",
      "email": null,
      "company": null
    }
  }
RESP3
echo ""

# --- Test 4: Enterprise Lead ---
printf '\033[1;33mTest 4: Enterprise Lead (Maximum scoring signals)\033[0m\n'
echo ""
dim "Command:"
cat <<'CURL4'
  curl -s -X POST https://YOUR_N8N_URL/webhook/lead-intake \
    -H 'Content-Type: application/json' \
    -d '{
      "name": "Jennifer Park",
      "email": "jpark@dataforge.ai",
      "company": "DataForge AI",
      "website": "dataforge.ai"
    }' | jq .
CURL4
echo ""

dim "Expected scoring breakdown:"
cat <<'SCORE4'
  Base score (valid data):           +30
  Professional email domain:         +15
  Substantial website content:       +10
  High-value keywords (AI, SaaS):    +20
  Company size indicators:           +10
  Buying intent (pricing/demo):      +10
  -----------------------------------------
  Estimated total:                   95 (HOT)
SCORE4
echo ""

dim "Side effect: Telegram notification with full enrichment details"
echo ""

###############################################################################
# Phase 4: Value Proposition
###############################################################################
header "Phase 4: Value Proposition"

cat <<'VALUE'

  WORKFLOW AUTOMATION ROI — Lead Enrichment Pipeline
  ===================================================

  CURRENT STATE (Manual Process)
  ------------------------------
  - Sales rep receives lead via email/form
  - Manually Googles the company, checks LinkedIn
  - Estimates lead quality based on gut feeling
  - Copies data into CRM, assigns to team member
  - Sends notification via Slack/email
  - Time per lead: ~15-30 minutes
  - Error rate: ~20% (missed signals, inconsistent scoring)

  AUTOMATED STATE (This Workflow)
  -------------------------------
  - Lead arrives via webhook (form, ad, API)
  - Instantly validated, enriched, scored, routed
  - Hot leads: team notified in <5 seconds
  - Nurture leads: queued automatically
  - Time per lead: <5 seconds
  - Error rate: <2% (deterministic scoring)

  BUILD TIME COMPARISON
  ---------------------
  Manual development (developer builds from scratch):
    Research + design:              2-4 hours
    Implementation + testing:       4-8 hours
    Documentation:                  1-2 hours
    Total:                          7-14 hours

  Odin AI Automation Agency:
    Workflow generation:            ~2 minutes
    Customization + review:         ~3 minutes
    Total:                          ~5 minutes

  TIME SAVINGS: 98% faster delivery

  MONTHLY OPERATIONAL SAVINGS
  ---------------------------
  Assumption: 50 leads/month, 20 min avg manual processing

  Manual:    50 leads x 20 min = 16.7 hours/month
  Automated: 50 leads x 0 min = 0 hours/month (fully automated)
  Saved:     16.7 hours/month

  At 200 leads/month:
  Manual:    200 leads x 20 min = 66.7 hours/month
  Saved:     66.7 hours/month

  FINANCIAL ROI
  -------------
  At $50/hr labor cost:
    50 leads/month:   $835/month saved    ($10,020/year)
    200 leads/month:  $3,335/month saved  ($40,020/year)

  At $75/hr labor cost:
    50 leads/month:   $1,253/month saved  ($15,030/year)
    200 leads/month:  $5,003/month saved  ($60,030/year)

  ADDITIONAL BENEFITS
  -------------------
  - Consistent scoring (no human bias or fatigue)
  - 24/7 operation (leads processed at 3 AM)
  - Instant hot lead response (5 sec vs hours)
  - Full audit trail for every lead
  - Easy to extend (CRM, email, Slack integrations)

VALUE

###############################################################################
# Summary
###############################################################################
header "Summary"

log "Files:"
echo "  1. $OUT_DIR/lead-enrichment.json"
echo "     n8n workflow definition (${NODE_COUNT} nodes, ready to import)"
echo ""
echo "  2. $(readlink -f "$0")"
echo "     This template script (validation, test scenarios, ROI analysis)"
echo ""

success "Workflow Automation template complete."
success "Import lead-enrichment.json into n8n to go live."
