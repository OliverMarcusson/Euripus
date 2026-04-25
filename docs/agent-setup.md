# Euripus Server Setup For Another AI Agent

This document is written as an implementation handoff for another AI agent that needs to deploy the Euripus Rust server on a Linux or Windows host with Docker Compose.

## Goal

Bring up the Euripus server and PostgreSQL using Docker Compose so the desktop app can authenticate users, store Xtreme Codes credentials securely, sync catalog data, keep favorites and recents server-side, and serve playback/search contracts.

## Preconditions

- Docker Engine is installed.
- Docker Compose v2 is installed.
- The repository is present on the target machine.
- Port `8080` is available for the API.
- Port `5432` is either available or intentionally remapped.
- You can provide strong secrets for JWT signing and credential encryption.

## Files To Know

- Local/dev Compose file: `docker-compose.yml`
- Self-hosted Compose file: `docker-compose.selfhosted.yml`
- NordVPN self-hosted override: `docker-compose.selfhosted.nordvpn.yml`
- Server env template: `apps/server/.env.example`
- NordVPN env template: `apps/server/.env.nordvpn.example`
- Self-hosted image env template: `.env.selfhosted-images.example`
- Publish script: `scripts/publish-images.ps1`
- Deploy script: `scripts/deploy.sh`
- Database migration: `apps/server/migrations/0001_init.sql`

## Required Environment Values

Create `apps/server/.env` from `apps/server/.env.example` and replace the placeholders before a real deployment.

- `APP_BIND_ADDRESS`
  Use `0.0.0.0:8080` unless there is a reverse proxy terminating on a different internal port.
- `APP_DATABASE_URL`
  Example: `postgres://euripus:euripus@postgres:5432/euripus`
- `APP_JWT_SECRET`
  Must be long and random.
- `APP_ENCRYPTION_KEY_B64`
  Must be base64 for exactly 32 raw bytes. This key encrypts stored Xtreme Codes passwords.
- `APP_ACCESS_TOKEN_MINUTES`
  Default `15`.
- `APP_REFRESH_TOKEN_DAYS`
  Default `30`.
- `APP_ALLOWED_ORIGINS`
  Comma-separated origin allow-list for CORS. Include your browser origin and any local development origins you still need.
- `APP_PUBLIC_ORIGIN`
  Public HTTPS origin exposed by your reverse proxy. Used to decide secure browser cookie behavior.
- `APP_BROWSER_COOKIE_SECURE`
  Keep `true` for HTTPS deployments behind a reverse proxy.
- `VPN_TYPE`
  Optional. Set to `openvpn` or `wireguard` in `apps/server/.env.nordvpn` when using the NordVPN Compose override.
- `OPENVPN_USER`
  Optional. NordVPN OpenVPN service credential username.
- `OPENVPN_PASSWORD`
  Optional. NordVPN OpenVPN service credential password.
- `WIREGUARD_PRIVATE_KEY`
  Optional. NordVPN WireGuard private key.
- `SERVER_COUNTRIES`
  Optional. Comma-separated preferred NordVPN countries.
- `RUST_LOG`
  Default `info`.

## Deployment Steps

1. Copy the env template.
   `cp apps/server/.env.example apps/server/.env`
2. Replace `APP_JWT_SECRET` with a strong random secret.
3. Replace `APP_ENCRYPTION_KEY_B64` with a new base64-encoded 32-byte key.
4. Confirm `APP_DATABASE_URL` points at the Compose PostgreSQL service or your external PostgreSQL instance.
5. Start the database first if you want to validate connectivity in stages.
   `docker compose up -d postgres`
6. Start the full backend stack.
   `docker compose up --build -d server`
7. Check health by requesting the API health endpoint.
   `curl -i http://127.0.0.1:8080/health`
8. Review logs if needed.
   `docker compose logs -f server`

## Validation Checklist

- `GET /health` returns `204`.
- The server logs show successful SQL migration execution.
- Registering a user succeeds through `POST /auth/register`.
- Saving a provider succeeds through `PUT /provider/xtreme`.
- Triggering sync succeeds through `POST /provider/sync`.
- Search results appear from `GET /search?q=...` after a sync completes.
- Favorites persist through `POST /favorites/:channelId` and `GET /favorites`.

## Reverse Proxy Guidance

If exposing Euripus as a browser service, put a reverse proxy in front of the `web` service from `docker-compose.selfhosted.yml`.

- Terminate TLS at the proxy.
- Route a dedicated host to the `web` service upstream.
- Forward `Authorization` headers unchanged.
- Forward `X-Forwarded-For`, `X-Forwarded-Proto`, and `X-Forwarded-Host`.
- Keep request body size moderate because auth and provider endpoints are small.
- Consider IP allow-lists or VPN access for private self-hosting.

## Optional NordVPN Container Routing

If you want the Euripus server to perform provider validation, sync jobs, and EPG fetches through NordVPN, use:

`EURIPUS_ENABLE_NORDVPN=true bun run prod:start`

That override runs a Gluetun container with NordVPN settings from `apps/server/.env.nordvpn` and shares its network namespace with the Rust server. The browser-facing `web` service then proxies `/api` traffic to the Gluetun container.

## GHCR Self-Hosted Workflow

For the browser-first self-hosted deployment, the target Fedora host should pull prebuilt images instead of building them locally.

1. On the Windows workstation, publish fresh `linux/amd64` images with:
   `bun run publish`
2. On the Fedora host, copy `.env.selfhosted-images.example` to `.env.selfhosted-images`.
3. Set `GHCR_USERNAME` and `GHCR_TOKEN` to a GitHub account and a package token. Use package write access on the Windows publisher and package read access on the Fedora deploy host.
4. Optionally pin `EURIPUS_IMAGE_TAG` to a published git SHA instead of `selfhosted-latest`.
5. Deploy with:
   `bun run prod:start`

The deploy script prefers `docker` and falls back to `podman` automatically on Fedora-style hosts.

Default image names:

- `ghcr.io/olivermarcusson/euripus-server`
- `ghcr.io/olivermarcusson/euripus-web`

This does not proxy the actual IPTV playback stream through NordVPN. Playback remains client-to-provider in v1.

## Operational Notes

- The server migrates the database automatically on startup.
- Provider credentials are encrypted before being stored in PostgreSQL.
- Media playback is not proxied through Euripus in v1; clients stream directly from the IPTV provider.
- Periodic sync jobs are queued automatically for stale provider profiles.
- If you rotate `APP_ENCRYPTION_KEY_B64`, previously stored provider passwords become unreadable unless you re-encrypt them during a migration.

## Safe Recovery Steps

If the server is up but users report empty catalogs:

1. Check `docker compose logs -f server`.
2. Verify the provider status via `GET /provider`.
3. Check the latest sync job via `GET /provider/sync-status`.
4. Trigger a manual sync again.
5. If provider validation is failing, re-save the Xtreme credentials.

## If The Deployment Needs To Become Internet-Facing

Keep the API and database separate conceptually even if they share one host now.

- Put PostgreSQL on a private network.
- Back up the database regularly.
- Add log shipping or structured log collection.
- Add per-user rate limiting at the reverse proxy.
- Add monitoring for `5xx` responses and sync job failures.
