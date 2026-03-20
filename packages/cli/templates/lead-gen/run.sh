#!/usr/bin/env bash
# lead-gen/run.sh — Lead Generation Template
# Scrapes Clutch.co digital marketing agencies, enriches with profile and
# website data, outputs JSON + CSV report.
#
# Environment:
#   ODIN_BROWSER_URL   — Browser server URL (default: http://127.0.0.1:9227)
#   ODIN_BROWSER_TOKEN — Optional auth token for browser server
#   ODIN_OUTPUT_DIR    — Output directory (default: ./output)
#
# Usage:
#   bash run.sh [--target "description"] [--profile-limit N] [--website-limit N]

set -euo pipefail

###############################################################################
# Usage
###############################################################################
usage() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Scrape business directories and enrich leads with website data.

Options:
  --target TEXT        Description of leads to find (default: "digital marketing agencies in Texas")
  --profile-limit N    Max profile pages to enrich (default: 10)
  --website-limit N    Max company websites to visit (default: 5)
  -h, --help           Show this help message

Environment Variables:
  ODIN_BROWSER_URL     Browser server URL (default: http://127.0.0.1:9227)
  ODIN_BROWSER_TOKEN   Optional auth token for browser server
  ODIN_OUTPUT_DIR      Output directory (default: ./output)

Output:
  leads.json           Full structured data for all scraped leads
  leads.csv            CSV export with key fields
EOF
  exit 0
}

###############################################################################
# Parse Arguments
###############################################################################
TARGET="digital marketing agencies in Texas"
PROFILE_LIMIT=10
WEBSITE_LIMIT=5

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target)      TARGET="$2"; shift 2 ;;
    --profile-limit) PROFILE_LIMIT="$2"; shift 2 ;;
    --website-limit) WEBSITE_LIMIT="$2"; shift 2 ;;
    -h|--help)     usage ;;
    *)             echo "Unknown option: $1" >&2; usage ;;
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
JSON_OUT="$OUT_DIR/leads.json"
CSV_OUT="$OUT_DIR/leads.csv"
NAV_DELAY=3
TMP_JS="/tmp/leadgen-eval-$$.js"

mkdir -p "$OUT_DIR"

trap 'rm -f "$TMP_JS"' EXIT

###############################################################################
# Helpers
###############################################################################
log() { echo "[$(date '+%H:%M:%S')] $*" >&2; }

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

# Evaluate JS in the browser. Writes JS to temp file to avoid shell escaping issues.
# Usage: browser_eval_file <<'JSEOF'
#   ...javascript...
# JSEOF
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

###############################################################################
# Phase 1: Scrape Directory
###############################################################################
phase1_scrape_directory() {
  log "=== PHASE 1: Scraping Clutch.co directory ==="

  local target_url="https://clutch.co/agencies/digital-marketing/texas"
  log "Navigating to $target_url"
  browser_navigate "$target_url"
  sleep "$NAV_DELAY"

  log "Extracting agency cards..."
  local raw
  raw=$(browser_eval_file <<'JSEOF'
JSON.stringify((() => {
  const results = [];
  const listItems = document.querySelectorAll("li");
  for (const li of listItems) {
    const h3 = li.querySelector("h3");
    if (!h3) continue;
    const link = h3.querySelector("a");
    if (!link) continue;
    const href = link.href;
    if (!href.includes("clutch.co/profile/")) continue;
    if (href.includes("r.clutch.co") || href.includes("redirect")) continue;

    const name = h3.textContent.trim();
    const profileUrl = href;
    const container = li.querySelector(":scope > div") || li;

    // Rating from span.sg-rating__number
    let rating = "";
    const ratingEl = container.querySelector(".sg-rating__number");
    if (ratingEl) rating = ratingEl.textContent.trim();

    // Location: look for "City, TX" pattern in small divs
    let loc = "";
    container.querySelectorAll("div").forEach(function(d) {
      const t = d.textContent.trim();
      if (/^[A-Z][a-z]+(\s[A-Z][a-z]+)?,\s*TX$/.test(t)) loc = t;
    });

    // Employees: "N - N" or "N+" pattern
    let employees = "";
    container.querySelectorAll("div").forEach(function(d) {
      const t = d.textContent.trim();
      if (/^\d[\d,]* - [\d,]+$/.test(t) || /^\d[\d,]+\+$/.test(t)) employees = t;
    });

    // Services: "NN% ServiceName" divs
    const services = [];
    container.querySelectorAll("div").forEach(function(d) {
      const t = d.textContent.trim();
      if (/^\d+%\s+[A-Z]/.test(t)) {
        const svc = t.replace(/^\d+%\s+/, "").split("\n")[0].trim();
        if (svc.length < 50 && services.indexOf(svc) === -1) services.push(svc);
      }
    });

    // Reviews count
    let reviews = "";
    container.querySelectorAll("a").forEach(function(a) {
      const t = a.textContent.trim();
      const m = t.match(/^(\d+)\s+reviews?$/);
      if (m) reviews = m[1];
    });

    results.push({
      name: name,
      profileUrl: profileUrl,
      rating: rating,
      location: loc || "Texas",
      employees: employees,
      reviews: reviews,
      services: services.slice(0, 5).join("; "),
      website: "",
      minProject: "",
      contactEmail: "",
      description: "",
      founder: ""
    });
  }
  return results;
})())
JSEOF
  )

  if [ "$raw" = "null" ] || [ -z "$raw" ]; then
    log "ERROR: Failed to extract directory data"
    echo "[]"
    return 1
  fi

  local count
  count=$(echo "$raw" | jq 'length')
  log "Extracted $count organic agency listings"

  # Deduplicate by name, preserving original page order
  local deduped
  deduped=$(echo "$raw" | jq 'reduce .[] as $item ([]; if [.[] | .name] | index($item.name) then . else . + [$item] end)')
  local dedup_count
  dedup_count=$(echo "$deduped" | jq 'length')
  log "After dedup: $dedup_count unique agencies"

  echo "$deduped"
}

###############################################################################
# Phase 2: Enrich from Clutch Profile Pages
###############################################################################
phase2_enrich_profiles() {
  local leads_json="$1"
  log ""
  log "=== PHASE 2: Enriching from Clutch profile pages (first $PROFILE_LIMIT) ==="

  local count
  count=$(echo "$leads_json" | jq 'length')
  local limit=$((count < PROFILE_LIMIT ? count : PROFILE_LIMIT))

  for i in $(seq 0 $((limit - 1))); do
    local name profileUrl
    name=$(echo "$leads_json" | jq -r ".[$i].name")
    profileUrl=$(echo "$leads_json" | jq -r ".[$i].profileUrl")

    log "  [$((i+1))/$limit] Enriching profile: $name"
    log "    URL: $profileUrl"

    browser_navigate "$profileUrl"
    sleep "$NAV_DELAY"

    local profile_data
    profile_data=$(browser_eval_file <<'JSEOF'
JSON.stringify((() => {
  const result = { website: "", minProject: "", employees: "", services: "" };

  // Website: look for external links that aren't clutch.co redirects
  document.querySelectorAll("a").forEach(function(a) {
    const href = a.href || "";
    const text = a.textContent.trim().toLowerCase();
    if (!result.website && href.startsWith("http")
        && !href.includes("clutch.co") && !href.includes("r.clutch.co")
        && !href.includes("facebook.com") && !href.includes("twitter.com")
        && !href.includes("linkedin.com") && !href.includes("instagram.com")
        && !href.includes("youtube.com") && !href.includes("g.clutch.co")
        && (text === "visit website" || text.includes("website"))) {
      result.website = href.split("?")[0];
    }
  });

  // Second try: extract URL from clutch redirect links
  if (!result.website) {
    document.querySelectorAll("a").forEach(function(a) {
      const href = a.href || "";
      if (href.includes("r.clutch.co/redirect") || href.includes("clutch.co/redirect")) {
        const uMatch = href.match(/[&?]u=([^&]+)/);
        if (uMatch) {
          try {
            let decoded = decodeURIComponent(uMatch[1]);
            decoded = decoded.split("?")[0];
            if (!decoded.includes("clutch.co") && !decoded.includes("ppc.clutch.co")
                && decoded.startsWith("http") && !result.website) {
              result.website = decoded;
            }
          } catch(e) {}
        }
      }
    });
  }

  // Summary data: min project size, employees
  const allText = document.body.innerText || "";
  const minMatch = allText.match(/Min\.\s*project\s*size[:\s]*\$?([\d,]+\+?)/i);
  if (minMatch) result.minProject = "$" + minMatch[1];

  const empMatch = allText.match(/Employees[:\s]*([\d,]+ - [\d,]+|[\d,]+\+?)/i);
  if (empMatch) result.employees = empMatch[1];

  // Services
  const navJunk = ["leave a review", "for providers", "post a project", "sign in", "join",
                   "select a service", "view profile", "visit website", "get matched"];
  const svcs = [];
  document.querySelectorAll("a, span, li").forEach(function(el) {
    const t = el.textContent.trim();
    const href = (el.href || "").toLowerCase();
    if (t.length > 3 && t.length < 50
        && !navJunk.some(function(j) { return t.toLowerCase().includes(j); })
        && (href.includes("/service") || href.includes("/focus")
            || (el.closest && el.closest("[class*=service], [class*=focus], [data-content*=service]")))
        && svcs.indexOf(t) === -1) {
      svcs.push(t);
    }
  });
  if (svcs.length > 0) result.services = svcs.slice(0, 5).join("; ");

  return result;
})())
JSEOF
    )

    if [ "$profile_data" != "null" ] && [ -n "$profile_data" ]; then
      local website minProject employees services
      website=$(echo "$profile_data" | jq -r '.website // ""')
      minProject=$(echo "$profile_data" | jq -r '.minProject // ""')
      employees=$(echo "$profile_data" | jq -r '.employees // ""')
      services=$(echo "$profile_data" | jq -r '.services // ""')

      if [ -n "$website" ] && [ "$website" != "" ]; then
        leads_json=$(echo "$leads_json" | jq --arg idx "$i" --arg val "$website" \
          '.[$idx | tonumber].website = $val')
        log "    Found website: $website"
      fi
      if [ -n "$minProject" ] && [ "$minProject" != "" ]; then
        leads_json=$(echo "$leads_json" | jq --arg idx "$i" --arg val "$minProject" \
          '.[$idx | tonumber].minProject = $val')
        log "    Min project: $minProject"
      fi
      if [ -n "$employees" ] && [ "$employees" != "" ]; then
        leads_json=$(echo "$leads_json" | jq --arg idx "$i" --arg val "$employees" \
          '.[$idx | tonumber].employees = $val')
      fi
      local existing_services
      existing_services=$(echo "$leads_json" | jq -r ".[$i].services // \"\"")
      if [ -n "$services" ] && [ "$services" != "" ] && [ -z "$existing_services" ]; then
        leads_json=$(echo "$leads_json" | jq --arg idx "$i" --arg val "$services" \
          '.[$idx | tonumber].services = $val')
      fi
    else
      log "    WARNING: Could not extract profile data"
    fi
  done

  echo "$leads_json"
}

###############################################################################
# Phase 3: Enrich from Company Websites
###############################################################################
phase3_enrich_websites() {
  local leads_json="$1"
  log ""
  log "=== PHASE 3: Enriching from company websites (first $WEBSITE_LIMIT with URLs) ==="

  local enriched=0
  local count
  count=$(echo "$leads_json" | jq 'length')

  for i in $(seq 0 $((count - 1))); do
    [ "$enriched" -ge "$WEBSITE_LIMIT" ] && break

    local website name
    website=$(echo "$leads_json" | jq -r ".[$i].website // \"\"")
    name=$(echo "$leads_json" | jq -r ".[$i].name")

    [ -z "$website" ] && continue
    [ "$website" = "" ] && continue

    enriched=$((enriched + 1))
    log "  [$enriched/$WEBSITE_LIMIT] Visiting website for: $name"
    log "    URL: $website"

    browser_navigate "$website"
    sleep "$NAV_DELAY"

    # Extract homepage info
    local homepage_data
    homepage_data=$(browser_eval_file <<'JSEOF'
JSON.stringify((() => {
  const result = { description: "", contactEmail: "", aboutUrl: "" };

  // Meta description
  const metaDesc = document.querySelector('meta[name="description"]');
  if (metaDesc && metaDesc.content) result.description = metaDesc.content.trim().substring(0, 300);

  if (!result.description) {
    const ogDesc = document.querySelector('meta[property="og:description"]');
    if (ogDesc && ogDesc.content) result.description = ogDesc.content.trim().substring(0, 300);
  }

  if (!result.description) {
    const paragraphs = document.querySelectorAll("p");
    for (const p of paragraphs) {
      const t = p.textContent.trim();
      if (t.length > 50 && t.length < 500) {
        result.description = t.substring(0, 300);
        break;
      }
    }
  }

  // Contact email from mailto: links
  document.querySelectorAll('a[href^="mailto:"]').forEach(function(a) {
    if (!result.contactEmail) {
      const email = a.href.replace("mailto:", "").split("?")[0].trim();
      if (email.includes("@")) result.contactEmail = email;
    }
  });

  if (!result.contactEmail) {
    const bodyText = document.body.innerText || "";
    const emailMatch = bodyText.match(/[\w.+-]+@[\w-]+\.[\w.]+/);
    if (emailMatch) result.contactEmail = emailMatch[0];
  }

  // About/Team page link
  document.querySelectorAll("a").forEach(function(a) {
    const text = a.textContent.trim().toLowerCase();
    const href = a.href || "";
    if ((text === "about" || text === "about us" || text === "our team"
         || text === "team" || text === "leadership" || text === "who we are")
        && href.startsWith("http") && !result.aboutUrl) {
      result.aboutUrl = href;
    }
  });

  return result;
})())
JSEOF
    )

    if [ "$homepage_data" != "null" ] && [ -n "$homepage_data" ]; then
      local description contactEmail aboutUrl
      description=$(echo "$homepage_data" | jq -r '.description // ""')
      contactEmail=$(echo "$homepage_data" | jq -r '.contactEmail // ""')
      aboutUrl=$(echo "$homepage_data" | jq -r '.aboutUrl // ""')

      if [ -n "$description" ] && [ "$description" != "" ]; then
        leads_json=$(echo "$leads_json" | jq --arg idx "$i" --arg val "$description" \
          '.[$idx | tonumber].description = $val')
        log "    Found description (${#description} chars)"
      fi
      if [ -n "$contactEmail" ] && [ "$contactEmail" != "" ]; then
        leads_json=$(echo "$leads_json" | jq --arg idx "$i" --arg val "$contactEmail" \
          '.[$idx | tonumber].contactEmail = $val')
        log "    Found email: $contactEmail"
      fi

      # Visit about/team page for founder info
      if [ -n "$aboutUrl" ] && [ "$aboutUrl" != "" ]; then
        log "    Visiting about/team page: $aboutUrl"
        browser_navigate "$aboutUrl"
        sleep "$NAV_DELAY"

        local about_data
        about_data=$(browser_eval_file <<'JSEOF'
JSON.stringify((() => {
  const result = { founder: "" };

  function norm(s) { return (s || "").replace(/[\r\n\t]+/g, " ").replace(/\s+/g, " ").trim(); }

  function validName(n) {
    n = norm(n);
    if (!n) return "";
    if (!/^[A-Z][a-z]+(\s[A-Z][a-z]+){1,3}$/.test(n)) return "";
    const reject = ["Our Team","About Us","Contact Us","Learn More","Read More","Get Started","View All","See More","Privacy Policy","Terms Service"];
    if (reject.some(function(r) { return r.toLowerCase() === n.toLowerCase(); })) return "";
    return n;
  }

  const roleWords = /\b(?:Founder|Co-Founder|CEO|Chief Executive Officer|Owner|President|Managing Director|Principal)\b/i;

  // Strategy 1: JSON-LD structured data
  const jsonLd = document.querySelectorAll('script[type="application/ld+json"]');
  for (const s of jsonLd) {
    try {
      const data = JSON.parse(s.textContent);
      const items = Array.isArray(data) ? data : [data];
      for (const item of items) {
        if (item.founder) {
          const fn = typeof item.founder === "string" ? item.founder : (item.founder.name || "");
          const v = validName(fn);
          if (v) { result.founder = v; return result; }
        }
        if (item.author) {
          const an = typeof item.author === "string" ? item.author : (item.author.name || "");
          const v = validName(an);
          if (v) { result.founder = v; return result; }
        }
      }
    } catch(e) {}
  }

  // Strategy 2: Schema.org itemprop markup
  const founderEls = document.querySelectorAll('[itemprop="founder"] [itemprop="name"], [itemprop="founder"][itemprop="name"]');
  for (const el of founderEls) {
    const v = validName(norm(el.textContent));
    if (v) { result.founder = v; return result; }
  }

  // Strategy 3: Structured team/about sections
  const teamEls = document.querySelectorAll('[class*="team"], [class*="founder"], [class*="leadership"], [class*="about"], [id*="team"], [class*="Team"], [class*="Founder"], [class*="Leadership"], [id*="Team"]');
  for (const el of teamEls) {
    const text = norm(el.innerText);
    const m1 = text.match(/(?:Founder|CEO|Co-Founder|Owner|President|Managing Director|Principal)\s*[:\-\u2013\u2014]?\s*([A-Z][a-z]+(?:\s[A-Z][a-z]+){1,3})/);
    if (m1) { const v = validName(m1[1]); if (v) { result.founder = v; return result; } }
    const m2 = text.match(/([A-Z][a-z]+(?:\s[A-Z][a-z]+){1,3})\s*[,\-\u2013\u2014]\s*(?:Founder|CEO|Co-Founder|Owner|President|Managing Director|Principal)/);
    if (m2) { const v = validName(m2[1]); if (v) { result.founder = v; return result; } }
  }

  // Strategy 4: Heading + sibling/parent role association
  document.querySelectorAll("h2, h3, h4, h5, strong, b").forEach(function(el) {
    if (result.founder) return;
    const name = norm(el.textContent);
    const v = validName(name);
    if (!v) return;
    const parent = el.parentElement;
    if (!parent) return;
    const parentText = norm(parent.innerText);
    if (roleWords.test(parentText)) {
      if (parentText.length < 500) {
        result.founder = v;
      }
    }
  });
  if (result.founder) return result;

  // Strategy 5: Meta author tag
  const authorMeta = document.querySelector('meta[name="author"]');
  if (authorMeta && authorMeta.content) {
    const v = validName(norm(authorMeta.content));
    if (v && v.split(" ").length <= 4) { result.founder = v; return result; }
  }

  return result;
})())
JSEOF
        )

        if [ "$about_data" != "null" ] && [ -n "$about_data" ]; then
          local founder
          founder=$(echo "$about_data" | jq -r '.founder // ""')
          if [ -n "$founder" ] && [ "$founder" != "" ]; then
            leads_json=$(echo "$leads_json" | jq --arg idx "$i" --arg val "$founder" \
              '.[$idx | tonumber].founder = $val')
            log "    Found founder/CEO: $founder"
          else
            log "    No founder/CEO found on about page"
          fi
        fi
      fi
    else
      log "    WARNING: Could not load website"
    fi
  done

  echo "$leads_json"
}

###############################################################################
# Phase 4: Output Report
###############################################################################
phase4_output_report() {
  local leads_json="$1"
  log ""
  log "=== PHASE 4: Generating reports ==="

  # Save full JSON
  echo "$leads_json" | jq '.' > "$JSON_OUT"
  log "Saved JSON: $JSON_OUT"

  # Build CSV
  {
    echo "Name,Location,Rating,Website,Employees,Contact Email,Services,Founder/CEO"
    echo "$leads_json" | jq -r '.[] | [
      .name,
      .location,
      .rating,
      .website,
      .employees,
      .contactEmail,
      (.services | gsub(";"; " |")),
      .founder
    ] | @csv'
  } > "$CSV_OUT"
  log "Saved CSV: $CSV_OUT"

  # Print summary
  local total with_website with_desc with_email with_founder
  total=$(echo "$leads_json" | jq 'length')
  with_website=$(echo "$leads_json" | jq '[.[] | select(.website != "")] | length')
  with_desc=$(echo "$leads_json" | jq '[.[] | select(.description != "")] | length')
  with_email=$(echo "$leads_json" | jq '[.[] | select(.contactEmail != "")] | length')
  with_founder=$(echo "$leads_json" | jq '[.[] | select(.founder != "")] | length')

  echo ""
  echo "=============================================="
  echo "  LEAD GENERATION — RESULTS SUMMARY"
  echo "=============================================="
  echo "  Total leads scraped:      $total"
  echo "  With website URL:         $with_website"
  echo "  With description:         $with_desc"
  echo "  With contact email:       $with_email"
  echo "  With founder/CEO:         $with_founder"
  echo "=============================================="
  echo ""
  echo "  Top 10 leads:"
  echo "  -----------------------------------------------"
  echo "$leads_json" | jq -r '.[0:10] | .[] |
    "  \(.name) | \(.location) | Rating: \(.rating // "N/A") | \(.website // "no website")"'
  echo ""
  echo "  Files:"
  echo "    JSON: $JSON_OUT"
  echo "    CSV:  $CSV_OUT"
  echo "=============================================="
}

###############################################################################
# Main
###############################################################################
main() {
  log "Starting Lead Generation"
  log "Target: $TARGET"
  log ""

  # Phase 1
  local leads
  leads=$(phase1_scrape_directory)
  local lead_count
  lead_count=$(echo "$leads" | jq 'length' 2>/dev/null || echo "0")
  if [ "$lead_count" -eq 0 ]; then
    log "FATAL: No leads scraped. Exiting."
    exit 1
  fi

  # Phase 2
  leads=$(phase2_enrich_profiles "$leads")

  # Phase 3
  leads=$(phase3_enrich_websites "$leads")

  # Phase 4
  phase4_output_report "$leads"

  log ""
  log "Lead Generation complete."
}

main "$@"
