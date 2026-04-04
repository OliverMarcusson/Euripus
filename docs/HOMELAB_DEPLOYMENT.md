# Euripus Homelab Deployment

This guide describes the browser-first self-hosted deployment for Euripus behind a reverse proxy.

## Topology

- Reverse proxy terminates HTTPS for your dedicated Euripus host.
- The proxy forwards traffic to the `web` service from `docker-compose.homelab.yml`.
- The `web` service serves the built SPA and proxies `/api/*` to the Rust server.
- PostgreSQL stays private on the Compose network.

## Required Setup

1. Copy `apps/server/.env.example` to `apps/server/.env`.
2. Replace `APP_JWT_SECRET`, `APP_ENCRYPTION_KEY_B64`, and `POSTGRES_PASSWORD` with strong values.
3. Set `APP_PUBLIC_ORIGIN` to your public HTTPS URL, for example `https://euripus.home.arpa`.
4. Set `APP_ALLOWED_ORIGINS` to include that public origin, plus any local development origins you still use.
5. Keep `APP_BROWSER_COOKIE_SECURE=true` for a reverse-proxied HTTPS deployment.

## Start The Stack

```bash
docker compose -f docker-compose.homelab.yml up --build -d
```

By default, the `web` service is published on host port `8088`. Override it with `EURIPUS_WEB_PORT` if needed.

## Reverse Proxy Expectations

Route your dedicated Euripus host to the single upstream `http://YOUR-HOST:8088`.

- Forward the `Host` header unchanged.
- Forward `X-Forwarded-For`, `X-Forwarded-Proto`, and `X-Forwarded-Host`.
- Do not expose the `server` or `postgres` containers directly.

## Runtime Behavior

- Browser auth uses an HTTP-only refresh cookie plus a readable CSRF cookie.
- The SPA uses `/api/*` for all browser traffic.
- Desktop auth remains available on the legacy unprefixed API routes for the Tauri shell.
- `/health` is exposed by the public `web` service and proxied through to the Rust backend.

## Validation Checklist

- `GET /health` returns `204` through the public web upstream.
- Loading `/guide` directly in the browser returns the SPA instead of a 404.
- Registering or logging in from the browser succeeds and survives a full page reload.
- Logging out clears the browser session.
- Saving a provider and triggering sync continue to work.
- Search, favorites, and playback requests succeed through `/api`.
