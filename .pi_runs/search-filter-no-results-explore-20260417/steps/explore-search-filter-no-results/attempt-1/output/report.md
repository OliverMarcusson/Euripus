---
run_id: search-filter-no-results-explore-20260417
step_id: explore-search-filter-no-results
role: explorer
attempt: 1
created_at: 2026-04-17T00:00:00Z
source_steps: []
repo_root: /home/oliver/src/Rust/Euripus
objective_digest: ce1bc30a18bedc38
---

## Goal
Investigate why filter-only search like `country:se` still returns no results after the PostgreSQL fallback hang fix, across client behavior, server Meilisearch/PostgreSQL search paths, and search metadata/indexing.

## Relevant Files
- `apps/client/src/features/search/search-page.tsx`
  - `SearchPage`
  - `parseSearchGuideQuery`
  - `getSearchAutocompleteState`
  - `getSearchAutocompleteSuggestions`
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
  - `get_search_filter_options`
  - `build_search_filter_options`
- `apps/server/src/server_main/search/queries.rs`
  - `search_channels_postgres`
  - `search_programs_postgres`
- `apps/server/src/server_main/search/lexicon.rs`
  - `parse_search_query`
  - `build_meili_search_filter`
- `apps/server/src/server_main/search/indexing.rs`
  - `refresh_search_metadata`
  - `rebuild_postgres_search_documents`
  - `rebuild_search_documents`
  - `build_meili_channel_doc`
  - `build_meili_program_doc`
  - `rebuild_meili_indexes`
  - `refresh_meili_channels_delta`
- `apps/server/src/server_main/search/rules.rs`
  - `load_pattern_groups`
  - `evaluate_patterns`
- `apps/server/src/server_main/sync/runner.rs`
  - `spawn_search_refresh`
- `apps/server/src/server_main/admin.rs`
  - `spawn_admin_reindex`
  - `test_search_query`
  - `test_patterns`
- `apps/server/migrations/0019_admin_search_rules.sql`
- `apps/server/migrations/0020_admin_provider_country_relations.sql`

## Findings
1. Client-side query handling does allow filter-only searches.
   - `apps/client/src/features/search/search-page.tsx`
   - `hasQuery` is `debouncedQuery.trim().length > 1`, so `country:se` is considered a valid query.
   - `apps/client/src/lib/api.ts` sends the raw query as `q=country:se` to both `/search/channels` and `/search/programs`.
   - No obvious client-side suppression explains zero results.

2. Server parsing explicitly supports filter-only queries.
   - `apps/server/src/server_main/search/lexicon.rs::parse_search_query`
   - `country:se` becomes:
     - `search = ""`
     - `countries = ["se"]`
   - `apps/server/src/server_main/search/mod.rs::parse_search_pagination` permits short/empty free text when structured filters exist.

3. PostgreSQL fallback now has dedicated filter-only paths, but they depend entirely on populated metadata columns.
   - `apps/server/src/server_main/search/queries.rs::search_channels_postgres`
     - filter-only branch uses `channels.search_country_code`, `search_provider_name`, `search_is_ppv`, `search_is_vip`
   - `apps/server/src/server_main/search/queries.rs::search_programs_postgres`
     - filter-only branch uses `coalesce(programs.search_country_code, channels.search_country_code)` and equivalent provider/flag fields
   - If `search_country_code` is `NULL` on rows, `country:se` will return nothing even though the new fallback query itself is correct.

4. Meilisearch filter-only search also depends on the same metadata being populated first.
   - `apps/server/src/server_main/search/mod.rs::search_channels_meili`
     - empty free-text path calls Meili with `query=""` and filter built from parsed filters
   - `apps/server/src/server_main/search/mod.rs::search_programs_meili`
     - uses `with_query(&parsed.search)`; for `country:se`, that is `""`
   - `apps/server/src/server_main/search/lexicon.rs::build_meili_search_filter`
     - adds `country_code = "se"`
   - `apps/server/src/server_main/search/indexing.rs::build_meili_channel_doc` and `build_meili_program_doc`
     - Meili docs get `country_code` from `search_country_code`
   - So Meili and PostgreSQL both fail the same way if metadata was never backfilled/refreshed.

5. The likely primary mismatch is: filter options come from admin rules, not from actual indexed/populated row metadata.
   - `apps/server/src/server_main/search/mod.rs::get_search_filter_options`
   - `build_search_filter_options` uses `rules::load_pattern_groups`, not `channels/programs` metadata.
   - Result: the UI can autocomplete/show `country:se` because a country rule exists, while searches still return zero because no channel/program rows currently have `search_country_code='se'`.

6. There is no migration-time backfill for the new metadata columns.
   - `apps/server/migrations/0019_admin_search_rules.sql`
     - adds `search_country_code`, `search_provider_name`, `search_is_ppv`, `search_is_vip`
     - adds indexes
     - creates admin rule tables
   - No migration populates those new columns for existing rows.
   - This makes existing installs especially likely to show empty filter-only results until a later sync/admin-triggered rebuild runs.

7. Metadata population only happens in background refresh/reindex flows.
   - `apps/server/src/server_main/search/indexing.rs::refresh_search_metadata`
     - evaluates admin rules and writes `search_country_code`, `search_provider_name`, flags onto `channels` and `programs`
   - Called from:
     - `apps/server/src/server_main/sync/runner.rs::spawn_search_refresh`
     - `apps/server/src/server_main/search/indexing.rs::rebuild_search_documents`
     - `apps/server/src/server_main/admin.rs::spawn_admin_reindex`
   - If the environment was upgraded but no sync/admin reindex has happened since, metadata likely remains null.

8. Admin rules must actually match channel/category/program text for country metadata to be assigned.
   - `apps/server/src/server_main/search/rules.rs::evaluate_patterns`
   - Country/provider values are not inferred automatically for the DB filter path; they are derived from admin pattern groups.
   - If country groups exist but their patterns do not match the real channel/category strings (e.g. expecting `SE:` / `SE|` while feed uses another convention), metadata remains null and `country:se` still returns nothing.
   - `apps/server/src/server_main/admin.rs::test_patterns` and `test_search_query` exist specifically to verify this.

9. Secondary issue: Meili program total count is likely wrong after post-fetch visibility filtering, but that does not explain “no results” by itself.
   - `apps/server/src/server_main/search/mod.rs::search_programs_meili`
   - It fetches Meili hits, loads DB rows, filters invisible channel IDs, then sets `total_count = items.len()`.
   - That can undercount/paginate incorrectly, but it is not the main cause of empty `country:se` results in both backends.

## Open Questions
1. Have existing users run a provider sync or admin rule save/import since `0019_admin_search_rules.sql` landed?
   - If not, metadata columns are probably still null.

2. Do the current admin country rules actually match the provider’s channel/category naming format?
   - Example risk: rules may define `SE:` / `SE|`, but real data may use other prefixes/labels.

3. Are rows in `channels`/`programs` currently populated with:
   - `search_country_code`
   - `search_provider_name`
   - `search_is_ppv`
   - `search_is_vip`
   ?
   The code strongly suggests this is the deciding runtime check.

4. Is Meilisearch currently ready for the affected user, or is the request already on PostgreSQL fallback?
   - Either way metadata is required, but it affects which runtime path to inspect first.

## Recommended Next Step
1. First verify runtime state, not client code.
   - Check whether affected rows actually have populated `search_country_code` values for the user.
   - Check whether current admin country rules match real channel/category strings.
   - Use the existing admin endpoints/symbols for quick verification:
     - `apps/server/src/server_main/admin.rs::test_search_query`
     - `apps/server/src/server_main/admin.rs::test_patterns`

2. Minimal runtime remediation if metadata is stale:
   - Trigger a full search rebuild path that runs `refresh_search_metadata`:
     - provider sync, or
     - admin rule save/import/delete to invoke `spawn_admin_reindex`
   - This is the most likely immediate fix for already-deployed data.

3. Minimal code fix after confirmation:
   - Add an automatic one-time/server-startup backfill or explicit admin task for `refresh_search_metadata` + search rebuild for all users after `0019`/`0020`, so filter-only search does not depend on a later sync/admin edit.
   - Optionally make `/search/filter-options` derive from populated metadata (or intersect rules with live metadata), so the UI does not advertise filters that currently match zero indexed rows.