---
run_id: ppv-favorites-plan-workflow-20260417b
step_id: plan
role: planner
attempt: 1
created_at: 2026-04-17T00:00:00Z
source_steps: ["explore"]
repo_root: /home/oliver/src/Rust/Euripus
objective_digest: 8fb2393545235324
---

## Goal
Add a separate PPV favorites feature so temporary PPV/event channels can be saved and revisited without being added to the existing long-term favorites list.

Recommended product/technical decision for v1:
- Model PPV favorites as a second, independent channel-favorites list.
- Do not model PPV favorites as program/event favorites in v1.
- Keep the existing `/favorites` page and data contract for normal favorites unchanged.
- Add a new dedicated PPV favorites route/page with its own storage, APIs, ordering, query key, and toggle state.

This is the safest fit for the current codebase because:
- channel identity is the only durable favorite target currently available,
- programs are deleted and reinserted on sync, so program IDs are not stable enough for persistent favorites,
- the existing favorites page already knows how to render a channel plus one selected current/upcoming/past program snapshot, which can be reused for PPV rows.

## Constraints
- The plan must stay within the explored facts from:
  - `apps/client/src`
  - `apps/server/src`
  - `apps/server/migrations`
  - `packages/shared/src`
- Existing favorites behavior must remain intact:
  - `/favorites` stays the normal favorites page,
  - category favorites remain on the normal favorites page only,
  - existing category/channel ordering semantics should not be mixed with PPV ordering.
- Current sync semantics matter:
  - channels are relatively stable because they upsert by `(user_id, profile_id, remote_stream_id)`,
  - stale channels are deleted after sync,
  - favorites tied to deleted channels are removed via `ON DELETE CASCADE`,
  - programs are fully deleted and reinserted on EPG/full sync, so `program.id` is ephemeral.
- Current client payloads do not expose PPV/event metadata, so minimal channel metadata must be surfaced before the client can render a separate PPV-favorite control or page confidently.
- Search metadata/index refresh is asynchronous after sync, so PPV classification may lag behind channel ingestion briefly.
- Hidden/placeholder PPV channels are already treated specially by server visibility heuristics; v1 should not depend on hidden-channel favoriting flows.

## Implementation Plan
1. **Adopt a channel-based PPV favorite model**
   - Store PPV favorites as a separate channel list, not as a new favorite-program/event entity.
   - Rationale:
     - `program.id` is unstable because programs are recreated on every EPG/full sync.
     - Channel favorites already survive ordinary channel updates when the underlying `remote_stream_id` remains stable.
     - Reusing channel-based rows allows the PPV page to show the same selected program snapshot logic already used by normal favorites.
   - Explicitly accept this v1 limitation:
     - if a provider rotates an event onto a brand-new channel / new `remote_stream_id`, the PPV favorite will not automatically remap.

2. **Add a dedicated PPV favorites table and migration**
   - Create a new migration in `apps/server/migrations` for a table such as `favorite_ppv_channels` with:
     - `user_id`
     - `channel_id`
     - `sort_order`
     - unique constraint on `(user_id, channel_id)`
     - foreign key to `channels(id)` with `ON DELETE CASCADE`
   - Keep this table fully separate from:
     - `favorites`
     - `favorite_channel_categories`
   - Do not add category support to PPV favorites.
   - Do not add a shared mixed-order table across normal favorites and PPV favorites.

3. **Extend shared channel-facing types with PPV state**
   - Update `packages/shared/src/index.ts` so channel-bearing payloads can expose:
     - `isPpv`
     - `isPpvFavorite`
   - Keep existing normal-favorite types intact.
   - Recommended contract shape:
     - normal `/favorites` response remains as-is,
     - PPV favorites response is a dedicated response using channel-favorite rows only, reusing the current favorite-channel row shape where possible.
   - Do **not** add program-favorite shared types in v1; document in code/comments that this is intentionally deferred because program IDs are not durable.

4. **Add server endpoints for PPV favorites**
   - In `apps/server/src/server_main/guide.rs`, add a parallel PPV-favorites API surface:
     - `GET /favorites/ppv`
     - `POST /favorites/ppv/{channel_id}`
     - `DELETE /favorites/ppv/{channel_id}`
     - `PUT /favorites/ppv/order`
   - Behavior:
     - `GET /favorites/ppv` returns ordered PPV favorite channels only.
     - Each row should include the same “selected program” snapshot strategy already used by normal channel favorites:
       1. current live program
       2. next upcoming program
       3. most recent past program
     - `PUT /favorites/ppv/order` validates that the submitted channel ID list exactly matches the current PPV favorite set before rewriting `sort_order`.
   - Validation:
     - validate that the channel belongs to the requesting user/profile before mutation,
     - do not rely solely on foreign-key existence.
   - Keep existing `/favorites` endpoints untouched except for any shared response-field additions (`isPpv`, `isPpvFavorite`).

5. **Populate PPV flags in channel responses**
   - Update server response-building so channel-bearing payloads used by guide/search/favorites can include:
     - `isPpv` from existing stored/computed PPV metadata,
     - `isPpvFavorite` via a join against the new PPV favorites table.
   - Apply this consistently to the channel responses used by:
     - guide views,
     - search channel results,
     - favorites page rows.
   - Do not block PPV-favorite storage on program metadata.
   - Do not introduce program-level favorite flags in v1.

6. **Add client API methods and query keys**
   - In `apps/client/src/lib/api.ts`, add methods for:
     - fetching PPV favorites,
     - adding/removing a PPV favorite,
     - updating PPV favorite order.
   - Introduce a dedicated React Query key, e.g.:
     - `["favorites", "ppv"]`
   - Keep the existing normal favorites query key unchanged:
     - `["favorites"]`

7. **Add a dedicated PPV favorite mutation hook**
   - Add a new hook alongside `apps/client/src/hooks/use-channel-favorite.ts`, e.g. `apps/client/src/hooks/use-ppv-favorite.ts`.
   - It should mirror the optimistic-update pattern of normal favorites, but for the new state:
     - toggle `isPpvFavorite` on channel-bearing cached data,
     - optimistically insert/remove rows in `["favorites", "ppv"]`,
     - use a synthetic optimistic row with `program: null` on add, matching the existing normal-favorites pattern.
   - Update relevant caches optimistically where channel state appears today:
     - `["guide", "category"]`
     - `["search", "channels"]`
     - `["recents"]`
     - `["favorites", "ppv"]`
   - On success:
     - invalidate `["favorites", "ppv"]`
   - On error:
     - roll back all optimistic changes.
   - Do **not** update `["favorites"]` when PPV favorite state changes.

8. **Add a separate PPV favorites route and page**
   - In `apps/client/src/router.tsx`, add a dedicated authenticated route such as:
     - `/favorites/ppv`
   - In `apps/client/src/components/layout/app-shell.tsx`, add a distinct navigation entry:
     - “PPV Favorites”
   - Create a new page component in `apps/client/src/features/channels/` that:
     - loads `["favorites", "ppv"]`,
     - renders PPV favorite channels only,
     - shows the same current/upcoming/past program metadata used by normal favorite channels,
     - supports reorder controls for channels only,
     - shows a clear empty state explaining that this page is for temporary PPV/event channels.
   - Keep `apps/client/src/features/channels/favorites-page.tsx` focused on:
     - category favorites
     - normal channel favorites
   - Add a small cross-link or explanatory copy between the two pages so users understand why PPV favorites are separate.

9. **Expose and use the PPV toggle in existing channel surfaces**
   - Update channel-list UIs that currently rely on channel favorite state so they can also show a PPV-favorite action when `channel.isPpv` is true.
   - Minimum target surfaces from the explored area:
     - guide-related channel views under `apps/client/src/features/channels/`
     - search channel results in `apps/client/src/features/search/search-page.tsx`
   - Keep normal favorite and PPV favorite as separate actions/state.
   - Do not require direct favoriting from program search results in v1, since search program rows do not currently carry channel-level favorite state.

10. **Keep normal favorites semantics isolated**
    - Do not add PPV entries as a third `FavoriteEntry` kind inside the existing `/favorites` response.
    - Do not change the normal favorites ordering API (`PUT /favorites/order`) to handle PPV items.
    - Do not add categories to the PPV page.
    - This avoids mixed-order complexity and preserves existing tests and UX assumptions.

11. **Test coverage**
    - Server tests:
      - `GET /favorites/ppv` returns ordered channel rows with selected program metadata.
      - `POST`/`DELETE /favorites/ppv/{channel_id}` mutate only PPV favorites.
      - `PUT /favorites/ppv/order` rejects lists that do not exactly match the current PPV favorite set.
      - channel payloads include `isPpv` and `isPpvFavorite` where expected.
      - deletion of stale channels removes PPV favorites via cascade behavior.
    - Client tests:
      - router and shell navigation expose the PPV favorites route.
      - PPV favorites page renders:
        - empty state,
        - channel rows,
        - selected program metadata when available,
        - minimal rows when no EPG exists.
      - optimistic PPV toggle updates the PPV page/query cache and channel flags without mutating the normal favorites page/query.
      - PPV reorder persists and re-renders correctly.
      - existing `favorites-page` tests continue to pass unchanged, confirming isolation.
    - Shared-contract checks:
      - client/server type updates compile cleanly with the new `isPpv` and `isPpvFavorite` fields.

12. **Document the deferred event-identity problem**
    - Add implementation notes/comments where appropriate stating:
      - PPV favorites are intentionally channel-based in v1,
      - program IDs cannot be used for durable favorites because sync deletes and recreates all programs,
      - a future event-level favorite feature would require a new stable synthetic identity scheme, not raw `program.id`.

## Acceptance Criteria
- A new authenticated PPV favorites page exists at a dedicated route and is reachable from shell navigation.
- Users can add/remove PPV favorites without adding/removing the same channel from normal favorites.
- Normal favorites (`/favorites`) continue to show only category favorites and normal channel favorites.
- PPV favorites have their own storage and ordering; normal favorites ordering remains unchanged.
- PPV favorites render channel rows with the same selected current/upcoming/past program snapshot behavior already used for normal favorite channels.
- Channel-bearing client payloads expose enough metadata to render both:
  - whether a channel is PPV,
  - whether it is in the PPV favorites list.
- Optimistic cache updates correctly keep guide/search/recents/channel lists in sync for PPV favorite state, and failures roll back cleanly.
- No persistence path relies on `program.id` for PPV favorites, and the implementation clearly documents why:
  - program rows are deleted and reinserted on sync, so program IDs are unstable.
- Relevant server and client tests are added/updated, and existing normal-favorites tests still pass.

## Risks
- **Provider channel rotation risk:** if a PPV event moves to a new channel / new `remote_stream_id`, channel-based PPV favorites will not automatically follow it.
- **PPV classification lag:** if `isPpv` depends on asynchronously refreshed metadata, the PPV toggle may briefly appear/disappear inconsistently after sync.
- **Hidden/placeholder channel edge cases:** server-side visibility heuristics may prevent some PPV-like channels from being discoverable/favoriteable through normal client flows.
- **UI complexity risk:** showing both normal favorite and PPV favorite controls on the same channel surfaces could confuse users unless labels/copy are explicit.
- **Optimistic cache drift:** multiple channel caches (`guide`, `search`, `recents`, PPV favorites page) increase the chance of inconsistent state if any update path is missed.
- **Scope creep risk:** attempting to solve true event-level favorites now would require a new durable event identity scheme and would substantially expand the change beyond the requested separate PPV favorites page.