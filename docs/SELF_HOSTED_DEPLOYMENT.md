# Euripus Self-Hosted Deployment

This guide describes the browser-first self-hosted deployment for Euripus behind a reverse proxy.

## Topology

- Reverse proxy terminates HTTPS for your dedicated Euripus host.
- The proxy forwards traffic to the `web` service from `docker-compose.selfhosted.yml`.
- The `web` service serves the built SPA and proxies `/api/*` to the Rust server.
- PostgreSQL stays private on the Compose network.

## Required Setup

1. Copy `apps/server/.env.example` to `apps/server/.env`.
2. Copy `.env.selfhosted-images.example` to `.env.selfhosted-images`.
3. Replace `APP_JWT_SECRET`, `APP_ENCRYPTION_KEY_B64`, and `POSTGRES_PASSWORD` with strong values.
4. Set `APP_PUBLIC_ORIGIN` to your public HTTPS URL, for example `https://euripus.home.arpa`.
5. Set `APP_ALLOWED_ORIGINS` to include that public origin, plus any local development origins you still use.
6. Keep `APP_BROWSER_COOKIE_SECURE=true` for a reverse-proxied HTTPS deployment.

## Publish Images

Run image builds on the Windows workstation with Docker Desktop and push them to private GHCR packages:

```powershell
bun run publish
```

On Linux or macOS with bash available, run the equivalent publisher with:

```bash
bun run publish
```

The publish script builds `linux/amd64` images for:

- `ghcr.io/olivermarcusson/euripus-server`
- `ghcr.io/olivermarcusson/euripus-web`

It pushes two tags for each image:

- the current git SHA
- `selfhosted-latest`

If `GHCR_USERNAME` and `GHCR_TOKEN` are set in the Windows environment, the script logs in to GHCR before pushing. Otherwise it assumes you have already run `docker login ghcr.io`.
By default it also loads `.env.selfhosted-images`, so you usually do not need to export anything manually. Override that path with `EURIPUS_PUBLISH_ENV_FILE` if needed.

## Deploy On Fedora

Store your private GHCR read credentials in `.env.selfhosted-images` on the Fedora host:

- `GHCR_USERNAME`
- `GHCR_TOKEN`

The Fedora token should have package read access only. Keep it non-interactive and host-local.
The deploy script prefers `docker` and automatically falls back to `podman` if `docker` is not installed.

Deploy the latest published images with:

```bash
bun run prod:start
```

The deploy script now waits for PostgreSQL and the server health check, and it automatically repairs SQLx migration checksum drift before the new server starts.

Deploy a specific immutable revision by setting `EURIPUS_IMAGE_TAG` in `.env.selfhosted-images` to the published git SHA.

By default, the deploy script pulls and starts:

- `ghcr.io/olivermarcusson/euripus-server:${EURIPUS_IMAGE_TAG:-selfhosted-latest}`
- `ghcr.io/olivermarcusson/euripus-web:${EURIPUS_IMAGE_TAG:-selfhosted-latest}`

By default, the `web` service is published on host port `8088`. Override it with `EURIPUS_WEB_PORT` if needed.

## Optional NordVPN Routing

If you want Euripus server-side traffic to leave through NordVPN, enable it in `.env.selfhosted-images`:

```bash
EURIPUS_ENABLE_NORDVPN=true bun run prod:start
```

This override adds a `gluetun` container configured for NordVPN and places the Rust server inside Gluetun's network namespace.

Copy `apps/server/.env.nordvpn.example` to `apps/server/.env.nordvpn`, then set the NordVPN values there:

- `VPN_TYPE=openvpn` with `OPENVPN_USER` and `OPENVPN_PASSWORD`
- Or `VPN_TYPE=wireguard` with `WIREGUARD_PRIVATE_KEY`
- Optional server selectors such as `SERVER_COUNTRIES`, `SERVER_REGIONS`, `SERVER_CITIES`, `SERVER_HOSTNAMES`, and `SERVER_CATEGORIES`

Use NordVPN service credentials for OpenVPN, not your regular Nord Account email and password.

If you run `Euripus-Sports` as a separate Podman container on the same host, point the NordVPN server override at the container hostname instead of a host loopback port:

```bash
APP_SPORTS_API_BASE_URL=http://sports-api:3000
```

With that hostname configured, `bun run prod:start` and `bun run prod:reset` automatically connect the external `sports-api` container to the Euripus compose bridge so the server can reach it without going through `127.0.0.1`.
Those scripts also wait for `GET /health` on the Sports API before reporting Euripus as ready, because the Sports API may spend several minutes doing its first refresh before it starts listening on port `3000`.

Important limitation:

- This only routes server-originated traffic through NordVPN.
- Browser playback still connects directly to the IPTV provider.
- If you need stream playback itself to use NordVPN, the client device must also be on VPN.

## Reverse Proxy Expectations

Route your dedicated Euripus host to the single upstream `http://YOUR-HOST:8088`.

- Forward the `Host` header unchanged.
- Forward `X-Forwarded-For`, `X-Forwarded-Proto`, and `X-Forwarded-Host`.
- Do not expose the `server` or `postgres` containers directly.

## Runtime Behavior

- Browser auth uses an HTTP-only refresh cookie plus a readable CSRF cookie.
- The SPA uses `/api/*` for all browser traffic.
- `/health` is exposed by the public `web` service and proxied through to the Rust backend.
- With the NordVPN override enabled, the `web` service proxies `/api` and `/health` to the `gluetun` container, which exposes the Rust server on port `8080` inside the VPN network namespace.

## Validation Checklist

- `docker compose -f docker-compose.selfhosted.yml images` shows the GHCR image references rather than local build contexts.
- `GET /health` returns `204` through the public web upstream.
- Loading `/guide` directly in the browser returns the SPA instead of a 404.
- Registering or logging in from the browser succeeds and survives a full page reload.
- Logging out clears the browser session.
- Saving a provider and triggering sync continue to work.
- Search, favorites, and playback requests succeed through `/api`.
