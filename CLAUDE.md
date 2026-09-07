# CLAUDE.md

Guidance for working in this repository.

## What this is

**Coffee Tracker** is a web app that replaces a team's paper coffee tally. Users log coffee/tea consumption and get a personal dashboard (caffeine curve, streaks, daily totals), an opt-in leaderboard with "coffee-twin" matching, achievements, stock tracking, and a staff-only Payments/Expenses page (shared tabs, PayPal links). Auth is OAuth (GitLab + Authentik/OIDC) gated by configurable groups, plus a guest login. Current version **1.4.0** (kept in sync between `backend/Cargo.toml` and `frontend/coffee-tracker/package.json`).

Default git branch is `master`. Commits follow Conventional Commits; `chore: bump version` marks releases.

## Architecture

```
Browser ──▶ Frontend (React/Vite, :3123) ──/api──▶ Backend (Rust/axum, :3124) ──▶ SQLite
```

- **Frontend** (`frontend/coffee-tracker/`) — React 19 + TypeScript + Vite 8. Talks to the backend via `/api/v1/...`. In dev, Vite proxies `/api`, `/api-docs`, `/swagger-ui` to `http://127.0.0.1:3124`; the container writes runtime settings to `dist/config.js` when it starts.
- **Backend** (`backend/`) — Rust 2024, axum 0.8 + sqlx (SQLite). Session-cookie auth. Serves Swagger UI at `/swagger-ui`, OpenAPI at `/api-docs/openapi.json`. Ships two binaries: `coffee-tracker` (server) and `coffee-admin` (QR CLI).
- **Database** — single SQLite file. In containers at `/data/coffee-tracker.db` (volume `./coffee_data`). Schema via SQL migrations run at startup.

> Note the port split: **frontend 3123, backend 3124**. The README's non-Docker notes mention 3000/3123 for the backend — the actual code default is `0.0.0.0:3124`.

## Running the stack

Local, built from source (recommended for dev):
```sh
docker compose -f docker-compose.yml -f docker-compose.local.yml up --build
```
Production images (configured through `FRONTEND_IMAGE` and `BACKEND_IMAGE`):
```sh
docker compose up
```
Frontend then at http://localhost:3123.

Backend alone, no Docker:
```sh
cd backend
DATABASE_URL=sqlite://coffee-tracker.db cargo run        # server on :3124, Swagger at /swagger-ui
cargo run --bin coffee-admin -- qr --machine-id main --base-url http://localhost:3124 --out machine-main.png
cargo run --bin openapi-export                            # print OpenAPI JSON
```

Frontend alone:
```sh
cd frontend/coffee-tracker
npm install
npm run generate:api    # regenerate src/api/schema.ts from fixtures/openapi.json — run before dev/build
npm run dev
```

## Backend (`backend/`)

- **Stack:** axum 0.8, tokio, sqlx 0.8 (SQLite, runtime queries — no compile-time checking / no `sqlx-data.json`), tower-http, tower-cookies, reqwest (OAuth + webhooks), utoipa (OpenAPI), clap (admin CLI), chrono/chrono-tz.
- **Entry:** `src/main.rs` → `Config::from_env()` → `db::connect` → `build_app` (in `src/lib.rs`). `AppState { config, db: SqlitePool, http }` is the axum state.
- **Modules under `src/`:**
  - `config.rs` — env parsing. `db.rs` — pool, `migrate!`, `seed_defaults`. `error.rs` — `ApiError`/`ApiResult` (JSON `{error}`, 500s masked).
  - `auth/mod.rs` — OAuth (GitLab + Authentik/OIDC), guest login, sessions, `CurrentUser` extractor (largest file).
  - `catalog/` — machines & beverages. `consumptions/` — log/delete drinks (triggers stock decrement, achievements, notifications).
  - `stats/` — per-user & global stats (`caffeine_decay`, `resolve_tz`). `achievements/` — evaluation engine. `social/` — leaderboard, coffee-twins (cosine similarity), streaks.
  - `payments/` + `expenses.rs` — debt/payment/expense tracking. `stock.rs` — stock items + auto-decrement + low-stock alerts. `notifications/` — Mattermost/Matrix webhooks. `version.rs`, `openapi/`.
- **API:** everything under `/api/v1`. Key groups: `auth/*`, `me`, `machines`, `beverages`, `consumptions`, `stats/me/*`, `stats/global/*`, `achievements/me`, `leaderboard`, `coffee-twins`, `payments/*`, `expenses`, `stock`, `version`. (Full list in Swagger UI.)
- **Auth model:** session cookies, not JWT. Cookie `coffee_session` = random token; DB stores only `SHA256(session_secret : token)`. TTL 180 days, `HttpOnly`/`SameSite=Lax`/`Secure` on https. `CurrentUser` (axum `FromRequestParts`) = authenticated-only. Login is gated by `AUTH_ALLOWED_GROUPS`; membership in `AUTH_STAFF_GROUPS` grants the elevated `stock_manager` role. Guests use `coffee_guest` cookie + a 12-char recovery code (hashed).
- **DB schema:** migrations in `backend/migrations/` (`202604280001_initial.sql` … `202605110003_payment_recipients.sql`). Core tables: `users`, `auth_identities`, `sessions`, `guest_identities`, `machines`, `beverages`, `consumptions`, `achievement_definitions`/`achievement_unlocks`, `payments`, `expense_entries`, `stock_items`.
- **Business rules worth knowing:** prices are integer **cents**; caffeine uses exponential decay; streaks count only weekday "lab days" (Sat/Sun excluded) in the configured timezone; consumptions have a 5-second create-dedupe and a 5-minute delete window; stock auto-decrements per beverage and fires one webhook on crossing the low threshold.
- **Build/test:** `cargo build [--release]`, `cargo test`. Tests are inline `#[cfg(test)] mod tests` in nearly every module (many `#[tokio::test]` spinning up temp SQLite DBs); no separate `tests/` dir.
- **Dockerfile:** multi-stage `rust:1.96.1-trixie` → `debian:trixie-slim`; `EXPOSE 3124`, `VOLUME /data`, `CMD ["coffee-tracker"]`.

## Frontend (`frontend/coffee-tracker/`)

> The real app is one level down; the outer `frontend/` only holds a stub lockfile.

- **Stack:** React 19 (function components + hooks), TypeScript 5.9 (strict, ESM), Vite 8, Tailwind CSS v4 (via `@tailwindcss/vite`, tokens in `src/index.css`, no `tailwind.config.js`), class-based dark mode.
- **Key deps:** `@tanstack/react-query` (all server state — hooks in `src/api/queries.ts`; no Redux), `react-router-dom` v7, `recharts`, `framer-motion` + `canvas-confetti`, `lucide-react`, `idb` (offline queue), `clsx`+`tailwind-merge` (`cn()` in `src/lib/cn.ts`).
- **API layer (`src/api/`):** `client.ts` (typed `fetch` wrapper, `credentials: 'include'`, `ApiError`, `isBackendUnavailableError`), `queries.ts` (React Query hooks + `queryKeys`), `schema.ts` (**generated** from `fixtures/openapi.json` — do not hand-edit; run `npm run generate:api`). Base URL comes from runtime `API_BASE_URL`, with `VITE_API_BASE_URL` as a development fallback. A 401 dispatches a `coffee:unauthorized` window event.
- **Routing (`src/App.tsx`):** `/login` public; everything else under `ProtectedRoute` → `AppShell`: `/` (LogPage, index), `/dashboard`, `/achievements`, `/leaderboard`, `/payments`, `/settings`, `/components` (dev gallery).
- **Pages (`src/pages/`):** LoginPage (GitLab/Authentik OAuth + guest), LogPage (log a coffee, optimistic + IndexedDB offline queue), DashboardPage (Recharts stats), AchievementsPage (confetti on unlock), LeaderboardPage (rankings + coffee-twins), PaymentsPage (debts, mark-paid, PayPal links, expenses), SettingsPage (theme, guest recovery, stock).
- **PWA-ish:** service worker `public/sw.js`; offline consumptions queued in IndexedDB (`lib/offlineQueue.ts`) and replayed via `hooks/useOfflineQueue.ts`.
- **Scripts:** `npm run dev`, `npm run build` (`tsc -b && vite build`), `npm run preview`, `npm run lint` (eslint), `npm run test` (Vitest + jsdom), `npm run generate:api`.
- **Tests:** Vitest, mostly unit tests of pure helpers (`src/lib/*.test.ts`, `src/api/client.test.ts`, `src/pages/*.test.ts`, `AppFooter.test.tsx`).
- Note: `dist/` is a committed prior build (generated output, not source).

## Config / environment

Env is read from the process environment. Root `.env.example` documents the complete setup; the frontend container turns its runtime branding and API values into `/config.js` on startup.

Key variables: `DATABASE_URL`, `BIND_ADDR`, `SESSION_SECRET`, `GITLAB_*` (OAuth), `AUTHENTIK_*` (OIDC), `AUTH_ALLOWED_GROUPS`, `AUTH_STAFF_GROUPS`, `PUBLIC_BASE_URL`, `FRONTEND_BASE_URL`, optional notification/SMTP settings, `TIMEZONE`, and the runtime branding variables listed in `.env.example`.

## CI/CD (`.gitlab-ci.yml`)

Three stages:
- **verify** — `backend_verify` (`rust:1.88-bookworm`: `cargo test` + `cargo build --release`) and `frontend_verify` (`node:22-bookworm`: `npm ci` → lint, test, build). Run on MRs and branches.
- **deploy** — Kaniko jobs build/push the configured Docker Hub repositories (`:latest` + `:${CI_COMMIT_SHORT_SHA}`), default branch only.
- **sync** — after a successful default-branch pipeline, publishes a clean snapshot commit to GitHub without exposing private GitLab history.

## Data & housekeeping notes

- Live DB persists in `coffee_data/coffee-tracker.db` (mounted to `/data`). `backend/coffee-tracker.db` is a local dev DB. All `*.db` are gitignored.
- `coffee_data/coffee-tracker.db.db` (double extension) looks like an accidental artifact.
