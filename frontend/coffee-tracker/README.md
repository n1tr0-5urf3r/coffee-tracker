# Coffee Tracker Frontend

React + TypeScript + Vite frontend for the Coffee Tracker backend.

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
