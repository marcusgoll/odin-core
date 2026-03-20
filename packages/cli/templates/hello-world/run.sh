#!/usr/bin/env bash
set -euo pipefail
NAME="${1:-World}"
echo "Hello, $NAME! This is Odin." | tee output/greeting.txt
echo "Template completed successfully."
