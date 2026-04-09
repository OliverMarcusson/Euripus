# Euripus

Euripus is a self-hostable IPTV application with a Rust API, a React web client, and a planned native Android TV receiver. It is now a full web deployment with the receiver path kept separate from the browser app.

## Workspace

- `apps/client`: React web client and PWA.
- `apps/server`: Axum + PostgreSQL API with Xtreme Codes sync, auth, favorites, search, and playback contracts.
- `apps/web`: Nginx-based production web service that serves the SPA and proxies `/api` to the Rust server.
- `apps/android-tv-native`: Native Android TV receiver scaffold.
- `packages/shared`: Shared TypeScript contracts for the frontend.

## Local Development

1. Copy `apps/server/.env.example` to `apps/server/.env`.
2. Run `bun install`.
3. Start the full local stack with `bun run dev:start`.
4. Stop it with `bun run dev:stop`.
5. Reset PostgreSQL + Meilisearch and restart everything with `bun run dev:reset`.

The Vite dev server proxies `/api` and `/health` to `http://127.0.0.1:8080`, so the browser client uses the same same-origin API shape in development and production.

`bun run dev:start` builds and starts PostgreSQL + the API in Docker, launches the frontend dev server on `0.0.0.0`, and waits for both the API and frontend to become ready before returning.

For Android TV receiver testing on your local network, use the LAN URL that `bun run dev:start` prints, such as `http://192.168.1.42:5173`, rather than `http://127.0.0.1:5173`.

## Self-Hosted Deployment

Use `docker-compose.selfhosted.yml` for the self-hosted web deployment. The deployment host pulls prebuilt Linux images from GHCR instead of building them locally.

1. Copy `apps/server/.env.example` to `apps/server/.env` and replace the placeholder secrets.
2. Copy `.env.selfhosted-images.example` to `.env.selfhosted-images`.
3. Set `APP_PUBLIC_ORIGIN` to the HTTPS URL exposed by your reverse proxy.
4. Set `APP_ALLOWED_ORIGINS` to include your public browser origin and any local development origins you still need.
5. Publish fresh images with `bun run publish`.
6. On the production host, start or refresh the stack with `bun run prod:start`.
   Stop it with `bun run prod:stop`.
   Reset PostgreSQL + Meilisearch and restart the stack with `bun run prod:reset`.
7. Point your reverse proxy at the host port `8088` by default, or override `EURIPUS_WEB_PORT`.

The `web` service is the only public upstream. It serves the SPA, forwards `/api/*` to the Rust backend, and keeps PostgreSQL private inside the Compose network.

Default image names:

- `ghcr.io/olivermarcusson/euripus-server`
- `ghcr.io/olivermarcusson/euripus-web`

Published tags:

- immutable tag: current git SHA
- moving tag: `selfhosted-latest`

Both helper scripts read `GHCR_USERNAME` and `GHCR_TOKEN` from `.env.selfhosted-images` by default. Use a token with package write access on the publishing machine and a read-only package token on the production host. The deploy script uses `docker` when available and falls back to `podman` automatically.

To route Euripus server-side traffic through NordVPN, add the override file:

```bash
cp apps/server/.env.nordvpn.example apps/server/.env.nordvpn
EURIPUS_ENABLE_NORDVPN=true bun run prod:start
```

That only affects server-originated traffic such as provider validation, sync jobs, and EPG fetches. Browser playback still goes directly from the client device to the IPTV provider.

## Operational Docs

- Server setup handoff: `docs/AI_SERVER_SETUP.md`
- Self-hosted deployment guide: `docs/SELF_HOSTED_DEPLOYMENT.md`
- Future browser and self-hosting plan: `docs/V2_BROWSER_SELF_HOSTED_PLAN.md`
