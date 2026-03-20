#!/usr/bin/env bash
# price-monitor/run.sh — Competitor Price Monitoring Template
# Scrapes product prices across multiple retailers using the Huginn browser,
# generates comparison reports, and simulates daily monitoring with alerts.
#
# Environment:
#   ODIN_BROWSER_URL   — Browser server URL (default: http://127.0.0.1:9227)
#   ODIN_BROWSER_TOKEN — Optional auth token for browser server
#   ODIN_OUTPUT_DIR    — Output directory (default: ./output)
#
# Usage:
#   bash run.sh [--config products.json]

set -euo pipefail

###############################################################################
# Usage
###############################################################################
usage() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Monitor competitor prices across multiple retailers.

Options:
  --config FILE    Path to products JSON config (uses built-in demo products if omitted)
  -h, --help       Show this help message

Environment Variables:
  ODIN_BROWSER_URL     Browser server URL (default: http://127.0.0.1:9227)
  ODIN_BROWSER_TOKEN   Optional auth token for browser server
  ODIN_OUTPUT_DIR      Output directory (default: ./output)

Output:
  prices.json          Raw scrape results with price, availability, and validation data
  price-history.json   Historical price snapshots and simulated alerts
EOF
  exit 0
}

###############################################################################
# Parse Arguments
###############################################################################
CONFIG_FILE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --config)  CONFIG_FILE="$2"; shift 2 ;;
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
OUT_DIR="${ODIN_OUTPUT_DIR:-./output}"
JSON_OUT="$OUT_DIR/prices.json"
HISTORY_OUT="$OUT_DIR/price-history.json"
NAV_DELAY=5
PAGE_LOAD_WAIT=5
TMP_JS="/tmp/pricemon-eval-$$.js"

mkdir -p "$OUT_DIR"
trap 'rm -f "$TMP_JS"' EXIT

###############################################################################
# Helpers
###############################################################################
log() { echo "[$(date '+%H:%M:%S')] $*"; }

_curl_browser() {
  local args=("$@")
  if [ -n "$AUTH_HEADER" ]; then
    curl -s -H "$AUTH_HEADER" "${args[@]}"
  else
    curl -s "${args[@]}"
  fi
}

ensure_browser() {
  local health
  health=$(_curl_browser "$BROWSER_URL/health" 2>/dev/null || echo '{}')
  if ! echo "$health" | jq -e '.ok' >/dev/null 2>&1; then
    log "  Browser server not responding"
    return 1
  fi
  local has_browser
  has_browser=$(echo "$health" | jq -r '.browser // false')
  if [ "$has_browser" != "true" ]; then
    log "  Relaunching browser..."
    _curl_browser -X POST "$BROWSER_URL/launch" \
      -H "Content-Type: application/json" \
      -d '{"headless": true, "url": "about:blank"}' >/dev/null 2>&1
    sleep 2
  fi
  return 0
}

browser_navigate() {
  local url="$1"
  ensure_browser || return 1
  local resp
  resp=$(_curl_browser -X POST "$BROWSER_URL/navigate" \
    -H "Content-Type: application/json" \
    -d "{\"url\": $(echo "$url" | jq -Rs .)}" 2>&1)
  if [ -z "$resp" ]; then
    echo '{"error":"empty response from browser"}'
    return 1
  fi
  if echo "$resp" | jq -e '.error' >/dev/null 2>&1; then
    echo "$resp"
    return 1
  fi
  # Check for bot detection in the page title
  local title
  title=$(echo "$resp" | jq -r '.title // ""')
  if [[ "$title" == *"Robot or human"* ]] || [[ "$title" == *"Access Denied"* ]] || [[ "$title" == *"Just a moment"* ]]; then
    echo "{\"error\":\"bot detection: $title\"}"
    return 1
  fi
  echo "$resp"
  return 0
}

browser_eval() {
  local js="$1"
  echo "$js" > "$TMP_JS"
  local js_encoded
  js_encoded=$(jq -Rs . < "$TMP_JS")
  local resp
  resp=$(_curl_browser -X POST "$BROWSER_URL/evaluate" \
    -H "Content-Type: application/json" \
    -d "{\"fn\": $js_encoded}" 2>/dev/null)
  [ -z "$resp" ] && resp='{"ok":false}'
  if echo "$resp" | jq -e '.ok' >/dev/null 2>&1; then
    echo "$resp" | jq -r '.result // "null"'
  else
    echo "null"
  fi
}

parse_price() {
  echo "$1" | grep -oP '\d+\.\d{2}' | head -1
}

###############################################################################
# Launch browser
###############################################################################
log "=== Competitor Price Monitoring ==="
log ""
log "Launching headless browser..."

_curl_browser -X POST "$BROWSER_URL/stop" -H "Content-Type: application/json" >/dev/null 2>&1
sleep 1

launch_resp=$(_curl_browser -X POST "$BROWSER_URL/launch" \
  -H "Content-Type: application/json" \
  -d '{"headless": true, "url": "about:blank"}' 2>&1)
if ! echo "$launch_resp" | jq -e '.ok' >/dev/null 2>&1; then
  log "ERROR: Failed to launch browser: $launch_resp"
  exit 1
fi
log "Browser launched successfully"

###############################################################################
# Phase 1: Define Products & Retailers
###############################################################################
log ""
log "=== PHASE 1: Product & Retailer Configuration ==="

# 5 popular consumer electronics products with URLs for 3 retailers.
declare -A PRODUCT_NAMES
declare -A PRODUCT_AMAZON
declare -A PRODUCT_WALMART
declare -A PRODUCT_BESTBUY

PRODUCT_IDS=("airpods_pro_2" "sony_wh1000xm5" "jbl_flip6" "apple_watch_se" "galaxy_buds_fe")

PRODUCT_NAMES[airpods_pro_2]="Apple AirPods Pro 2 (USB-C)"
PRODUCT_AMAZON[airpods_pro_2]="https://www.amazon.com/dp/B0D1XD1ZV3"
PRODUCT_WALMART[airpods_pro_2]="https://www.walmart.com/ip/Apple-AirPods-Pro-2nd-Generation-with-MagSafe-Case-USB-C/5689919121"
PRODUCT_BESTBUY[airpods_pro_2]="https://www.bestbuy.com/site/apple-airpods-pro-2nd-generation-with-magsafe-case-usb-c-white/6447382.p"

PRODUCT_NAMES[sony_wh1000xm5]="Sony WH-1000XM5 Headphones"
PRODUCT_AMAZON[sony_wh1000xm5]="https://www.amazon.com/dp/B09XS7JWHH"
PRODUCT_WALMART[sony_wh1000xm5]="https://www.walmart.com/ip/Sony-WH-1000XM5-Wireless-Noise-Canceling-Headphones-Black/473912557"
PRODUCT_BESTBUY[sony_wh1000xm5]="https://www.bestbuy.com/site/sony-wh-1000xm5-wireless-noise-canceling-over-the-ear-headphones-black/6505727.p"

PRODUCT_NAMES[jbl_flip6]="JBL Flip 6 Bluetooth Speaker"
PRODUCT_AMAZON[jbl_flip6]="https://www.amazon.com/dp/B0FDY3ML2H"
PRODUCT_WALMART[jbl_flip6]="https://www.walmart.com/ip/JBL-Flip-6-Portable-Waterproof-Speaker-Black/460059654"
PRODUCT_BESTBUY[jbl_flip6]="https://www.bestbuy.com/site/jbl-flip-6-portable-waterproof-speaker-black/6484647.p"

PRODUCT_NAMES[apple_watch_se]="Apple Watch SE (2nd Gen, 40mm)"
PRODUCT_AMAZON[apple_watch_se]="https://www.amazon.com/dp/B0DGJ736JM"
PRODUCT_WALMART[apple_watch_se]="https://www.walmart.com/ip/Apple-Watch-SE-2nd-Gen-GPS-40mm-Starlight-Aluminum-Case-with-Starlight-Sport-Band-S-M/5033741510"
PRODUCT_BESTBUY[apple_watch_se]="https://www.bestbuy.com/site/apple-watch-se-2nd-generation-gps-40mm-starlight-aluminum-case-with-starlight-sport-band-s-m/6340234.p"

PRODUCT_NAMES[galaxy_buds_fe]="Samsung Galaxy Buds FE"
PRODUCT_AMAZON[galaxy_buds_fe]="https://www.amazon.com/dp/B0CL8CJPVM"
PRODUCT_WALMART[galaxy_buds_fe]="https://www.walmart.com/ip/Samsung-Galaxy-Buds-FE-Graphite/2546078498"
PRODUCT_BESTBUY[galaxy_buds_fe]="https://www.bestbuy.com/site/samsung-galaxy-buds-fe-true-wireless-earbud-headphones-graphite/6557892.p"

RETAILER_IDS=("amazon" "walmart" "bestbuy")
declare -A RETAILER_DISPLAY
RETAILER_DISPLAY[amazon]="Amazon"
RETAILER_DISPLAY[walmart]="Walmart"
RETAILER_DISPLAY[bestbuy]="Best Buy"

log "Configured ${#PRODUCT_IDS[@]} products across ${#RETAILER_IDS[@]} retailers"
log "Products: ${PRODUCT_IDS[*]}"
log "Retailers: Amazon, Walmart, Best Buy"

###############################################################################
# Phase 2: Scrape Current Prices
###############################################################################
log ""
log "=== PHASE 2: Price Scraping ==="

RESULTS="[]"
SCRAPE_COUNT=0
ERROR_COUNT=0
declare -A BLOCKED_RETAILERS_MAP

# Amazon scraper
scrape_amazon() {
  browser_eval 'JSON.stringify((() => {
  var result = {
    retailer: "Amazon",
    validation_title: document.title || "",
    price: null,
    availability: "unknown",
    validation_context: ""
  };
  var priceEls = document.querySelectorAll("span.a-price span.a-offscreen");
  if (priceEls.length > 0) {
    result.price = priceEls[0].textContent.trim();
  }
  if (!result.price) {
    var pb = document.querySelector("#priceblock_ourprice, #priceblock_dealprice, #corePrice_feature_div .a-offscreen");
    if (pb) result.price = pb.textContent.trim();
  }
  var avail = document.querySelector("#availability span");
  if (avail) {
    var at = avail.textContent.trim().toLowerCase();
    if (at.includes("in stock")) result.availability = "In Stock";
    else if (at.includes("unavailable") || at.includes("out of stock")) result.availability = "Out of Stock";
    else result.availability = avail.textContent.trim().substring(0, 40);
  }
  if (result.availability === "unknown") {
    var addCart = document.querySelector("#add-to-cart-button, input[name=\"submit.add-to-cart\"]");
    result.availability = addCart ? "In Stock" : "unknown";
  }
  var h1 = document.querySelector("#productTitle, #title span");
  result.validation_context = h1 ? h1.textContent.trim().substring(0, 120) : "";
  return result;
})())'
}

# Walmart scraper
scrape_walmart() {
  browser_eval 'JSON.stringify((() => {
  var result = {
    retailer: "Walmart",
    validation_title: document.title || "",
    price: null,
    availability: "unknown",
    validation_context: ""
  };
  if (document.title.includes("Robot") || document.title.includes("blocked")) {
    result.error = "bot_detected";
    return result;
  }
  var priceEl = document.querySelector("[itemprop=\"price\"]");
  if (priceEl) {
    result.price = priceEl.content || priceEl.textContent.trim();
  }
  if (!result.price) {
    var spans = document.querySelectorAll("span");
    for (var i = 0; i < spans.length; i++) {
      var t = spans[i].textContent.trim();
      if (/^(Now |current price )?\$\d+\.\d{2}$/.test(t) && spans[i].children.length === 0) {
        result.price = t.replace(/^(Now |current price )/i, "").trim();
        break;
      }
    }
  }
  var heading = document.querySelector("h1[itemprop=\"name\"], h1");
  result.validation_context = heading ? heading.textContent.trim().substring(0, 120) : "";
  return result;
})())'
}

# Best Buy scraper
scrape_bestbuy() {
  browser_eval 'JSON.stringify((() => {
  var result = {
    retailer: "Best Buy",
    validation_title: document.title || "",
    price: null,
    availability: "unknown",
    validation_context: ""
  };
  var priceEl = document.querySelector(".priceView-customer-price span, .priceView-hero-price span");
  if (priceEl) {
    result.price = priceEl.textContent.trim();
  }
  if (!result.price) {
    var spans = document.querySelectorAll("span");
    for (var i = 0; i < spans.length; i++) {
      var t = spans[i].textContent.trim();
      if (/^\$\d+\.\d{2}$/.test(t) && spans[i].children.length === 0) {
        result.price = t;
        break;
      }
    }
  }
  var heading = document.querySelector(".sku-title h1, h1");
  result.validation_context = heading ? heading.textContent.trim().substring(0, 120) : "";
  return result;
})())'
}

# Record a scrape result into the RESULTS array
record_result() {
  local pid="$1" pname="$2" retailer="$3" url="$4"
  local price="${5:-}" avail="${6:-unknown}" vtitle="${7:-}" vcontext="${8:-}" error="${9:-}"

  local entry
  entry=$(jq -n \
    --arg pid "$pid" \
    --arg pname "$pname" \
    --arg retailer "$retailer" \
    --arg url "$url" \
    --arg price "$price" \
    --arg avail "$avail" \
    --arg vtitle "$vtitle" \
    --arg vcontext "$vcontext" \
    --arg error "$error" \
    --arg ts "$(date -Iseconds)" \
    '{
      product_id: $pid,
      product_name: $pname,
      retailer: $retailer,
      url: $url,
      price: (if $price == "" then null else $price end),
      availability: $avail,
      validation_title: $vtitle,
      validation_context: $vcontext,
      error: (if $error == "" then null else $error end),
      scraped_at: $ts
    }')
  RESULTS=$(echo "$RESULTS" | jq --argjson e "$entry" '. + [$e]')
}

# Check if a Best Buy page shows the expected product
validate_bestbuy_product() {
  local expected_name="$1"
  local page_context="$2"
  local key_words
  key_words=$(echo "$expected_name" | tr '[:upper:]' '[:lower:]' | grep -oP '[a-z]{3,}' | head -3)
  local match_count=0
  local total_words=0
  for word in $key_words; do
    total_words=$((total_words + 1))
    if echo "$page_context" | tr '[:upper:]' '[:lower:]' | grep -q "$word"; then
      match_count=$((match_count + 1))
    fi
  done
  local threshold=2
  [ "$total_words" -lt 2 ] && threshold="$total_words"
  [ "$match_count" -ge "$threshold" ]
}

# Main scraping loop
for pid in "${PRODUCT_IDS[@]}"; do
  pname="${PRODUCT_NAMES[$pid]}"
  log ""
  log "--- Scraping: $pname ---"

  for rid in "${RETAILER_IDS[@]}"; do
    retailer_display="${RETAILER_DISPLAY[$rid]}"

    case "$rid" in
      amazon)  url="${PRODUCT_AMAZON[$pid]}" ;;
      walmart) url="${PRODUCT_WALMART[$pid]}" ;;
      bestbuy) url="${PRODUCT_BESTBUY[$pid]}" ;;
    esac

    log "  [$retailer_display] Navigating..."

    nav_ok=0
    nav_result=$(browser_navigate "$url" 2>&1) || nav_ok=$?

    if [ $nav_ok -ne 0 ]; then
      error_msg=$(echo "$nav_result" | jq -r '.error // "navigation failed"' 2>/dev/null || echo "navigation failed")
      log "  [$retailer_display] BLOCKED: $error_msg"
      ERROR_COUNT=$((ERROR_COUNT + 1))
      BLOCKED_RETAILERS_MAP[$retailer_display]=1
      record_result "$pid" "$pname" "$retailer_display" "$url" "" "blocked" "" "" "$error_msg"
      sleep "$NAV_DELAY"
      continue
    fi

    sleep "$PAGE_LOAD_WAIT"

    # Post-navigation bot detection check
    page_title=""
    page_title=$(browser_eval 'document.title' 2>/dev/null) || true
    if [[ "$page_title" == *"Robot or human"* ]] || [[ "$page_title" == *"Access Denied"* ]]; then
      log "  [$retailer_display] BLOCKED: bot detection on page"
      ERROR_COUNT=$((ERROR_COUNT + 1))
      BLOCKED_RETAILERS_MAP[$retailer_display]=1
      record_result "$pid" "$pname" "$retailer_display" "$url" "" "blocked" "$page_title" "" "bot detection after navigation"
      sleep "$NAV_DELAY"
      continue
    fi

    if [[ "$page_title" == "Page Not Found" ]] || [[ "$page_title" == "404"* ]]; then
      log "  [$retailer_display] ERROR: product page not found (404)"
      ERROR_COUNT=$((ERROR_COUNT + 1))
      record_result "$pid" "$pname" "$retailer_display" "$url" "" "not_found" "$page_title" "" "404 page not found"
      sleep "$NAV_DELAY"
      continue
    fi

    # Run the appropriate scraper
    log "  [$retailer_display] Extracting price data..."
    scrape_result=""
    case "$rid" in
      amazon)  scrape_result=$(scrape_amazon) || true ;;
      walmart) scrape_result=$(scrape_walmart) || true ;;
      bestbuy) scrape_result=$(scrape_bestbuy) || true ;;
    esac

    if [ "$scrape_result" = "null" ] || [ -z "$scrape_result" ]; then
      log "  [$retailer_display] Failed to extract data"
      ERROR_COUNT=$((ERROR_COUNT + 1))
      record_result "$pid" "$pname" "$retailer_display" "$url" "" "error" "" "" "extraction failed"
      sleep "$NAV_DELAY"
      continue
    fi

    # Check for in-page bot detection
    scrape_error=""
    scrape_error=$(echo "$scrape_result" | jq -r '.error // empty' 2>/dev/null) || true
    if [ -n "$scrape_error" ]; then
      log "  [$retailer_display] BLOCKED: $scrape_error"
      ERROR_COUNT=$((ERROR_COUNT + 1))
      BLOCKED_RETAILERS_MAP[$retailer_display]=1
      record_result "$pid" "$pname" "$retailer_display" "$url" "" "blocked" "" "" "$scrape_error"
      sleep "$NAV_DELAY"
      continue
    fi

    # Extract fields
    price=""
    availability="unknown"
    vtitle=""
    vcontext=""
    price=$(echo "$scrape_result" | jq -r '.price // empty' 2>/dev/null) || true
    availability=$(echo "$scrape_result" | jq -r '.availability // "unknown"' 2>/dev/null) || true
    vtitle=$(echo "$scrape_result" | jq -r '.validation_title // ""' 2>/dev/null) || true
    vcontext=$(echo "$scrape_result" | jq -r '.validation_context // ""' 2>/dev/null) || true

    # Validate Best Buy didn't redirect to a different product
    if [ "$rid" = "bestbuy" ] && [ -n "$vcontext" ] && [ -n "$price" ] && [ "$price" != "null" ]; then
      if ! validate_bestbuy_product "$pname" "$vcontext"; then
        log "  [$retailer_display] MISMATCH: page shows wrong product (bot redirect)"
        log "    Expected: $pname"
        log "    Got: ${vcontext:0:60}"
        ERROR_COUNT=$((ERROR_COUNT + 1))
        record_result "$pid" "$pname" "$retailer_display" "$url" "" "mismatch" "$vtitle" "$vcontext" "product mismatch - bot redirect to: ${vcontext:0:80}"
        sleep "$NAV_DELAY"
        continue
      fi
    fi

    if [ -n "$price" ] && [ "$price" != "null" ]; then
      log "  [$retailer_display] Price: $price | Status: $availability"
      SCRAPE_COUNT=$((SCRAPE_COUNT + 1))
      record_result "$pid" "$pname" "$retailer_display" "$url" "$price" "$availability" "$vtitle" "$vcontext"
    else
      log "  [$retailer_display] Price not found (dynamic loading or different page structure)"
      ERROR_COUNT=$((ERROR_COUNT + 1))
      record_result "$pid" "$pname" "$retailer_display" "$url" "" "$availability" "$vtitle" "$vcontext" "price not found on page"
    fi

    sleep "$NAV_DELAY"
  done
done

# Save raw results
log ""
log "=== Saving raw price data ==="
echo "$RESULTS" | jq '.' > "$JSON_OUT"
total_entries=$(echo "$RESULTS" | jq 'length')
log "Saved $JSON_OUT ($total_entries entries)"

###############################################################################
# Phase 3: Generate Comparison Report
###############################################################################
log ""
log "=== PHASE 3: Price Comparison Report ==="

echo ""
echo "=============================================================================="
echo "  COMPETITOR PRICE MONITORING REPORT"
echo "  Generated: $(date '+%Y-%m-%d %H:%M:%S')"
echo "=============================================================================="
echo ""
printf "%-35s | %-12s | %-12s | %-12s | %-10s\n" "Product" "Amazon" "Walmart" "Best Buy" "Best Deal"
echo "------------------------------------+--------------+--------------+--------------+-----------"

COMPARISON_DATA="[]"

for pid in "${PRODUCT_IDS[@]}"; do
  pname="${PRODUCT_NAMES[$pid]}"

  amazon_raw=$(echo "$RESULTS" | jq -r --arg pid "$pid" '[.[] | select(.product_id == $pid and .retailer == "Amazon")][0].price // "N/A"')
  walmart_raw=$(echo "$RESULTS" | jq -r --arg pid "$pid" '[.[] | select(.product_id == $pid and .retailer == "Walmart")][0].price // "N/A"')
  bestbuy_raw=$(echo "$RESULTS" | jq -r --arg pid "$pid" '[.[] | select(.product_id == $pid and .retailer == "Best Buy")][0].price // "N/A"')

  [ "$amazon_raw" = "null" ] && amazon_raw="N/A"
  [ "$walmart_raw" = "null" ] && walmart_raw="N/A"
  [ "$bestbuy_raw" = "null" ] && bestbuy_raw="N/A"

  amazon_num=$(parse_price "$amazon_raw" 2>/dev/null || echo "")
  walmart_num=$(parse_price "$walmart_raw" 2>/dev/null || echo "")
  bestbuy_num=$(parse_price "$bestbuy_raw" 2>/dev/null || echo "")

  best_deal="N/A"
  best_price=""
  for rname_num in "Amazon:$amazon_num" "Walmart:$walmart_num" "Best Buy:$bestbuy_num"; do
    rname="${rname_num%%:*}"
    rnum="${rname_num#*:}"
    if [ -n "$rnum" ]; then
      if [ -z "$best_price" ] || (( $(echo "$rnum < $best_price" | bc -l 2>/dev/null || echo 0) )); then
        best_price="$rnum"
        best_deal="$rname"
      fi
    fi
  done

  amazon_disp="$amazon_raw"
  walmart_disp="$walmart_raw"
  bestbuy_disp="$bestbuy_raw"

  walmart_err=$(echo "$RESULTS" | jq -r --arg pid "$pid" '[.[] | select(.product_id == $pid and .retailer == "Walmart")][0].error // ""')
  bestbuy_err=$(echo "$RESULTS" | jq -r --arg pid "$pid" '[.[] | select(.product_id == $pid and .retailer == "Best Buy")][0].error // ""')
  [ -n "$walmart_err" ] && [ "$walmart_disp" = "N/A" ] && walmart_disp="BLOCKED"
  [ -n "$bestbuy_err" ] && [ "$bestbuy_disp" = "N/A" ] && bestbuy_disp="BLOCKED"

  printf "%-35s | %-12s | %-12s | %-12s | %-10s\n" "${pname:0:35}" "$amazon_disp" "$walmart_disp" "$bestbuy_disp" "$best_deal"

  comp_entry=$(jq -n \
    --arg pid "$pid" \
    --arg pname "$pname" \
    --arg araw "$amazon_disp" \
    --arg anum "${amazon_num:-}" \
    --arg wraw "$walmart_disp" \
    --arg wnum "${walmart_num:-}" \
    --arg braw "$bestbuy_disp" \
    --arg bnum "${bestbuy_num:-}" \
    --arg best "$best_deal" \
    --arg bestp "${best_price:-}" \
    '{
      product_id: $pid,
      product_name: $pname,
      prices: {
        amazon: { display: $araw, numeric: (if $anum != "" then ($anum | tonumber) else null end) },
        walmart: { display: $wraw, numeric: (if $wnum != "" then ($wnum | tonumber) else null end) },
        bestbuy: { display: $braw, numeric: (if $bnum != "" then ($bnum | tonumber) else null end) }
      },
      best_deal: { retailer: $best, price: (if $bestp != "" then ($bestp | tonumber) else null end) }
    }')
  COMPARISON_DATA=$(echo "$COMPARISON_DATA" | jq --argjson e "$comp_entry" '. + [$e]')
done

echo "=============================================================================="
echo ""

# Price analysis
echo "--- Price Analysis ---"
echo ""
echo "$COMPARISON_DATA" | jq -r '
  .[] |
  .product_name as $name |
  [
    (if .prices.amazon.numeric then {r: "Amazon", p: .prices.amazon.numeric} else empty end),
    (if .prices.walmart.numeric then {r: "Walmart", p: .prices.walmart.numeric} else empty end),
    (if .prices.bestbuy.numeric then {r: "Best Buy", p: .prices.bestbuy.numeric} else empty end)
  ] |
  if length > 1 then
    (sort_by(.p) | .[0]) as $low |
    (sort_by(-.p) | .[0]) as $high |
    "  \($name):\n    Lowest:  \($low.r) at $\($low.p)\n    Highest: \($high.r) at $\($high.p)\n    Spread:  $\(($high.p - $low.p) * 100 | round / 100) (\((($high.p - $low.p) / $low.p * 100) * 10 | round / 10)%)\n"
  elif length == 1 then
    "  \($name): \(.[0].r) at $\(.[0].p) (single source)\n"
  else
    "  \($name): No prices available\n"
  end
'

# Summary
echo ""
echo "--- Summary ---"
echo "  Products tracked:    ${#PRODUCT_IDS[@]}"
echo "  Prices captured:     $SCRAPE_COUNT"
echo "  Errors/blocked:      $ERROR_COUNT"
blocked_list=""
for r in "${!BLOCKED_RETAILERS_MAP[@]}"; do
  blocked_list="$blocked_list $r"
done
if [ -n "$blocked_list" ]; then
  echo "  Blocked retailers:  $blocked_list"
  echo ""
  echo "  Note: Blocked retailers use bot detection (CAPTCHAs, HTTP/2 protocol"
  echo "  errors, or JavaScript challenges). In production, these can be handled"
  echo "  with rotating proxies, browser fingerprint rotation, or API-based"
  echo "  price feeds instead of scraping."
fi
echo ""

# Validation evidence
echo "--- Validation Evidence ---"
echo ""
echo "$RESULTS" | jq -r '
  .[] |
  select(.price != null) |
  "  [\(.retailer)] \(.product_name)\n    Price: \(.price)\n    Page title: \(.validation_title[0:80])\n    Context: \(.validation_context[0:80])\n"
'

###############################################################################
# Phase 4: Simulate Daily Monitoring (History + Alerts)
###############################################################################
log ""
log "=== PHASE 4: Daily Monitoring Simulation ==="

TODAY=$(date +%Y-%m-%d)

if [ -f "$HISTORY_OUT" ]; then
  HISTORY=$(cat "$HISTORY_OUT")
else
  HISTORY='{"entries": [], "alerts": []}'
fi

SNAPSHOT=$(jq -n \
  --arg date "$TODAY" \
  --arg ts "$(date -Iseconds)" \
  --argjson prices "$COMPARISON_DATA" \
  --arg scrape_count "$SCRAPE_COUNT" \
  --arg error_count "$ERROR_COUNT" \
  '{
    date: $date,
    scraped_at: $ts,
    products_tracked: ($prices | length),
    prices_captured: ($scrape_count | tonumber),
    errors: ($error_count | tonumber),
    prices: $prices
  }')

HISTORY=$(echo "$HISTORY" | jq --argjson snap "$SNAPSHOT" '.entries += [$snap]')

echo ""
echo "--- Simulated Price Change Alerts ---"
echo ""

ALERTS='[]'

generate_alert() {
  local product="$1" retailer="$2" was="$3" now="$4"
  local diff abs_diff pct direction

  diff=$(echo "scale=2; $was - $now" | bc -l 2>/dev/null || echo "0")
  abs_diff="$diff"
  if echo "$diff" | grep -q '^-'; then
    direction="INCREASED"
    abs_diff=$(echo "$diff" | sed 's/^-//')
  else
    direction="DROPPED"
  fi
  pct=$(printf "%.1f" "$(echo "scale=4; ($abs_diff / $was) * 100" | bc -l 2>/dev/null || echo 0)" 2>/dev/null || echo "0.0")

  printf "  %s: %s %s \$%s at %s (was \$%s, now \$%s) [%s%%]\n" \
    "$([ "$direction" = "DROPPED" ] && echo 'PRICE DROP' || echo 'PRICE RISE')" \
    "$product" "$direction" "$abs_diff" "$retailer" "$was" "$now" "$pct"

  local entry
  entry=$(jq -n \
    --arg product "$product" \
    --arg retailer "$retailer" \
    --arg was "$was" \
    --arg now "$now" \
    --arg diff "$abs_diff" \
    --arg pct "$pct" \
    --arg direction "$direction" \
    --arg date "$TODAY" \
    '{
      date: $date,
      product: $product,
      retailer: $retailer,
      previous_price: ($was | tonumber),
      current_price: ($now | tonumber),
      difference: ($diff | tonumber),
      percent_change: ($pct | tonumber),
      direction: $direction
    }')
  ALERTS=$(echo "$ALERTS" | jq --argjson a "$entry" '. + [$a]')
}

# Generate mock alerts based on actual scraped prices
amazon_airpods=$(echo "$COMPARISON_DATA" | jq -r '.[0].prices.amazon.numeric // empty')
amazon_sony=$(echo "$COMPARISON_DATA" | jq -r '.[1].prices.amazon.numeric // empty')
amazon_jbl=$(echo "$COMPARISON_DATA" | jq -r '.[2].prices.amazon.numeric // empty')

if [ -n "$amazon_airpods" ]; then
  mock_yesterday=$(echo "$amazon_airpods + 15" | bc -l)
  generate_alert "AirPods Pro 2" "Amazon" "$mock_yesterday" "$amazon_airpods"
fi
if [ -n "$amazon_sony" ]; then
  mock_yesterday=$(echo "$amazon_sony - 22" | bc -l)
  generate_alert "Sony WH-1000XM5" "Amazon" "$mock_yesterday" "$amazon_sony"
fi
if [ -n "$amazon_jbl" ]; then
  mock_yesterday=$(echo "$amazon_jbl + 10.04" | bc -l)
  generate_alert "JBL Flip 6" "Amazon" "$mock_yesterday" "$amazon_jbl"
fi

echo ""
echo "  (Alerts above are simulated using today's actual prices +/- mock changes."
echo "   In production, alerts fire when a price changes between daily scrapes.)"

HISTORY=$(echo "$HISTORY" | jq --argjson alerts "$ALERTS" '.alerts += $alerts')
echo "$HISTORY" | jq '.' > "$HISTORY_OUT"

log ""
log "History saved to $HISTORY_OUT"
log ""
echo "=============================================================================="
echo "  Price monitoring complete."
echo ""
echo "  Output files:"
echo "    Raw data:   $JSON_OUT"
echo "    History:    $HISTORY_OUT"
echo ""
echo "  Results: $SCRAPE_COUNT prices captured across ${#PRODUCT_IDS[@]} products"
echo "=============================================================================="
