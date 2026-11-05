# Coffee Tracker Frontend

React + TypeScript + Vite frontend for the Coffee Tracker backend.

## Simple view

Enable **Settings → Simple view** for a quieter layout. The switch is saved to
that account and restored on other devices after signing in. The full view is
the default. Simple view keeps drink logging, prices, daily counts, undo, and
offline status; navigation opens from **Menu**, and machine chores expand on demand.
The menu contains Log, Stats, Payments, and Settings. Stats shows personal drink
totals, and Payments shows the account’s amount due with options to pay and mark the tab paid.

## Development

Run the backend first on `127.0.0.1:3124`, then:

```sh
npm install
npm run generate:api
npm run dev
```

The Vite dev server proxies `/api`, `/api-docs`, and `/swagger-ui` to the
backend. To point at a different backend:

```sh
VITE_API_PROXY_TARGET=http://127.0.0.1:3124 npm run dev
```

## Preview Build

For a local `vite preview`, the API origin can still be supplied at build time:

```sh
VITE_API_BASE_URL=http://127.0.0.1:3124 npm run build
npm run preview
```

The Docker image uses runtime configuration instead: its startup command writes
`dist/config.js` from `API_BASE_URL`, `APP_NAME`, `ORGANIZATION_*`, `LOGO_URL`,
and the other branding environment variables. A deployment can therefore reuse
the same image without rebuilding it.

## Checks

```sh
npm run build
npm run test
npm run lint
```
