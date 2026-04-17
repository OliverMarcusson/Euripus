---
run_id: ppv-favorites-plan-workflow-20260417b
step_id: explore
role: explorer
attempt: 1
created_at: 2026-04-17T00:00:00Z
source_steps: []
repo_root: /home/oliver/src/Rust/Euripus
objective_digest: fe2c9f0588093565
---

## Goal
Inspect the current implementation around favorites, navigation, server favorites endpoints, PPV/event metadata, and sync semantics to identify what already exists and what constraints matter for planning PPV event favorites.

## Relevant Files
- `apps/client/src/router.tsx`
- `apps/client/src/components/layout/app-shell.tsx`
- `apps/client/src/features/channels/favorites-page.tsx`
- `apps/client/src/features/channels/favorites-page.test.tsx`
- `apps/client/src/features/channels/guide-page.tsx`
- `apps/client/src/features/channels/guide-page-sections.tsx`
- `apps/client/src/features/channels/use-guide-page-state.ts`
- `apps/client/src/features/search/search-page.tsx`
- `apps/client/src/hooks/use-channel-favorite.ts`
- `apps/client/src/hooks/use-category-favorite.ts`
- `apps/client/src/lib/api.ts`
- `apps/client/src/lib/utils.ts`
- `apps/client/src/store/guide-navigation-store.ts`
- `packages/shared/src/index.ts`
- `apps/server/src/server_main/guide.rs`
- `apps/server/src/server_main.rs`
- `apps/server/src/server_main/search/queries.rs`
- `apps/server/src/server_main/search/indexing.rs`
- `apps/server/src/server_main/sync/persistence.rs`
- `apps/server/src/server_main/sync/runner.rs`
- `apps/server/migrations/0001_init.sql`
- `apps/server/migrations/0015_favorite_channel_categories.sql`
- `apps/server/migrations/0016_favorite_sort_order.sql`
- `apps/server/migrations/0019_admin_search_rules.sql`

## Findings
### Favorites data model today
- Shared client/server payloads only model:
  - favorite channels via `FavoriteChannelEntry = GuideChannelEntry & { kind: "channel"; order }`
  - favorite categories via `FavoriteCategoryEntry = { kind: "category"; category; order }`
  - unioned as `FavoriteEntry`
- There is no shared type for favorite programs, favorite events, or favorite PPV-specific entities in `packages/shared/src/index.ts`.
- Current `Channel` and `Program` shared types also do not expose PPV/event flags. They expose only generic guide/search fields.

### Client favorites UX and routing
- Favorites page is a top-level authenticated route at `/favorites` in `apps/client/src/router.tsx`.
- Favorites appears in shell navigation in `apps/client/src/components/layout/app-shell.tsx`.
- `FavoritesPage` loads `getFavorites()` with query key `["favorites"]` and renders:
  - category favorites first
  - channel favorites second
  - optional current/upcoming/past program metadata for channel favorites
  - reorder controls for categories and channels separately
- Favorites page tests confirm:
  - EPG metadata is shown when a favorite channel has a current program
  - rows stay minimal when no EPG exists
  - favorite categories render before favorite channels
- Category favorites on the favorites page use an `Open` action that:
  - stores `pendingOpenCategoryId` in Zustand via `requestOpenCategory`
  - navigates to `/guide`
- `useGuidePageState` consumes `pendingOpenCategoryId`, opens that category, and forces it visible even if guide preferences/filtering would otherwise hide it.
- There is no analogous navigation path for a specific favorite event/program.

### Favorite mutations and local cache sync
- Channel favorite mutation (`use-channel-favorite.ts`) toggles based on `channel.isFavorite` and calls:
  - `POST /favorites/:channelId`
  - `DELETE /favorites/:channelId`
- It performs optimistic updates for:
  - `["favorites"]`
  - `["guide","category"]`
  - `["search","channels"]`
  - `["recents"]`
- On success it invalidates only `["favorites"]`; guide/search/recents rely on optimistic mutation state unless an error occurs.
- When optimistically adding a channel favorite, the page inserts a synthetic favorite entry with:
  - `kind: "channel"`
  - `program: null`
  - next local `order`
- Category favorite mutation (`use-category-favorite.ts`) similarly optimistically updates:
  - `["guide","overview"]`
  - `["favorites"]`
- Reordering on `FavoritesPage` is optimistic against `["favorites"]` and persists via `PUT /favorites/order` with separate `categoryIds` and `channelIds`.

### Server favorites APIs
- Guide router exposes:
  - `GET /favorites`
  - `POST /favorites/{channel_id}`
  - `DELETE /favorites/{channel_id}`
  - `POST /favorites/categories/{category_id}`
  - `DELETE /favorites/categories/{category_id}`
  - `PUT /favorites/order`
- `GET /favorites` returns:
  - category favorites from `favorite_channel_categories`
  - channel favorites from `favorites`
  - each channel favorite may include one selected `program`
- Category favorites are ordered by `favorite_channel_categories.sort_order`.
- Channel favorites are ordered primarily by `favorites.sort_order`.
- `PUT /favorites/order` validates that submitted category/channel ID lists contain exactly the current favorite set, then rewrites sort order indexes.
- `POST /favorites/{channel_id}` inserts `(user_id, channel_id, sort_order)` with `ON CONFLICT DO NOTHING`.
- `POST /favorites/categories/{category_id}` inserts only if the category belongs to the same user.
- `POST /favorites/{channel_id}` does not perform an equivalent ownership existence check in the query itself; it inserts directly against `favorites(channel_id)` and depends on FK validity.
- There is no API for favorite programs/events, and no mixed ordering model across categories/channels/events beyond the current separate category/channel lists.

### How favorites payloads are populated
- Favorite channel rows return channel fields plus one chosen guide program using a lateral subquery.
- Program selection prefers:
  1. current live program
  2. next upcoming program
  3. most recent past program
- This means favorites already have one event-like program snapshot available, but only as derived metadata for a favorited channel, not as the favorited entity itself.
- `ChannelResponse` includes `is_favorite`; `ProgramResponse` does not.

### Search, PPV, and event metadata availability
- Search parsing supports `ppv`, `!ppv`, `vip`, `!vip`, `country:...`, `provider:...`, `epg` on both client and server.
- Server search metadata columns were added in `0019_admin_search_rules.sql`:
  - `search_country_code`
  - `search_provider_name`
  - `search_is_ppv`
  - `search_is_vip`
  on both `channels` and `programs`
- `refresh_search_metadata()` computes PPV/VIP/provider/country metadata and writes it back to DB after sync.
- Internal Meilisearch docs additionally derive/store:
  - `event_titles`
  - `event_keywords`
  - `is_event_channel`
  - `is_placeholder_channel`
  - `is_hidden`
- Event titles come from the top 3 ranked program titles per channel within the retained EPG window.
- Channel visibility hiding is based on server-side heuristics in `apps/server/src/server_main.rs`, using:
  - PPV branding in channel/category text
  - placeholder markers like `ENDED`, `NO EVENT STREAMING`, etc.
  - generic numbered PPV channel names with no meaningful event titles
  - past event dates embedded in names
- Search indexes use that metadata internally, but current client-facing shared payloads do not expose:
  - `isPpv`
  - `isEventChannel`
  - `eventTitles`
  - `eventKeywords`
  - `isHidden`
- Search result payloads returned to the client are still plain `Channel` / `Program` shapes.
- Search channel rows include `is_favorite`; search program rows do not.
- Therefore the app can search/filter for PPV/event-like content, but cannot currently render PPV/event metadata directly from shared API payloads except indirectly via title text and attached guide program data.

### Client display behavior for event-like channel names
- Client utility `formatEventChannelTitle()` rewrites embedded event timestamps in channel/program titles for display using either:
  - a supplied reference program start time
  - inferred timezone/date heuristics
- This is presentation-only. It does not create stable IDs or event entities.

### Sync behavior relevant to favorites
- Channel persistence is keyed by `(user_id, profile_id, remote_stream_id)` via unique index and `ON CONFLICT` upsert.
- On normal channel updates, existing channel rows keep their DB `id`; mutable fields update in place.
- Stale channels are deleted after sync if their `remote_stream_id` is no longer present.
- Favorites table references `channels(id)` with `ON DELETE CASCADE`, so deleting a stale channel also deletes its favorite row automatically.
- Programs are not updated in place:
  - full sync deletes all programs for the profile and reinserts
  - EPG sync also deletes all programs for the profile and reinserts
- That means program IDs are ephemeral across EPG/full syncs and are not suitable today for persistent favorites without an additional stable identity scheme.
- Search metadata/index refresh runs asynchronously after sync completion.
- Channel visibility cache is invalidated after sync.
- Search metadata/indexes are refreshed after sync, but favorites themselves have no special remapping step during sync.

### Practical implications for PPV event favorites
- Current durable favorite identity is channel ID only.
- Channels remain stable across updates as long as provider keeps the same `remote_stream_id`.
- Programs/events do not have stable persisted identity across sync.
- Server already derives event-like context from EPG titles and PPV heuristics, but that context is mostly internal to search/visibility and not exposed as a favoriteable API entity.
- Hidden PPV placeholder channels are excluded from guide/search visible-channel flows, so any PPV favorite design tied to hidden channels would need explicit handling.

## Open Questions
- What should the favorite target be for a PPV event:
  - channel row
  - program row
  - synthetic event entity derived from channel + program title + start/end
  - something keyed by provider `remote_stream_id` plus event timing/title?
- If the provider rotates PPV event channels by changing `remote_stream_id`, should favorites survive that transition?
- If a PPV event favorite is meant to survive EPG refreshes, what stable identifier should replace current program IDs?
- Should hidden/placeholder PPV channels be favoriteable if they later become real event channels, or should the system only allow favorites for currently visible event-capable channels?
- Should PPV event favorites appear on `/favorites` as a third kind, or should they project back into existing channel favorites UX?
- Does the planning target require direct client access to PPV/event metadata (`isPpv`, `eventTitles`, etc.), since current shared payloads do not expose it?
- Is cross-navigation needed from a favorite event into search results, guide category/channel, or direct playback?
- If an event favorite is modeled independently of channels, how should reorder semantics interact with the existing separate category/channel sort lists?

## Recommended Next Step
Define the intended durable identity for a PPV event favorite before API design. The main constraint is that channel IDs are relatively stable across sync, but program IDs are not. After that decision, map whether the feature should:
1. extend favorites as a new third entity kind with new shared/API types, or
2. treat PPV event favorites as enhanced channel favorites backed by exposed PPV/event metadata.