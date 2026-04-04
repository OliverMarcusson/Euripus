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

- Root Compose file: `docker-compose.yml`
- Server env template: `apps/server/.env.example`
- Server image build: `apps/server/Dockerfile`
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

If exposing Euripus beyond a private network, put a reverse proxy in front of the API.

- Terminate TLS at the proxy.
- Restrict the API to HTTPS only.
- Forward `Authorization` headers unchanged.
- Keep request body size moderate because auth and provider endpoints are small.
- Consider IP allow-lists or VPN access for private self-hosting.

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

