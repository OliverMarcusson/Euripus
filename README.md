# Euripus

Euripus is a self-hostable IPTV application with a Rust API, a React web client, and a Tauri desktop shell. It can now be deployed behind a reverse proxy as a browser-first homelab service while keeping the desktop client path intact.

## Workspace

- `apps/desktop`: React client used by both the browser build and the Tauri desktop shell.
- `apps/server`: Axum + PostgreSQL API with Xtreme Codes sync, auth, favorites, search, and playback contracts.
- `apps/web`: Nginx-based production web service that serves the SPA and proxies `/api` to the Rust server.
- `packages/shared`: Shared TypeScript contracts for the frontend.

## Local Development

1. Copy `apps/server/.env.example` to `apps/server/.env`.
2. Run `bun install`.
3. Start backend dependencies with `bun run dev:db`.
4. Run the Rust API with `bun run dev:server`.
5. Run the React client with `bun run dev:desktop`.
6. Run the Tauri shell with `bun run tauri:dev`.

The Vite dev server proxies `/api` and `/health` to `http://127.0.0.1:8080`, so the browser client uses the same same-origin API shape in development and production.

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
- `bun run user-test:start:dev`
  Starts the same user-test stack through the dynamic foreground launcher.
- `bun run user-test:start:dev:web`
  Starts the dynamic foreground launcher in web-only mode and opens the browser.
- `bun run user-test:stop`
  Stops the launched desktop/web process and shuts down the Docker services.

## Homelab Deployment

Use `docker-compose.homelab.yml` for the browser-first self-hosted deployment. The homelab host now pulls prebuilt Linux images from GHCR instead of building them locally.

1. Copy `apps/server/.env.example` to `apps/server/.env` and replace the placeholder secrets.
2. Copy `.env.homelab-images.example` to `.env.homelab-images`.
3. Set `APP_PUBLIC_ORIGIN` to the HTTPS URL exposed by your reverse proxy.
4. Set `APP_ALLOWED_ORIGINS` to include your public browser origin and any local development origins you still need.
5. On your Windows workstation, publish fresh images with `bun run homelab:publish`.
   On Linux or macOS, use `bun run homelab:publish:sh` or `./scripts/publish-homelab-images.sh`.
6. On the Fedora homelab host, deploy them with `./scripts/deploy-homelab-images.sh`.
7. Point your reverse proxy at the host port `8088` by default, or override `EURIPUS_WEB_PORT`.

The `web` service is the only public upstream. It serves the SPA, forwards `/api/*` to the Rust backend, and keeps PostgreSQL private inside the Compose network.

Default image names:

- `ghcr.io/olivermarcusson/euripus-server`
- `ghcr.io/olivermarcusson/euripus-web`

Published tags:

- immutable tag: current git SHA
- moving tag: `homelab-latest`

Both helper scripts read `GHCR_USERNAME` and `GHCR_TOKEN` from `.env.homelab-images` by default. Use a token with package write access on the Windows publisher and a read-only package token on the Fedora host. The Fedora deploy script uses `docker` when available and falls back to `podman` automatically.

To route Euripus server-side traffic through NordVPN, add the override file:

```bash
cp apps/server/.env.nordvpn.example apps/server/.env.nordvpn
EURIPUS_ENABLE_NORDVPN=true ./scripts/deploy-homelab-images.sh
```

That only affects server-originated traffic such as provider validation, sync jobs, and EPG fetches. Playback still goes directly from the client device to the IPTV provider.

## Operational Docs

- Server setup handoff: `docs/AI_SERVER_SETUP.md`
- Homelab deployment guide: `docs/HOMELAB_DEPLOYMENT.md`
- Future browser and self-hosting plan: `docs/V2_BROWSER_SELF_HOSTED_PLAN.md`
