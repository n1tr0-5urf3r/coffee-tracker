#!/usr/bin/env bash
# Regenerate the OpenAPI spec from the Rust backend and the TypeScript types from it.
#   backend (utoipa) --openapi-export--> <frontend>/fixtures/openapi.json --openapi-typescript--> <frontend>/src/api/schema.ts
# Run after changing API types/handlers or updating dependencies. Renovate runs it as a postUpgradeTask.
#   --frontend-only   skip the Rust build and only regenerate schema.ts from the committed openapi.json
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
frontend="$root/frontend/coffee-tracker"

if [[ "${1:-}" != "--frontend-only" ]]; then
  tmp="$(mktemp)"
  (cd "$root/backend" && cargo run --quiet --locked --bin openapi-export) > "$tmp"
  mv "$tmp" "$frontend/fixtures/openapi.json"
fi

cd "$frontend"
npm ci --ignore-scripts --no-audit --no-fund --loglevel=error
npm run --silent generate:api
