# Remaining Performance Improvement Action Plan

## Goal
Improve user-perceived responsiveness, reduce unnecessary network and database load, and remove the main scaling bottlenecks still left in the client and server.

## Priority 9 — Validate and optimize PostgreSQL fallback search
**Why next:** The fallback path may still be significantly slower now that GIN search indexes were dropped, and it matters whenever Meilisearch is unavailable or still bootstrapping.

**Files**
- `apps/server/src/server_main/search/queries.rs`
- `apps/server/migrations/0010_drop_search_document_gin_indexes.sql`

**Problems observed**
- Fallback queries still use trigram and full-text operators with ranking/window functions.
- GIN indexes used for those operators were dropped.
- The fallback path is now rebuilt less often, but it still needs to be acceptable when used.

**Actions**
1. Run `EXPLAIN ANALYZE` on fallback search queries.
2. Decide whether fallback is acceptable as-is or needs dedicated indexing.
3. If fallback is rare, document degraded mode expectations.
4. If fallback must remain fast, rework query/index strategy.

**Expected impact**
- Better reliability when Meilisearch is unavailable or bootstrapping

**How to verify**
- Benchmark search latency with Meilisearch disabled.

---

## Priority 10 — Review sync pipeline write amplification
**Why after fallback search:** Important for scale, but likely lower ROI than validating the remaining degraded-mode search bottleneck first.

**Files**
- `apps/server/src/server_main/sync/persistence.rs`
- `apps/server/src/server_main/sync/scheduler.rs`
- `apps/server/src/server_main/app.rs`

**Problems observed**
- Programs are deleted and bulk reinserted on sync.
- Periodic sync scans all valid profiles.
- Shared DB pool is capped at 10 connections.

**Actions**
1. Measure pool contention during sync and normal traffic.
2. Consider increasing pool size if contention is real.
3. Evaluate incremental program updates instead of delete-and-reinsert.
4. Consider isolating background sync/index work from request-serving capacity.

**Expected impact**
- Better server stability under concurrent background work
- Less write amplification on large profiles

**How to verify**
- Compare sync times and DB pool pressure under load.

---

## Priority 11 — Minor/cleanup opportunities
**Files**
- `apps/server/src/server_main/guide.rs`

**Actions**
1. Batch favorite reorder writes instead of updating rows one by one if list size becomes meaningful.

**Expected impact**
- Smaller incremental gains and cleaner runtime behavior

---

## Suggested execution order
1. Validate/optimize PostgreSQL fallback search
2. Review sync write amplification and DB pool contention
3. Cleanup/minor optimizations

## Recommended measurement pass before the remaining server work
Collect:
- `EXPLAIN (ANALYZE, BUFFERS)` for:
  - search fallback queries
  - guide/category queries if touched during write-amplification work
- end-to-end timings for:
  - full sync
  - background metadata refresh
  - PostgreSQL fallback rebuild
  - Meilisearch refresh
  - `/search/channels` with Meilisearch disabled
  - `/search/programs` with Meilisearch disabled
- DB pool observations during concurrent sync + request traffic

## Success criteria
- Acceptable search latency when Meilisearch is unavailable
- Lower sync write amplification and DB churn
- Better stability under concurrent background work
- Smaller remaining cleanup wins without regressions
