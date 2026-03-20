#!/usr/bin/env bash
# content-ops/run.sh — Content Operations Template
# Demonstrates automated content research, drafting, and scheduling.
# Scrapes industry articles, generates social media posts via LLM,
# builds a weekly content calendar, and outputs a validated JSON report.
#
# Environment:
#   ODIN_BROWSER_URL   — Browser server URL (default: http://127.0.0.1:9227)
#   ODIN_BROWSER_TOKEN — Optional auth token for browser server
#   ODIN_LLM_URL       — LLM API endpoint (default: http://localhost:11434/api/chat)
#   ODIN_LLM_MODEL     — LLM model name (default: qwen3.5:4b)
#   ODIN_LLM_API_KEY   — API key for cloud LLM providers (optional)
#   ODIN_OUTPUT_DIR    — Output directory (default: ./output)
#
# Usage:
#   bash run.sh [--source SITE] [--count N]

set -euo pipefail

###############################################################################
# Usage
###############################################################################
usage() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Research articles, generate social posts, and build a content calendar.

Options:
  --source SITE    Website to scrape articles from (default: searchengineland.com)
  --count N        Number of articles to research (default: 3)
  -h, --help       Show this help message

Environment Variables:
  ODIN_BROWSER_URL     Browser server URL (default: http://127.0.0.1:9227)
  ODIN_BROWSER_TOKEN   Optional auth token for browser server
  ODIN_LLM_URL         LLM API endpoint (default: http://localhost:11434/api/chat)
  ODIN_LLM_MODEL       LLM model name (default: qwen3.5:4b)
  ODIN_LLM_API_KEY     API key for cloud LLM providers
  ODIN_OUTPUT_DIR      Output directory (default: ./output)

Output:
  content-calendar.json   Full report with articles, generated posts, and weekly calendar
EOF
  exit 0
}

###############################################################################
# Parse Arguments
###############################################################################
SOURCE_SITE="searchengineland.com"
ARTICLE_COUNT=3

while [[ $# -gt 0 ]]; do
  case "$1" in
    --source)  SOURCE_SITE="$2"; shift 2 ;;
    --count)   ARTICLE_COUNT="$2"; shift 2 ;;
    -h|--help) usage ;;
    *)         echo "Unknown option: $1" >&2; usage ;;
  esac
done

###############################################################################
# Config
###############################################################################
BROWSER_URL="${ODIN_BROWSER_URL:-http://127.0.0.1:9227}"
AUTH_HEADER=""
if [ -n "${ODIN_BROWSER_TOKEN:-}" ]; then
  AUTH_HEADER="Authorization: Bearer $ODIN_BROWSER_TOKEN"
fi
LLM_URL="${ODIN_LLM_URL:-http://localhost:11434/api/chat}"
LLM_MODEL="${ODIN_LLM_MODEL:-qwen3.5:4b}"
LLM_API_KEY="${ODIN_LLM_API_KEY:-}"
OUT_DIR="${ODIN_OUTPUT_DIR:-./output}"
JSON_OUT="$OUT_DIR/content-calendar.json"
NAV_DELAY=3
TMP_JS="/tmp/content-ops-eval-$$.js"

mkdir -p "$OUT_DIR"
trap 'rm -f "$TMP_JS"' EXIT

###############################################################################
# Helpers
###############################################################################
log() { echo "[$(date '+%H:%M:%S')] $*"; }

_curl_browser() {
  local args=("$@")
  if [ -n "$AUTH_HEADER" ]; then
    curl -sf -H "$AUTH_HEADER" "${args[@]}"
  else
    curl -sf "${args[@]}"
  fi
}

browser_navigate() {
  local url="$1"
  _curl_browser -X POST "$BROWSER_URL/navigate" \
    -H "Content-Type: application/json" \
    -d "{\"url\": $(echo "$url" | jq -Rs .)}" >/dev/null 2>&1 || true
}

browser_eval_file() {
  cat > "$TMP_JS"
  local js_encoded
  js_encoded=$(jq -Rs . < "$TMP_JS")
  local resp
  resp=$(_curl_browser -X POST "$BROWSER_URL/evaluate" \
    -H "Content-Type: application/json" \
    -d "{\"fn\": $js_encoded}" 2>/dev/null || echo '{"ok":false}')
  if echo "$resp" | jq -e '.ok' >/dev/null 2>&1; then
    echo "$resp" | jq -r '.result // "null"'
  else
    echo "null"
  fi
}

# Call LLM for text generation. Falls back to template if LLM fails.
llm_generate() {
  local prompt="$1"
  local fallback="$2"

  local headers=(-H "Content-Type: application/json")
  if [ -n "$LLM_API_KEY" ]; then
    headers+=(-H "Authorization: Bearer $LLM_API_KEY")
  fi

  local payload
  payload=$(jq -n \
    --arg model "$LLM_MODEL" \
    --arg prompt "$prompt" \
    '{
      model: $model,
      messages: [{role: "user", content: $prompt}],
      stream: false,
      think: false
    }')

  local resp
  resp=$(curl -sf --max-time 60 "$LLM_URL" \
    "${headers[@]}" \
    -d "$payload" 2>/dev/null || echo "")

  if [ -n "$resp" ]; then
    local content
    content=$(echo "$resp" | jq -r '.message.content // empty' 2>/dev/null)
    if [ -n "$content" ]; then
      echo "$content"
      return 0
    fi
  fi

  log "  WARNING: LLM failed, using template fallback"
  echo "$fallback"
  return 0
}

###############################################################################
# Phase 1: Research — Scrape Industry Articles
###############################################################################
log "=========================================="
log "PHASE 1: Research — Scraping Industry Articles"
log "=========================================="

RESEARCH_URL="https://$SOURCE_SITE"

log "Navigating to $RESEARCH_URL ..."
browser_navigate "$RESEARCH_URL"
sleep "$NAV_DELAY"

log "Extracting article links from homepage..."
ARTICLE_LINKS=$(browser_eval_file <<'JSEOF'
JSON.stringify((() => {
  // Search Engine Land uses slug-based URLs with numeric IDs (e.g., /topic-name-472061)
  const links = [];
  const seen = new Set();

  const allLinks = document.querySelectorAll('a[href]');
  for (const el of allLinks) {
    const href = el.href;
    const title = el.textContent.trim();
    if (href && title && title.length > 15
        && href.match(/searchengineland\.com\/[a-z].*-\d{5,}$/)
        && !seen.has(href)
        && !href.includes('/library/')
        && !href.includes('/guide/')
        && !href.includes('/author/')
        && !title.startsWith('<')) {
      seen.add(href);
      links.push({url: href, title: title.substring(0, 200)});
    }
    if (links.length >= 6) break;
  }
  return links.slice(0, 6);
})())
JSEOF
)

log "Raw article links: $ARTICLE_LINKS"

# Parse article links — try fallback site if primary fails
if [ "$ARTICLE_LINKS" = "null" ] || [ -z "$ARTICLE_LINKS" ]; then
  log "ERROR: Could not extract article links. Trying fallback site..."

  RESEARCH_URL="https://blog.hubspot.com/marketing"
  log "Navigating to $RESEARCH_URL ..."
  browser_navigate "$RESEARCH_URL"
  sleep "$NAV_DELAY"

  ARTICLE_LINKS=$(browser_eval_file <<'JSEOF'
JSON.stringify((() => {
  const links = [];
  const seen = new Set();
  const allLinks = document.querySelectorAll('a[href]');
  for (const el of allLinks) {
    const href = el.href;
    const title = el.textContent.trim();
    if (href && title && title.length > 15 && !seen.has(href)
        && href.match(/blog\.hubspot\.com\/marketing\/[a-z]/)
        && !href.includes('/category/') && !href.includes('/tag/')
        && !href.endsWith('/marketing/') && !href.endsWith('/marketing')
        && !title.startsWith('<')) {
      seen.add(href);
      links.push({url: href, title: title.substring(0, 200)});
    }
    if (links.length >= 6) break;
  }
  return links.slice(0, 6);
})())
JSEOF
  )
fi

# Validate we got something
LINK_COUNT=$(echo "$ARTICLE_LINKS" | jq 'length' 2>/dev/null || echo "0")
if [ "$LINK_COUNT" -lt 1 ]; then
  log "ERROR: No articles found on either site. Cannot proceed."
  exit 1
fi

log "Found $LINK_COUNT candidate articles. Will scrape top $ARTICLE_COUNT."

# Array to collect article data
ARTICLES_JSON="[]"

for i in $(seq 0 $((ARTICLE_COUNT - 1))); do
  ARTICLE_URL=$(echo "$ARTICLE_LINKS" | jq -r ".[$i].url // empty")
  ARTICLE_TITLE_HINT=$(echo "$ARTICLE_LINKS" | jq -r ".[$i].title // empty")

  if [ -z "$ARTICLE_URL" ]; then
    log "Skipping article $((i+1)): no URL found"
    continue
  fi

  log ""
  log "--- Article $((i+1)) of $ARTICLE_COUNT ---"
  log "URL: $ARTICLE_URL"
  log "Navigating..."
  browser_navigate "$ARTICLE_URL"
  sleep "$NAV_DELAY"

  # Extract article content
  ARTICLE_DATA=$(browser_eval_file <<'JSEOF'
JSON.stringify((() => {
  const pageTitle = document.title || '';

  let title = '';
  const h1 = document.querySelector('h1');
  if (h1) title = h1.textContent.trim();
  if (!title) title = pageTitle;

  let metaDesc = '';
  const metaTag = document.querySelector('meta[name="description"]') || document.querySelector('meta[property="og:description"]');
  if (metaTag) metaDesc = metaTag.getAttribute('content') || '';

  let summary = '';
  const paragraphs = document.querySelectorAll('article p, .entry-content p, .post-content p, .article-body p, main p');
  const goodParagraphs = [];
  for (const p of paragraphs) {
    const text = p.textContent.trim();
    if (text.length > 50 && !text.startsWith('\u00a9') && !text.includes('cookie') && !text.includes('newsletter')) {
      goodParagraphs.push(text);
      if (goodParagraphs.length >= 2) break;
    }
  }
  summary = goodParagraphs.join(' ').substring(0, 500);

  if (!summary && metaDesc) summary = metaDesc;

  let snippet = '';
  if (goodParagraphs.length > 0) {
    snippet = goodParagraphs[0].substring(0, 200);
  } else if (metaDesc) {
    snippet = metaDesc.substring(0, 200);
  }

  let takeaway = '';
  const boldElements = document.querySelectorAll('article strong, .entry-content strong, .post-content strong');
  for (const b of boldElements) {
    const text = b.textContent.trim();
    if (text.length > 20 && text.length < 200) {
      takeaway = text;
      break;
    }
  }
  if (!takeaway && summary) {
    const firstSentence = summary.match(/^[^.!?]+[.!?]/);
    takeaway = firstSentence ? firstSentence[0] : summary.substring(0, 150);
  }

  return {
    page_title: pageTitle.substring(0, 300),
    title: title.substring(0, 300),
    url: window.location.href,
    meta_description: metaDesc.substring(0, 500),
    summary: summary,
    snippet: snippet,
    takeaway: takeaway
  };
})())
JSEOF
  )

  if [ "$ARTICLE_DATA" = "null" ] || [ -z "$ARTICLE_DATA" ]; then
    log "  WARNING: Could not extract data from article $((i+1)), using hint"
    ARTICLE_DATA=$(jq -n \
      --arg title "$ARTICLE_TITLE_HINT" \
      --arg url "$ARTICLE_URL" \
      '{
        page_title: $title,
        title: $title,
        url: $url,
        meta_description: "",
        summary: "Content could not be extracted from this page.",
        snippet: "",
        takeaway: ""
      }')
  fi

  # Parse and display
  A_TITLE=$(echo "$ARTICLE_DATA" | jq -r '.title')
  A_URL=$(echo "$ARTICLE_DATA" | jq -r '.url')
  A_SUMMARY=$(echo "$ARTICLE_DATA" | jq -r '.summary')
  A_SNIPPET=$(echo "$ARTICLE_DATA" | jq -r '.snippet')
  A_TAKEAWAY=$(echo "$ARTICLE_DATA" | jq -r '.takeaway')
  A_PAGE_TITLE=$(echo "$ARTICLE_DATA" | jq -r '.page_title')

  log "  Title: $A_TITLE"
  log "  Summary: ${A_SUMMARY:0:120}..."
  log "  Takeaway: ${A_TAKEAWAY:0:100}"

  ARTICLES_JSON=$(echo "$ARTICLES_JSON" | jq \
    --arg title "$A_TITLE" \
    --arg url "$A_URL" \
    --arg summary "$A_SUMMARY" \
    --arg snippet "$A_SNIPPET" \
    --arg takeaway "$A_TAKEAWAY" \
    --arg page_title "$A_PAGE_TITLE" \
    '. + [{
      title: $title,
      url: $url,
      summary: $summary,
      snippet: $snippet,
      takeaway: $takeaway,
      page_title: $page_title,
      posts: {}
    }]')
done

SCRAPED_COUNT=$(echo "$ARTICLES_JSON" | jq 'length')
log ""
log "Phase 1 complete: scraped $SCRAPED_COUNT articles"

###############################################################################
# Phase 2: Generate Post Drafts via LLM
###############################################################################
log ""
log "=========================================="
log "PHASE 2: Generating Social Media Post Drafts"
log "=========================================="

POSTS_GENERATED=0

for i in $(seq 0 $((SCRAPED_COUNT - 1))); do
  A_TITLE=$(echo "$ARTICLES_JSON" | jq -r ".[$i].title")
  A_SUMMARY=$(echo "$ARTICLES_JSON" | jq -r ".[$i].summary")
  A_TAKEAWAY=$(echo "$ARTICLES_JSON" | jq -r ".[$i].takeaway")

  log ""
  log "--- Generating posts for Article $((i+1)): ${A_TITLE:0:60} ---"

  # Generate X post (under 280 chars)
  log "  Generating X post..."
  X_PROMPT="Based on this article: ${A_TITLE}. Summary: ${A_SUMMARY:0:300}. Key takeaway: ${A_TAKEAWAY:0:150}. Write a single X (Twitter) post about the key insight. STRICT RULES: Must be under 280 characters total. Be punchy, conversational, and add value. Include one relevant emoji. Do NOT use hashtags. Output ONLY the post text, nothing else."
  X_FALLBACK="Key insight from \"${A_TITLE:0:80}\": ${A_TAKEAWAY:0:120}. This matters for anyone in digital marketing. What do you think?"
  X_POST=$(llm_generate "$X_PROMPT" "$X_FALLBACK")

  # Clean up: take first line if multi-line, trim to 280 chars
  X_POST=$(echo "$X_POST" | head -1 | sed 's/^["'"'"']//;s/["'"'"']$//' | cut -c1-280)
  log "  X post (${#X_POST} chars): ${X_POST:0:100}..."

  # Generate LinkedIn post (2-3 paragraphs)
  log "  Generating LinkedIn post..."
  LI_PROMPT="Based on this article: ${A_TITLE}. Summary: ${A_SUMMARY:0:300}. Key takeaway: ${A_TAKEAWAY:0:150}. Write a LinkedIn post about this topic. RULES: 2-3 short paragraphs, professional but conversational tone, add your own opinion or analysis, include a question at the end to encourage engagement. Do NOT start with 'I just read' or 'Check out'. Do NOT use hashtags. Output ONLY the post text."
  LI_FALLBACK="The latest from the digital marketing world: ${A_TITLE}.

${A_SUMMARY:0:200}

The key takeaway here is clear: ${A_TAKEAWAY:0:150}. For SaaS companies and digital marketers, this is worth paying attention to. The landscape keeps shifting, and staying ahead means adapting fast.

What strategies are you using to stay competitive in this space?"
  LI_POST=$(llm_generate "$LI_PROMPT" "$LI_FALLBACK")

  # Clean up LinkedIn post
  LI_POST=$(echo "$LI_POST" | sed 's/^["'"'"']//;s/["'"'"']$//')

  log "  LinkedIn post (${#LI_POST} chars): ${LI_POST:0:100}..."

  # Update articles JSON with posts
  ARTICLES_JSON=$(echo "$ARTICLES_JSON" | jq \
    --arg idx "$i" \
    --arg x_post "$X_POST" \
    --arg li_post "$LI_POST" \
    '.[$idx | tonumber].posts = {x: $x_post, linkedin: $li_post}')

  POSTS_GENERATED=$((POSTS_GENERATED + 2))
done

log ""
log "Phase 2 complete: generated $POSTS_GENERATED posts"

###############################################################################
# Phase 3: Create Content Calendar (Mon-Fri)
###############################################################################
log ""
log "=========================================="
log "PHASE 3: Building Content Calendar"
log "=========================================="

# Calculate next Monday
NEXT_MONDAY=$(date -d "next Monday" '+%Y-%m-%d' 2>/dev/null || date -d "+$((8 - $(date +%u))) days" '+%Y-%m-%d' 2>/dev/null || date '+%Y-%m-%d')

CALENDAR_JSON="[]"
DAYS=("Monday" "Tuesday" "Wednesday" "Thursday" "Friday")
DATES=()

for d in 0 1 2 3 4; do
  DAY_DATE=$(date -d "$NEXT_MONDAY + $d days" '+%Y-%m-%d' 2>/dev/null || echo "$NEXT_MONDAY")
  DATES+=("$DAY_DATE")
done

# Build calendar entries:
# Monday:    Article 1 - X post
# Tuesday:   Article 1 - LinkedIn post
# Wednesday: Article 2 - X post
# Thursday:  Article 2 - LinkedIn post
# Friday:    Article 3 - X post
SCHEDULE_MAP=(
  "0:x:Monday"
  "0:linkedin:Tuesday"
  "1:x:Wednesday"
  "1:linkedin:Thursday"
  "2:x:Friday"
)

for entry in "${SCHEDULE_MAP[@]}"; do
  IFS=':' read -r art_idx platform day <<< "$entry"
  day_idx=0
  for di in "${!DAYS[@]}"; do
    if [ "${DAYS[$di]}" = "$day" ]; then
      day_idx=$di
      break
    fi
  done

  ART_TITLE=$(echo "$ARTICLES_JSON" | jq -r ".[$art_idx].title // \"Article $((art_idx+1))\"")
  POST_CONTENT=$(echo "$ARTICLES_JSON" | jq -r ".[$art_idx].posts.$platform // \"[Draft pending]\"")

  CALENDAR_JSON=$(echo "$CALENDAR_JSON" | jq \
    --arg day "$day" \
    --arg date "${DATES[$day_idx]}" \
    --arg platform "$platform" \
    --arg article "$ART_TITLE" \
    --arg article_idx "$art_idx" \
    --arg content "$POST_CONTENT" \
    '. + [{
      day: $day,
      date: $date,
      platform: $platform,
      article_title: $article,
      article_index: ($article_idx | tonumber),
      content: $content
    }]')

  log "  $day ($platform): ${ART_TITLE:0:60}"
done

log ""
log "Phase 3 complete: 5-day content calendar built"

###############################################################################
# Phase 4: Output Report
###############################################################################
log ""
log "=========================================="
log "PHASE 4: Generating Output Report"
log "=========================================="

GENERATED_AT=$(date -u '+%Y-%m-%dT%H:%M:%SZ')

REPORT_JSON=$(jq -n \
  --arg generated_at "$GENERATED_AT" \
  --argjson articles_researched "$SCRAPED_COUNT" \
  --argjson posts_drafted "$POSTS_GENERATED" \
  --argjson calendar "$CALENDAR_JSON" \
  --argjson articles "$ARTICLES_JSON" \
  '{
    generated_at: $generated_at,
    articles_researched: $articles_researched,
    posts_drafted: $posts_drafted,
    calendar: $calendar,
    articles: $articles
  }')

echo "$REPORT_JSON" | jq '.' > "$JSON_OUT"
log "Report saved to $JSON_OUT"

###############################################################################
# Phase 5: Validation + Formatted Summary
###############################################################################
log ""
log "=========================================="
log "PHASE 5: Validation & Summary"
log "=========================================="

echo ""
echo "============================================================"
echo "  CONTENT OPERATIONS — RESULTS"
echo "============================================================"
echo ""
echo "Generated: $GENERATED_AT"
echo "Articles Researched: $SCRAPED_COUNT"
echo "Posts Drafted: $POSTS_GENERATED"
echo ""

echo "------------------------------------------------------------"
echo "  ARTICLE VALIDATION"
echo "------------------------------------------------------------"
for i in $(seq 0 $((SCRAPED_COUNT - 1))); do
  echo ""
  echo "  Article $((i+1)):"
  echo "    Page Title: $(echo "$ARTICLES_JSON" | jq -r ".[$i].page_title")"
  echo "    URL:        $(echo "$ARTICLES_JSON" | jq -r ".[$i].url")"
  SNIPPET=$(echo "$ARTICLES_JSON" | jq -r ".[$i].snippet")
  if [ -n "$SNIPPET" ] && [ "$SNIPPET" != "null" ] && [ "$SNIPPET" != "" ]; then
    echo "    Snippet:    \"${SNIPPET:0:150}...\""
    echo "    Status:     VERIFIED (real content extracted)"
  else
    echo "    Snippet:    [none extracted]"
    echo "    Status:     PARTIAL (title/URL confirmed, content extraction limited)"
  fi
done

echo ""
echo "------------------------------------------------------------"
echo "  WEEKLY CONTENT CALENDAR"
echo "------------------------------------------------------------"
echo ""

CALENDAR_LEN=$(echo "$CALENDAR_JSON" | jq 'length')
for i in $(seq 0 $((CALENDAR_LEN - 1))); do
  CAL_DAY=$(echo "$CALENDAR_JSON" | jq -r ".[$i].day")
  CAL_DATE=$(echo "$CALENDAR_JSON" | jq -r ".[$i].date")
  CAL_PLATFORM=$(echo "$CALENDAR_JSON" | jq -r ".[$i].platform")
  CAL_ARTICLE=$(echo "$CALENDAR_JSON" | jq -r ".[$i].article_title")

  PLATFORM_LABEL="X (Twitter)"
  [ "$CAL_PLATFORM" = "linkedin" ] && PLATFORM_LABEL="LinkedIn"

  echo "  $CAL_DAY ($CAL_DATE) — $PLATFORM_LABEL"
  echo "    Article: ${CAL_ARTICLE:0:70}"
  echo ""
done

echo "------------------------------------------------------------"
echo "  SAMPLE POSTS"
echo "------------------------------------------------------------"

for i in $(seq 0 $((SCRAPED_COUNT - 1))); do
  A_TITLE=$(echo "$ARTICLES_JSON" | jq -r ".[$i].title")
  X_POST=$(echo "$ARTICLES_JSON" | jq -r ".[$i].posts.x")
  LI_POST=$(echo "$ARTICLES_JSON" | jq -r ".[$i].posts.linkedin")

  echo ""
  echo "  --- Article $((i+1)): ${A_TITLE:0:65} ---"
  echo ""
  echo "  [X Post] (${#X_POST} chars)"
  echo "  $X_POST"
  echo ""
  echo "  [LinkedIn Post]"
  echo "$LI_POST" | sed 's/^/  /'
  echo ""
done

echo "============================================================"
echo "  Report: $JSON_OUT"
echo "============================================================"

log ""
log "Content Operations complete."
