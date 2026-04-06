# Performance Improvements + Meilisearch Integration

## Context

Sync and search are both unacceptably slow. The root causes are:
- Two dead GIN indexes on `programs` maintained on every sync INSERT
- `rebuild_search_documents` runs inside the main sync transaction, blocking commit while inserting potentially millions of rows and updating GIN indexes
- Built-in Xtream XMLTV fetch is sequential after external EPG sources complete
- Every search fires two queries with the same expensive GIN+trigram predicate
- PostgreSQL GIN+trigram is not well-suited for fuzzy/relevance search at scale

Meilisearch replaces the PostgreSQL full-text/trigram search path with a purpose-built search engine, eliminating the `search_documents` GIN indexes as a sync bottleneck and delivering millisecond search.

---

## Phase 1 — Database Fixes (Migration 0009)

**File**: `apps/server/migrations/0009_search_and_session_indexes.sql`

```sql
-- Drop unused GIN indexes on programs (maintained on every sync INSERT, never queried)
DROP INDEX IF EXISTS programs_search_tsv_idx;
DROP INDEX IF EXISTS programs_search_trgm_idx;

-- Session token lookup (WHERE refresh_token_hash = $1, no index today)
CREATE INDEX IF NOT EXISTS sessions_refresh_token_hash_idx
  ON sessions (refresh_token_hash);

-- Periodic sync worker scan (WHERE status = 'valid', no index today)
CREATE INDEX IF NOT EXISTS provider_profiles_status_idx
  ON provider_profiles (status);
```

---

## Phase 2 — Sync Pipeline Fixes

### 2a. Move `rebuild_search_documents` outside the main transaction

**File**: `apps/server/src/main.rs`

Currently `persist_full_sync_data` and `persist_epg_sync_data` both run `rebuild_search_documents` inside their transaction, meaning the GIN index updates for potentially millions of rows must complete before commit.

Change: call `rebuild_search_documents` **after** `transaction.commit()`, using the pool directly. This means live data is visible immediately; search index catches up moments later.

```rust
// persist_full_sync_data — after transaction.commit():
transaction.commit().await?;
rebuild_search_documents(&state.pool, user_id).await?;  // now takes &PgPool, not &mut Transaction
```

Change `rebuild_search_documents` signature from `transaction: &mut Transaction<'_, Postgres>` to `pool: &PgPool`, and use it directly.

### 2b. Wrap search document rebuild with index disable/enable

Inside the new `rebuild_search_documents(pool, user_id)`:

Drop and recreate the GIN indexes around the bulk INSERT rather than maintaining them row-by-row:
```sql
DROP INDEX IF EXISTS search_documents_tsv_idx;
DROP INDEX IF EXISTS search_documents_trgm_idx;
-- ... DELETE + INSERT ...
CREATE INDEX search_documents_tsv_idx ON search_documents USING GIN (tsv);
CREATE INDEX search_documents_trgm_idx ON search_documents USING GIN (search_text gin_trgm_ops);
```

This is significantly faster than maintaining GIN indexes during INSERT. The indexes are built once at the end.

Note: this approach is only safe once Meilisearch is the primary search path (Phase 3), since it leaves the indexes absent during the rebuild window. Until then, keep the indexes and just move the call outside the transaction (2a is enough).

### 2c. Fetch built-in Xtream XMLTV concurrently

**File**: `apps/server/src/main.rs`, `fetch_epg_feeds` function (~line 3822)

Currently external sources are fetched via `JoinSet` then `xtreme::fetch_xmltv` is called after the join loop completes. The built-in fetch (often slow) blocks the entire EPG phase.

Change: spawn `xtreme::fetch_xmltv` into the same `JoinSet` at startup alongside external sources. Use an enum to distinguish result types:

```rust
enum EpgFetchResult {
    External(ExternalEpgFetchResult),
    BuiltIn(Result<XmltvFeed>),
}
```

Spawn all at once, collect results in join loop. This means the 10-minute XMLTV timeout for the built-in feed runs concurrently with external source fetches.

---

## Phase 3 — Meilisearch Integration

### 3a. Add Meilisearch service to Docker Compose

**File**: `docker-compose.yml` — add service:
```yaml
  meilisearch:
    image: getmeili/meilisearch:v1.15
    environment:
      MEILI_MASTER_KEY: ${MEILI_MASTER_KEY:-change-me-dev-key}
      MEILI_ENV: development
    volumes:
      - meilisearch-data:/meili_data

volumes:
  meilisearch-data:
```

**File**: `docker-compose.selfhosted.yml` — add same service with `restart: unless-stopped` and no host port binding:
```yaml
  meilisearch:
    image: docker.io/getmeili/meilisearch:v1.15
    restart: unless-stopped
    env_file:
      - apps/server/.env
    volumes:
      - euripus-meilisearch-data:/meili_data

volumes:
  euripus-meilisearch-data:
```

Add `depends_on: meilisearch` to the `server` service in both files.

### 3b. Add config and dependency

**File**: `apps/server/Cargo.toml` — add:
```toml
meilisearch-sdk = { version = "0.28", default-features = false, features = ["reqwest-rustls-tls"] }
tokio-retry = "0.3"
```

**File**: `apps/server/src/config.rs` — add optional fields:
```rust
pub meilisearch_url: Option<String>,
pub meilisearch_api_key: Option<String>,
```
Load from `APP_MEILISEARCH_URL` and `APP_MEILISEARCH_API_KEY` using `read_optional_env`. If absent, search falls back to PostgreSQL.

**File**: `apps/server/.env.example` — document new vars:
```
APP_MEILISEARCH_URL=http://meilisearch:7700
APP_MEILISEARCH_API_KEY=change-me-dev-key
```

### 3c. Add Meilisearch client to AppState

**File**: `apps/server/src/main.rs`

```rust
use meilisearch_sdk::client::Client as MeilisearchClient;

struct AppState {
    pool: PgPool,
    config: Arc<Config>,
    http_client: reqwest::Client,
    meili: Option<Arc<MeilisearchClient>>,  // None = Postgres fallback
}
```

Build and configure indexes at startup (after pool creation):
```rust
async fn setup_meilisearch(config: &Config) -> Result<Option<Arc<MeilisearchClient>>> {
    let Some(url) = &config.meilisearch_url else { return Ok(None) };
    let client = MeilisearchClient::new(url, config.meilisearch_api_key.as_deref())?;
    // Configure channels index
    let channels_index = client.index("channels");
    channels_index.set_filterable_attributes(["user_id"]).await?;
    channels_index.set_searchable_attributes(["title", "subtitle", "search_text"]).await?;
    // Configure programs index
    let programs_index = client.index("programs");
    programs_index.set_filterable_attributes(["user_id"]).await?;
    programs_index.set_searchable_attributes(["title", "subtitle", "search_text"]).await?;
    programs_index.set_sortable_attributes(["sort_priority", "starts_at"]).await?;
    Ok(Some(Arc::new(client)))
}
```

### 3d. Meilisearch document types

Add to `main.rs`:

```rust
#[derive(Serialize, Deserialize)]
struct MeiliChannelDoc {
    id: String,          // "{user_id}_{entity_id}"
    user_id: String,
    entity_id: String,
    title: String,
    subtitle: Option<String>,
    search_text: String,
}

#[derive(Serialize, Deserialize)]
struct MeiliProgramDoc {
    id: String,          // "{user_id}_{entity_id}"
    user_id: String,
    entity_id: String,
    title: String,
    subtitle: Option<String>,  // channel_name
    search_text: String,
    starts_at: i64,      // Unix timestamp for sort
    ends_at: i64,
    can_catchup: bool,
    channel_id: Option<String>,
    sort_priority: i32,  // computed at index time: 0=live, 1=catchup, 2=upcoming, 3=past
}
```

`sort_priority` is computed at index time rather than query time — simpler and faster.

### 3e. Implement `rebuild_meili_indexes`

The existing `rebuild_search_documents` (Postgres) is kept as fallback. Add a parallel Meilisearch path called after sync commits:

```rust
async fn rebuild_meili_indexes(meili: &MeilisearchClient, user_id: Uuid, pool: &PgPool) -> Result<()> {
    let user_id_str = user_id.to_string();

    // Delete existing docs for this user
    meili.index("channels")
        .delete_documents_by_filter(format!("user_id = {user_id_str}"))
        .await?;
    meili.index("programs")
        .delete_documents_by_filter(format!("user_id = {user_id_str}"))
        .await?;

    // Fetch channels from Postgres and push to Meilisearch in batches of 10,000
    // Fetch programs from Postgres and push to Meilisearch in batches of 10,000
}
```

Use keyset pagination (ORDER BY id, LIMIT 10000) for the Postgres fetches to avoid OFFSET performance degradation on large tables.

### 3f. Update `search_channels` and `search_programs` handlers

Replace the two-query Postgres pattern with Meilisearch when `state.meili.is_some()`:

```rust
async fn search_channels(...) {
    if let Some(meili) = &state.meili {
        return search_channels_meili(meili, ...).await;
    }
    // existing Postgres path
}

async fn search_channels_meili(meili: &MeilisearchClient, user_id: Uuid, term: &str, offset: i64, limit: i64) -> ... {
    let results = meili.index("channels")
        .search()
        .with_query(term)
        .with_filter(&format!("user_id = {}", user_id))
        .with_offset(offset as usize)
        .with_limit(limit as usize)
        .execute::<MeiliChannelDoc>()
        .await?;

    let entity_ids: Vec<Uuid> = results.hits.iter()
        .map(|hit| Uuid::parse_str(&hit.result.entity_id).unwrap())
        .collect();

    // Fetch full channel rows from Postgres by ID (IN clause)
    // Return ChannelSearchResponse with total_count from results.estimated_total_hits
}
```

`estimated_total_hits` replaces the separate COUNT query entirely.

### 3g. Combine search COUNT+data (Postgres fallback path)

Even on the Postgres fallback, eliminate the double-query by using a window function:

```sql
WITH matches AS (
  SELECT sd.entity_id,
         COUNT(*) OVER () AS total_count,
         ROW_NUMBER() OVER (ORDER BY ...) AS ordinal
  FROM search_documents sd
  WHERE sd.user_id = $1
    AND sd.entity_type = 'channel'
    AND (sd.tsv @@ plainto_tsquery('simple', $2) OR sd.search_text % $2)
)
SELECT entity_id, total_count FROM matches
OFFSET $3 LIMIT $4
```

This returns `total_count` alongside results in one pass.

### 3h. Migration 0010 — drop search_documents GIN indexes

Once Meilisearch is the primary search path, drop the GIN indexes to stop paying the maintenance cost during `rebuild_search_documents`:

**File**: `apps/server/migrations/0010_drop_search_document_gin_indexes.sql`
```sql
DROP INDEX IF EXISTS search_documents_tsv_idx;
DROP INDEX IF EXISTS search_documents_trgm_idx;
-- Keep the table and B-tree indexes for the Postgres fallback path
```

The generated `tsv` column can be removed eventually, but keeping the table with just the B-tree indexes is fine for now.

---

## Phase 4 — Session Auth Cache

**File**: `apps/server/src/main.rs`

Add `dashmap` to `Cargo.toml`:
```toml
dashmap = "6"
```

Add to `AppState`:
```rust
session_cache: Arc<dashmap::DashMap<(Uuid, Uuid), std::time::Instant>>,
```

In `require_auth`, check cache before querying Postgres:
```rust
let cache_key = (session_id, user_id);
let now = std::time::Instant::now();
if let Some(expiry) = state.session_cache.get(&cache_key) {
    if *expiry > now {
        return Ok(AuthContext { user_id, session_id });
    }
}
// ... DB query ...
// On success, cache with 30-second TTL:
state.session_cache.insert(cache_key, now + Duration::from_secs(30));
```

Evict on session revoke by calling `state.session_cache.remove(&cache_key)` in the revoke handler.

---

## Files Changed

| File | Change |
|---|---|
| `apps/server/migrations/0009_search_and_session_indexes.sql` | New — drop unused programs GIN indexes, add refresh_token_hash + provider_profiles.status indexes |
| `apps/server/migrations/0010_drop_search_document_gin_indexes.sql` | New — drop search_documents GIN indexes after Meilisearch is live |
| `apps/server/src/config.rs` | Add `meilisearch_url`, `meilisearch_api_key` optional fields |
| `apps/server/src/main.rs` | AppState, sync pipeline, search handlers, session cache |
| `apps/server/Cargo.toml` | Add `meilisearch-sdk`, `dashmap` |
| `docker-compose.yml` | Add `meilisearch` service + volume |
| `docker-compose.selfhosted.yml` | Add `meilisearch` service + volume |
| `apps/server/.env.example` | Document `APP_MEILISEARCH_URL`, `APP_MEILISEARCH_API_KEY`, `MEILI_MASTER_KEY` |

---

## Implementation Order

1. **Migration 0009** — standalone, no code changes needed
2. **Phase 2a** — move rebuild outside transaction (immediate sync speedup)
3. **Phase 2c** — concurrent XMLTV fetch (immediate sync speedup)
4. **Phase 3a–3c** — Meilisearch infra + config + AppState scaffolding
5. **Phase 3d–3f** — document types + index rebuild + search handlers (core feature)
6. **Phase 3g** — combine COUNT+data in Postgres fallback
7. **Migration 0010** — drop GIN indexes after Meilisearch confirmed working
8. **Phase 2b** — drop/recreate GIN around rebuild (after 0010)
9. **Phase 4** — session cache (independent, can be done any time)

---

## Verification

- **Sync**: trigger a full sync; the "rebuilding-search" phase should complete significantly faster with rebuild outside the transaction
- **Search**: `GET /search/channels?q=...` and `GET /search/programs?q=...` should return noticeably faster with Meilisearch
- **Meilisearch health**: `curl http://localhost:7700/health` should return `{"status":"available"}`
- **Fallback**: set `APP_MEILISEARCH_URL` to an invalid value; search should still work via Postgres
- **Session cache**: revoke a session; the next request with that token should be rejected within one cache TTL (30s)
- **Migration 0009**: `EXPLAIN SELECT ... FROM sessions WHERE refresh_token_hash = $1` should show an Index Scan, not a Seq Scan
