---
run_id: search-filter-hang-explore-20260417
step_id: explore-search-filter-hang
role: explorer
attempt: 1
created_at: 2026-04-17T00:00:00Z
source_steps: []
repo_root: /home/oliver/src/Rust/Euripus
objective_digest: ce56176fa20e851a
---

## Goal
Investigate why a filter-only search such as `country:se` can leave the search UI appearing to load forever, by checking the client search page, its tests, and the server search parsing/handling. Identify the most likely root cause and the smallest safe fix.

## Relevant Files
- `apps/client/src/features/search/search-page.tsx`
  - `SearchPage`
  - `getSearchAutocompleteState`
  - `buildSearchGuideState`
  - `parseSearchGuideQuery`
- `apps/client/src/features/search/search-page.test.tsx`
- `apps/client/src/lib/api.ts`
  - `searchChannels`
  - `searchPrograms`
  - `getSearchFilterOptions`
- `apps/server/src/server_main/search/mod.rs`
  - `search_channels`
  - `search_programs`
  - `parse_search_pagination`
  - `search_channels_meili`
  - `search_programs_meili`
- `apps/server/src/server_main/search/queries.rs`
  - `search_channels_postgres`
  - `search_programs_postgres`
- `apps/server/src/server_main/search/lexicon.rs`
  - `ParsedSearch`
  - `parse_search_query`
  - `build_meili_search_filter`
- `apps/server/src/server_main/search/indexing.rs`
  - `configure_meili_indexes`

## Findings
1. Client does allow filter-only queries to execute.
   - In `apps/client/src/features/search/search-page.tsx`, `hasQuery` is `debouncedQuery.trim().length > 1`.
   - `country:se` therefore enables both infinite queries:
     - `searchChannels(debouncedQuery, pageParam, SEARCH_PAGE_SIZE)`
     - `searchPrograms(debouncedQuery, pageParam, SEARCH_PAGE_SIZE)`
   - The client is not blocking filter-only input.

2. The client “loading” state waits on both channel and program searches.
   - `isInitialLoading` is true when either:
     - `channelQuery.isPending && !channels.length`, or
     - `programQuery.isPending && !programs.length`
   - So if either backend request stalls, the page stays in the loading skeleton state and `SearchResults` never renders.
   - That matches the reported UX: the UI looks like it is loading indefinitely, even if the problem is server-side.

3. Server parsing explicitly supports filter-only queries.
   - In `apps/server/src/server_main/search/lexicon.rs`, `parse_search_query("country:se")` is tested and produces:
     - `parsed.search == ""`
     - `parsed.countries == ["se"]`
   - In `apps/server/src/server_main/search/mod.rs`, `parse_search_pagination` allows short/empty free text when structured filters exist:
     - `has_structured_filters` includes countries/providers/ppv/vip/epg.
     - The 2-character minimum is only enforced when there are no structured filters.
   - So parsing/validation is not the bug.

4. Meilisearch path is intentionally prepared for filter-only searches.
   - `build_meili_search_filter` in `apps/server/src/server_main/search/lexicon.rs` builds a filter using `country_code`, `provider_name`, `is_ppv`, `is_vip`, `has_epg`, `is_hidden`, and `user_id`.
   - `apps/server/src/server_main/search/indexing.rs` configures those fields as filterable on both `channels` and `programs`.
   - `search_channels_meili` in `apps/server/src/server_main/search/mod.rs` has an explicit `if parsed.search.is_empty()` branch for filter-only searches and runs a filtered empty-query search.
   - `search_programs_meili` also issues Meili search with `with_query(&parsed.search)`, which can be empty.

5. The PostgreSQL fallback does not have a filter-only fast path and is the most likely cause of the hang.
   - In `apps/server/src/server_main/search/queries.rs`, both fallback queries always go through `search_documents` and ranking logic, even when `parsed.search.trim()` is empty.
   - For channels:
     - `search_channels_postgres` uses `COUNT(*) OVER ()` and `ROW_NUMBER() OVER (ORDER BY ... similarity(sd.search_text, $2) DESC ...)`.
     - When `$2 == ''`, the search predicate becomes effectively “match everything that passes filters”:
       - `($2 = '' OR sd.tsv @@ plainto_tsquery('simple', $2) OR sd.search_text % $2)`
     - That means a filter-only query can force a full ranked scan of all matching `search_documents` rows for the user, then join back to `channels`.
   - For programs:
     - `search_programs_postgres` does the same against `search_documents`/`programs`, also with window functions and ranking across all filtered rows.
   - This is much more expensive than a normal text search and much more likely to stall badly on large datasets.

6. The asymmetry between Meili and Postgres strongly suggests the fallback path was missed.
   - `search_channels_meili` has a dedicated empty-search branch.
   - `search_channels_postgres` and `search_programs_postgres` do not.
   - That inconsistency is the clearest code-level sign of an incomplete implementation for filter-only searches.

7. Existing tests cover parsing and client autocomplete, but not server execution of filter-only searches.
   - `apps/server/src/server_main/search/lexicon.rs` tests prove parsing support.
   - `apps/client/src/features/search/search-page.test.tsx` includes autocomplete that produces `"country:se "`, but only with mocked API calls.
   - There is no test in the inspected files covering actual server behavior for a filter-only request through Postgres fallback or Meili execution.

8. Secondary server bugs exist, but they do not best explain the “indefinite loading” symptom.
   - In `apps/server/src/server_main/search/mod.rs`, `search_programs_meili` sets `total_count = items.len() as i64` after pagination/filtering, rather than using Meili’s total hits.
   - The same file’s filter-only branch in `search_channels_meili` computes `total_count` from returned hit ids, not estimated total hits.
   - These can cause incorrect pagination/next page behavior, but they do not by themselves explain an initial request that never seems to finish.

## Open Questions
- Does the reported hang reproduce only when Meilisearch is unavailable/not ready for that user, or also when Meili is active?
  - The code points most strongly to the PostgreSQL fallback path.
- Is the “indefinite” behavior actually a very slow server response, or a request that never completes due to database plan/pathology in production data?
  - The current code is enough to make filter-only fallback queries much heavier than normal, but logs/trace data would confirm whether it is slowness vs a true stuck request.
- Are there DB indexes that make the fallback filter-only path tolerable in some environments?
  - Nothing in the inspected search files shows a dedicated optimized path for empty free text.

## Recommended Next Step
Implement the smallest safe server-side fix in the PostgreSQL fallback path:

1. Add an explicit `search_term.is_empty()` branch in:
   - `apps/server/src/server_main/search/queries.rs` / `search_channels_postgres`
   - `apps/server/src/server_main/search/queries.rs` / `search_programs_postgres`

2. In that branch, avoid `search_documents`, `plainto_tsquery`, `%`, `similarity`, and windowed ranking over the full text corpus.
   - Query directly from `channels` / `programs` using only structured filters (`country`, `provider`, `ppv`, `vip`, `epg`, visibility), with a simple deterministic order and normal `LIMIT/OFFSET` handling.

3. Add tests for filter-only execution, not just parsing:
   - server-side tests covering `country:se` and `provider:viaplay` on fallback paths
   - optionally a client test that asserts mocked filter-only queries leave the skeleton state once both responses resolve

This is the smallest safe fix because it targets the apparent slow/hanging path without changing client semantics or the already-working query parser.