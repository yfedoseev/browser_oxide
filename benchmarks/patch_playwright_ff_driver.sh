#!/usr/bin/env bash
# Fix a playwright-firefox DRIVER bug that crashes the Node driver process
# ("Connection closed while reading from the driver") whenever a page fires an
# uncaught JS error with NO source location (common on bing/yahoo/microsoft/
# cnn/aws/etc. — ad/tracker scripts, cross-origin errors).
#
# Root cause (playwright coreBundle.js, FFBrowserContext page-error path):
#   url:    pageError.location.url           // throws if location is undefined
#   line:   pageError.location.lineNumber
#   column: pageError.location.columnNumber
# then the protocol validator requires location.url to be a *string*, so even a
# null-check that yields `undefined` fails ("expected string, got undefined").
#
# Fix: default to ('' , 0, 0) when location is missing. This is what makes
# camoufox survive heavy sites (and sustained loops). Idempotent; backs up once.
#
# Usage: patch_playwright_ff_driver.sh <venv1> [venv2 ...]
set -uo pipefail
for venv in "$@"; do
  CB="$venv/lib/python3.14/site-packages/playwright/driver/package/lib/coreBundle.js"
  [ -f "$CB" ] || { echo "skip (no coreBundle): $venv"; continue; }
  cp -n "$CB" "$CB.orig"
  # restore from backup first so the patch is deterministic/idempotent
  cp -f "$CB.orig" "$CB"
  sed -i \
    -e 's/pageError\.location\.url/(pageError.location?.url ?? "")/g' \
    -e 's/pageError\.location\.lineNumber/(pageError.location?.lineNumber ?? 0)/g' \
    -e 's/pageError\.location\.columnNumber/(pageError.location?.columnNumber ?? 0)/g' \
    "$CB"
  n=$(grep -c '?? ""' "$CB")
  echo "patched: $venv ($n url-default sites)"
done
