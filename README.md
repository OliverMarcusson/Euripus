# Euripus

Euripus is a Windows-first IPTV desktop client built with Tauri v2, React, Bun, and a Rust backend service. The backend is designed to remain reusable for a future Android TV client.

## Workspace

- `apps/desktop`: Tauri v2 + React desktop client.
- `apps/server`: Axum + PostgreSQL API with Xtreme Codes sync, auth, favorites, search, and playback contracts.
- `packages/shared`: Shared TypeScript contracts for the frontend.

## Local Development

1. Copy `apps/server/.env.example` to `apps/server/.env`.
2. Run `bun install`.
3. Start backend dependencies with `bun run dev:db`.
4. Run the Rust API with `bun run dev:server`.
5. Run the desktop web UI with `bun run dev:desktop`.
6. Run the Tauri shell with `bun run tauri:dev`.

## User Testing

Use one command to bring up the full local stack for real user testing:

- `bun run user-test:start`

That command will:

- build and start PostgreSQL + the API in Docker
- launch the desktop frontend and Tauri shell
- wait for the API and frontend to become ready before returning

Useful variants:

- `bun run user-test:start:web`
  Starts the API + web client only and opens the browser at `http://127.0.0.1:5173`.
- `bun run user-test:stop`
  Stops the launched desktop/web process and shuts down the Docker services.

## Docker Compose

The backend is built around Docker Compose for local development. PostgreSQL and the server can run together in containers, while the desktop app keeps running locally through Bun and Tauri.

## Operational Docs

- Server setup handoff: `docs/AI_SERVER_SETUP.md`
- Future browser and self-hosting plan: `docs/V2_BROWSER_SELF_HOSTED_PLAN.md`
