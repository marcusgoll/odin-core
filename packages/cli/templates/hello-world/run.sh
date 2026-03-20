#!/usr/bin/env bash
set -euo pipefail

OUT_DIR="${ODIN_OUTPUT_DIR:-./output}"
mkdir -p "$OUT_DIR"

NAME="${1:-World}"
echo "Hello, $NAME! This is Odin." | tee "$OUT_DIR/greeting.txt"
echo "Template completed successfully."
