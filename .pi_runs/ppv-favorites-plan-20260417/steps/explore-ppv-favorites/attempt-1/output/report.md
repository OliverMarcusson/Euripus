---
run_id: ppv-favorites-plan-20260417
step_id: explore-ppv-favorites
role: explorer
attempt: 1
created_at: 2026-04-17T00:00:00Z
source_steps: []
repo_root: /home/oliver/src/Rust/Euripus
objective_digest: 07652b0f5a4ae85a
---

## Goal
Inspect the existing favorites flow, route/navigation wiring, server favorites APIs, and available PPV/event metadata to determine what must change to support a separate PPV favorites page for time-limited PPV events without mixing them into regular channel favorites.

## Relevant Files
- `packages/shared/src/index.ts`
  - Shared API contracts for `Channel`, `Program`, `FavoriteEntry`, `FavoriteOrderPayload`, search payloads.
- `apps/client/src/router.tsx`
  - Authenticated route tree; currently only one `/favorites` route.
- `apps/client/src/components/layout/app-shell.tsx`
  - Main desktop/mobile navigation; currently one Favorites nav item.
- `apps/client/src/lib/api.ts`
  - Client API wrappers for `/favorites`, category favorites, and reorder endpoint.
- `apps/client/src/features/channels/favorites-page.tsx`
  - Current unified favorites UI for category favorites + channel favorites.
- `apps/client/src/hooks/use-channel-favorite.ts`
  - Optimistic channel favorite toggle behavior and cache invalidation.
- `apps/client/src/hooks/use-category-favorite.ts`
  - Optimistic category favorite toggle behavior and cache invalidation.
- `apps/client/src/features/channels/guide-page-sections.tsx`
  - Guide channel rows expose favorite toggles and use event-title formatting.
- `apps/client/src/features/search/search-page.tsx`
  - Search UI supports `ppv` / `!ppv` filters, but channel result objects do not expose PPV metadata.
- `apps/client/src/lib/utils.ts`
  - `formatEventChannelTitle()` rewrites event-like channel names for display.
- `apps/client/src/features/channels/favorites-page.test.tsx`
  - Current favorites page coverage.
- `apps/client/src/router.test.tsx`
  - Existing router redirect coverage.
- `apps/server/src/server_main/guide.rs`
  - Favorites routes, favorites list queries, add/remove/reorder handlers, guide/category DTO mapping.
- `apps/server/src/server_main.rs`
  - Core `ChannelResponse` / `ProgramResponse`; PPV/event visibility heuristics via `classify_channel_visibility*`.
- `apps/server/src/server_main/search/mod.rs`
  - Search endpoints, Meilisearch/Postgres selection, visibility filtering.
- `apps/server/src/server_main/search/queries.rs`
  - Postgres channel/program search queries; PPV filtering uses persisted search metadata.
- `apps/server/src/server_main/search/indexing.rs`
  - Search metadata derivation, event-title extraction, Meili docs with `is_ppv`, `event_titles`, `is_event_channel`.
- `apps/server/migrations/0001_init.sql`
  - Base `favorites` table.
- `apps/server/migrations/0015_favorite_channel_categories.sql`
  - Category favorites table.
- `apps/server/migrations/0016_favorite_sort_order.sql`
  - Sort order columns and indexes for both favorite tables.
- `apps/server/migrations/0019_admin_search_rules.sql`
  - `search_country_code`, `search_provider_name`, `search_is_ppv`, `search_is_vip` on `channels` and `programs`.

## Findings
### Backend / API / data model
- Favorites are currently stored in only two persistence buckets:
  - `favorites(user_id, channel_id, created_at, sort_order)` in `apps/server/migrations/0001_init.sql` + `0016_favorite_sort_order.sql`
  - `favorite_channel_categories(user_id, category_id, created_at, sort_order)` in `apps/server/migrations/0015_favorite_channel_categories.sql` + `0016_favorite_sort_order.sql`
- There is no server-side concept of a separate PPV favorite, event favorite, or favorite type discriminator.
- There is also no table for favoriting a `program`/event. The only favorite entity types today are:
  - channel
  - channel category
- Shared client/server DTOs reflect that limitation:
  - `Channel` has no `isPpv`, `isEventChannel`, or event metadata fields.
  - `FavoriteEntry` is only `FavoriteCategoryEntry | FavoriteChannelEntry`.
  - `FavoriteOrderPayload` only contains `categoryIds` and `channelIds`.
- Server favorites endpoints are all in `apps/server/src/server_main/guide.rs`:
  - `GET /favorites`
  - `POST /favorites/{channel_id}`
  - `DELETE /favorites/{channel_id}`
  - `POST /favorites/categories/{category_id}`
  - `DELETE /favorites/categories/{category_id}`
  - `PUT /favorites/order`
- `GET /favorites` currently returns a single mixed payload:
  - category favorites first
  - then channel favorites
  - categories and channels each have their own independent `sort_order`
- `PUT /favorites/order` validates and rewrites the full set of favorite category IDs and the full set of favorite channel IDs. This is important:
  - it requires the request to include every favorite channel exactly once
  - it requires the request to include every favorite category exactly once
  - therefore a filtered UI view cannot safely reuse this endpoint for reordering only a PPV subset unless the server also changes its validation model
- `POST /favorites/{channel_id}` simply inserts into `favorites` with `ON CONFLICT DO NOTHING`.
  - It does not validate visibility, PPV status, event status, or expiry.
  - It does not prevent favoriting a hidden PPV placeholder/expired channel if the caller knows the ID.
- PPV classification exists today, but only as search/visibility metadata, not as favorite-type data:
  - persisted columns: `channels.search_is_ppv`, `programs.search_is_ppv`
  - transient/search-doc metadata: `is_ppv`, `event_titles`, `event_keywords`, `is_event_channel`, `is_hidden`
- Existing event metadata is derived mainly from `programs.title` and channel/category text:
  - `load_channel_event_titles()` collects up to 3 ranked program titles per channel
  - `detect_event_channel()` and `derive_event_keywords()` infer event-ness
  - `build_meili_channel_doc()` stores `event_titles`, `event_keywords`, `is_event_channel`
- That event metadata is not exposed through the normal guide/favorites/channel API responses.
- Guide/search/listing visibility already hides some PPV channels:
  - `classify_channel_visibility*()` in `apps/server/src/server_main.rs` marks placeholder/generic/past-date PPV channels hidden
  - guide/search/channels endpoints use `load_channel_visibility_map()` and filter to visible IDs
- `GET /favorites` does not apply that visibility filtering.
  - Result: a channel favorite can remain visible in Favorites even if the same channel would now be hidden in guide/search due to PPV placeholder/past-event heuristics.
  - This is a concrete behavioral gap relevant to “time-limited PPV events”.

### Frontend / routing / UI
- Routing currently exposes a single favorites page at `/favorites` in `apps/client/src/router.tsx`.
- Navigation in `apps/client/src/components/layout/app-shell.tsx` has a single Favorites nav item for both desktop and mobile.
- The current page in `apps/client/src/features/channels/favorites-page.tsx` assumes one unified saved-items view:
  - header title: `Favorites`
  - one query key: `["favorites"]`
  - fetches `getFavorites()`
  - splits returned entries into category favorites and channel favorites
  - renders all categories before all channels
  - reorder mutation persists full category/channel order back through `reorderFavorites()`
- Channel favorite optimistic updates in `use-channel-favorite.ts` also assume a single favorites collection:
  - query key `["favorites"]`
  - adding a favorite appends a `kind: "channel"` entry with `program: null`
  - removing a favorite removes it from the same list
  - also updates guide/search/recents `channel.isFavorite`
- Category favorite optimistic updates in `use-category-favorite.ts` assume the same shared `["favorites"]` cache and append/remove `kind: "category"` entries there.
- `Channel.isFavorite` is a single boolean used across guide, search, settings recents, and favorites.
  - There is no way in the current client model to represent:
    - regular favorite vs PPV favorite
    - channel favorite vs event-only favorite
    - favorited because of current PPV event but not as a regular channel
- Search UI already exposes PPV filters to users:
  - `ppv` / `!ppv` parsing exists in `apps/client/src/features/search/search-page.tsx`
  - server search supports those filters in both Postgres and Meili paths
- But search results still return plain `Channel` objects with no `isPpv` field, so the client cannot branch UI or favorite behavior on server-classified PPV status without either:
  - adding new fields to shared `Channel`, or
  - re-deriving PPV heuristics client-side from names/categories.
- Display support for PPV/event-ish channels exists:
  - `formatEventChannelTitle()` in `apps/client/src/lib/utils.ts` rewrites embedded event timestamps in channel names for local display
  - guide/search/favorites already use that helper when rendering channel names
- There is no dedicated PPV page component or route today.

### Test impact
- Client test coverage found for favorites is limited to:
  - rendering program metadata
  - rendering empty/non-EPG state
  - category rows appearing before channel rows
- Router tests currently cover redirect behavior only; no explicit assertions around favorites route registration or nav items.
- I did not find server endpoint tests specifically covering favorites API behavior or favorite reorder edge cases.
- Existing server unit tests already cover PPV visibility heuristics in `apps/server/src/server_main.rs`:
  - hidden placeholder PPV
  - hidden generic numbered PPV
  - visible event-specific PPV
  - visible LIVE/NEXT PPV
  - hidden PPV with past month/day marker
- Existing search/indexing tests already cover PPV/event metadata derivation, but that metadata is search-oriented, not favorites-oriented.

### Concrete constraints that matter for implementation
- If the product requirement is “separate PPV favorites page without mixing into regular channel favorites,” the current persistence model is insufficient by itself.
  - A single `favorites` table keyed only by `channel_id` cannot distinguish:
    - a normal favorite channel
    - a PPV favorite channel
    - the same channel being present in both buckets
- If the requirement is specifically “time-limited PPV events,” the current model is even further away:
  - there is no durable favorite target for an event/program identity beyond `program.id`
  - event metadata for channels is derived from near-term EPG titles and can change over time
  - current favorite records do not store event start/end or event title snapshot
- Reusing the current `/favorites/order` contract for a PPV-only page is risky because the server validates against the entire favorite-channel set.
- Reusing current `Channel.isFavorite` is also insufficient if regular favorites and PPV favorites must coexist independently.
- Existing visibility logic may need to be reused for PPV favorites, otherwise expired/placeholder PPV favorites can leak into a new page or regular favorites page.
- Because shared types omit PPV metadata on `Channel`, any frontend-only split would currently rely on brittle client heuristics instead of server truth.

## Open Questions
- Should a PPV favorite be stored against:
  - the channel ID,
  - the program/event ID,
  - or a new composite/snapshotted PPV event record?
- Must the same underlying PPV channel be allowed in both:
  - regular favorites, and
  - PPV favorites
  independently?
- Should expired PPV favorites automatically disappear, archive, or remain visible with ended metadata?
- Should PPV favorites exclude category favorites entirely, or is there also a notion of PPV category/group favorites?
- Is the desired page scoped to:
  - only currently active/upcoming PPV event channels,
  - all PPV-classified channels,
  - or only channels with meaningful event titles?
- Should guide/search favorite buttons behave differently for PPV channels:
  - route to PPV favorites instead of regular favorites,
  - offer two favorite actions,
  - or infer destination automatically?
- Does the product want server APIs split as separate endpoints, or a single favorites endpoint with filtering/type parameters?
- If PPV favorites are event-based, what should happen when the same channel carries a new event later:
  - update existing favorite,
  - create a new favorite entry,
  - or expire the old one?

## Recommended Next Step
Decide the storage model first, then shape APIs around it. The current codebase strongly suggests that a clean implementation needs a backend data-model change rather than only a frontend page split.

Most likely planning path:
1. Choose favorite identity:
   - preferred question to resolve: channel-based PPV bucket vs event/program-based PPV bucket.
2. Add explicit backend representation for PPV favorites instead of overloading `favorites`.
3. Expose server-classified PPV/event metadata in shared response types needed by the UI.
4. Add dedicated API/query keys/routes for PPV favorites, rather than filtering the existing mixed `/favorites` response.
5. Then update client routing/navigation and optimistic cache logic to keep regular favorites and PPV favorites independent.
6. Add tests for:
   - new route/nav entry
   - PPV favorites page rendering
   - toggle behavior for PPV items
   - API list/add/remove/reorder semantics
   - expiry/visibility handling for time-limited events.