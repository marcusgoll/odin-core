# Price Monitor Template

Scrapes product prices across multiple retailers (Amazon, Walmart, Best Buy), generates comparison reports, and simulates daily monitoring with price change alerts.

## What It Does

1. **Product Configuration** -- Defines products and their URLs across 3 retailers
2. **Price Scraping** -- Visits each product page and extracts current price, availability, and validation data
3. **Bot Detection Handling** -- Detects and gracefully handles CAPTCHA pages, access blocks, and product redirects
4. **Comparison Report** -- Generates a formatted price comparison table with best-deal analysis
5. **History & Alerts** -- Maintains price history and simulates daily change alerts

## Prerequisites

- Odin Huginn browser server running (`odin agent start`)
- `jq` and `bc` installed
- `curl` installed

## Usage

```bash
# Run with built-in demo products (5 consumer electronics)
odin run price-monitor

# Run with custom product config
odin run price-monitor --config my-products.json
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ODIN_BROWSER_URL` | `http://127.0.0.1:9227` | Huginn browser server URL |
| `ODIN_BROWSER_TOKEN` | (none) | Optional auth token for browser server |
| `ODIN_OUTPUT_DIR` | `./output` | Directory for output files |

## Output Files

- `prices.json` -- Raw scrape results with price, availability, and validation data
- `price-history.json` -- Historical price snapshots and simulated alerts

## Demo Products

The built-in configuration tracks 5 products across Amazon, Walmart, and Best Buy:
- Apple AirPods Pro 2 (USB-C)
- Sony WH-1000XM5 Headphones
- JBL Flip 6 Bluetooth Speaker
- Apple Watch SE (2nd Gen, 40mm)
- Samsung Galaxy Buds FE

## Notes

Some retailers (Walmart, Best Buy) use aggressive bot detection. The script handles blocked requests gracefully and reports them. In production, consider rotating proxies or API-based price feeds.
