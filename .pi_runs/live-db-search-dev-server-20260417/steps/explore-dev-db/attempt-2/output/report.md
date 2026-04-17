---
run_id: live-db-search-dev-server-20260417
step_id: explore-dev-db
role: explorer
attempt: 2
created_at: 2026-04-17T00:00:00Z
source_steps: []
repo_root: /home/oliver/src/Rust/Euripus
objective_digest: 083dfeb6b71f1ba2
---

## Goal
Determine, from first-party repo files only, how the local development stack connects to its database, where the database-related environment/config lives, and which commands/files matter if you need to inspect real synced channel/provider data in order to build admin search rules.

## Relevant Files
- `README.md`
- `docker-compose.yml`
- `apps/server/.env.example`
- `apps/server/.env`
- `apps/server/src/main.rs`
- `apps/server/src/config.rs`
- `apps/server/src/server_main/app.rs`
- `apps/server/src/server_main.rs`
- `apps/server/src/server_main/provider.rs`
- `apps/server/src/server_main/sync/runner.rs`
- `apps/server/src/server_main/search/mod.rs`
- `apps/server/src/server_main/search/rules.rs`
- `apps/server/src/server_main/search/indexing.rs`
- `apps/server/src/server_main/admin.rs`
- `apps/server/migrations/0001_init.sql`
- `apps/server/migrations/0019_admin_search_rules.sql`
- `apps/server/migrations/0020_admin_provider_country_relations.sql`
- `docs/AI_SERVER_SETUP.md`
- `docs/admin-search-rule-json.md`
- `apps/client/.env`
- `apps/client/.env.example`
- `apps/client/src/lib/api.ts`
- `apps/client/package.json`

## Findings
1. **Local dev does not appear to use an external/live database.**
   - `apps/server/.env` and `apps/server/.env.example` both set `APP_DATABASE_URL=postgres://euripus:euripus@postgres:5432/euripus`.
   - `docker-compose.yml` defines a `postgres` service and injects `apps/server/.env` into both `postgres` and `server`.
   - `docker-compose.yml` also exposes PostgreSQL on host port `5432:5432`.
   - Based on these files, the local dev API talks to the Compose Postgres container over the internal hostname `postgres`, not to a remote production DB.

2. **Server startup loads `.env`, reads `APP_DATABASE_URL`, waits for Postgres, then migrates automatically.**
   - `apps/server/src/main.rs` calls `dotenvy::dotenv().ok()` and then `euripus_server::run()`.
   - `apps/server/src/config.rs` reads `APP_DATABASE_URL` into `Config.database_url`.
   - `apps/server/src/server_main/app.rs`:
     - builds config from env,
     - calls `wait_for_postgres(&config.database_url)`,
     - runs `sqlx::migrate!("./migrations")`.
   - So the DB connection path is: `.env` → `Config::from_env()` → `wait_for_postgres()` → SQLx `PgPool`.

3. **The local browser dev server talks to the API, not directly to Postgres.**
   - `README.md` says the Vite dev server proxies `/api` and `/health` to `http://127.0.0.1:8080`.
   - `apps/client/.env` uses `VITE_API_BASE_URL=/api`.
   - `apps/client/.env.example` uses `VITE_API_BASE_URL=http://127.0.0.1:8080`.
   - `apps/client/src/lib/api.ts` defaults `API_BASE_URL` to `/api`.
   - So browser-side inspection of real data happens through API routes; DB access is server-side.

4. **Documented local-dev command flow exists, but the actual script implementation is outside the allowed read set.**
   - `README.md` says:
     - copy `apps/server/.env.example` to `apps/server/.env`
     - run `bun install`
     - start with `bun run dev:start`
     - stop with `bun run dev:stop`
     - reset with `bun run dev:reset`
   - `README.md` further says `bun run dev:start` starts PostgreSQL + API in Docker and launches the frontend dev server.
   - I could not inspect the actual `dev:start` script body because root `package.json`/scripts were not in the allowed paths.

5. **The DB stores synced provider/catalog data locally after a provider sync.**
   - `apps/server/migrations/0001_init.sql` creates:
     - `provider_profiles`
     - `channel_categories`
     - `channels`
     - `programs`
     - `sync_jobs`
     - `search_documents`
   - `apps/server/src/server_main/provider.rs` exposes:
     - `GET /provider`
     - `POST /provider/validate`
     - `PUT /provider/xtreme`
     - `POST /provider/sync`
     - `GET /provider/sync-status`
   - `apps/server/src/server_main/sync/runner.rs` validates the provider, fetches categories/channels/EPG, persists them, then refreshes search metadata/indexes.
   - This means “actual channel/provider data” used for search rules is populated only after saving a provider and running sync.

6. **Search-rule metadata is derived from synced channel/program text and stored back into Postgres.**
   - `apps/server/migrations/0019_admin_search_rules.sql` adds these columns:
     - on `channels`: `search_country_code`, `search_provider_name`, `search_is_ppv`, `search_is_vip`
     - on `programs`: same set
   - `apps/server/src/server_main/search/indexing.rs`:
     - loads compiled admin rules,
     - evaluates rules against channel/category/program text,
     - writes derived metadata into those `search_*` columns,
     - rebuilds fallback `search_documents`,
     - optionally rebuilds Meilisearch indexes.
   - So the DB itself becomes the source of truth for rule outputs after indexing runs.

7. **Admin search rules live in dedicated DB tables and are managed through API endpoints.**
   - `apps/server/migrations/0019_admin_search_rules.sql` creates:
     - `admin_search_pattern_groups`
     - `admin_search_patterns`
   - `apps/server/migrations/0020_admin_provider_country_relations.sql` creates:
     - `admin_search_provider_countries`
   - `apps/server/src/server_main/admin.rs` exposes:
     - `GET/POST/DELETE /admin/search/pattern-groups`
     - `POST /admin/search/pattern-group-import`
     - `PUT/DELETE /admin/search/pattern-groups/{id}`
     - `POST /admin/search/test`
     - `POST /admin/search/test-query`
   - `docs/admin-search-rule-json.md` documents the JSON import format.

8. **The most relevant API/files for inspecting real synced data before building rules are:**
   - Provider lifecycle:
     - `apps/server/src/server_main/provider.rs`
     - client calls in `apps/client/src/lib/api.ts`: `getProvider`, `saveProvider`, `triggerProviderSync`, `getSyncStatus`
   - Channel/program inspection:
     - `apps/client/src/lib/api.ts`: `getChannels`, `getGuide`, `getChannelGuide`, `searchChannels`, `searchPrograms`
     - `apps/server/src/server_main/search/mod.rs`: `/search/status`, `/search/filter-options`, `/search/channels`, `/search/programs`
   - Rule logic:
     - `apps/server/src/server_main/search/rules.rs`
     - `apps/server/src/server_main/search/indexing.rs`
     - `apps/server/src/server_main/admin.rs`

9. **Search filters exposed to the client are built from admin rule tables, not discovered directly from current channel data.**
   - `apps/server/src/server_main/search/mod.rs#get_search_filter_options` loads pattern groups and builds country/provider filter options from enabled admin rules.
   - Therefore, to build new rules from actual synced data, you would inspect synced `channels`/`programs` data first, then create/import rule groups, then let reindexing propagate metadata.

10. **Meilisearch is part of local dev, but PostgreSQL remains the core persisted dataset.**
    - `docker-compose.yml` defines a `meilisearch` service.
    - `apps/server/.env(.example)` contains `APP_MEILISEARCH_URL`.
    - `apps/server/src/server_main/app.rs` initializes Meilisearch if configured.
    - `apps/server/src/server_main/search/indexing.rs` can rebuild PostgreSQL fallback search documents and Meilisearch indexes.
    - For “actual channel/provider data,” Postgres tables (`provider_profiles`, `channels`, `programs`) are the durable source; Meilisearch is a search index layer.

## Open Questions
1. `README.md` documents `bun run dev:start`, but the actual script implementation was not readable within the allowed paths. If exact startup commands/scripts are needed, the root script definitions still need inspection.
2. The objective says “live database,” but first-party local-dev files point to a local Compose Postgres container, not a remote/live DB. If “live” meant a shared remote development DB, I found no evidence of that in the allowed files.
3. Direct SQL query examples for inspecting `channels`/`programs` were not present in the allowed files; only schema and API paths were available.
4. `docker-compose.selfhosted.yml` and related prod scripts are referenced in docs but were outside the allowed read set, so I did not compare local-dev DB wiring vs self-hosted/prod wiring.

## Recommended Next Step
Use the local dev stack as documented in `README.md`, then inspect synced data through the existing API flow:
1. start local dev with `bun run dev:start`;
2. save provider credentials via the provider flow (`PUT /provider/xtreme`);
3. trigger sync (`POST /provider/sync`);
4. inspect real data with:
   - `GET /provider`
   - `GET /channels`
   - `GET /guide`
   - `GET /search/channels`
   - `GET /search/programs`
   - `GET /search/filter-options`
5. build/import admin rules through:
   - `POST /admin/search/test`
   - `POST /admin/search/pattern-group-import`
6. verify how rules map onto data by checking the rule logic in:
   - `apps/server/src/server_main/search/rules.rs`
   - `apps/server/src/server_main/search/indexing.rs`