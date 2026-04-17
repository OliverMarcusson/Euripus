# Sports page v1 plan

## Goal
Implement a new authenticated `/sports` page in Euripus that integrates with the external Sports API described in `../Sports-API/docs/api-and-euripus.md`.

v1 should include:
- overview page at `/sports`
- event details in v1
- watch guidance display only, no exact playback resolution yet
- competition filters/chips in v1

---

## Product scope

### Main page: `/sports`
The page should present sports events in a way that feels native to Euripus while making the Sports API’s ranking and provider guidance immediately useful.

Primary sections:
- **Live now**
- **Today**
- **Coming up**

Primary controls:
- refresh action
- competition filter chips
- possibly a time-window selector for upcoming later, but not required for the first pass

### Event detail in v1
Each event should open an event detail view, likely as a drawer or modal first, unless route-based detail feels cleaner during implementation.

The detail view should show:
- title
- competition
- sport
- start/end time
- venue
- round label
- participants
- recommended provider
- full list of watch availabilities
- search metadata / source metadata where useful

This is an **information and watch-guidance surface**, not a playback surface.

---

## Architecture plan

## 1. Add a Sports API adapter to the Euripus Rust server
Do not call the external Sports API directly from the browser client.

Instead, add a server-side adapter in Euripus that:
- reads Sports API base URL from config
- calls the upstream Sports API via `reqwest`
- normalizes the upstream payload into Euripus-friendly camelCase response models
- exposes same-origin `/api/sports/*` endpoints to the React app

### Proposed config
Add a new server environment variable:

```env
APP_SPORTS_API_BASE_URL=http://127.0.0.1:3000
```

### Behavior when not configured
If `APP_SPORTS_API_BASE_URL` is missing, the backend should return a clear structured error and the frontend should show a clean empty/error state such as:
- “Sports is not configured on this Euripus server.”

---

## 2. Add Euripus backend routes
Create a new server module, likely:

```text
apps/server/src/server_main/sports.rs
```

### Initial Euripus routes
These should proxy the upstream Sports API endpoints needed for v1:

- `GET /api/sports/live`
- `GET /api/sports/today`
- `GET /api/sports/upcoming?hours=72`
- `GET /api/sports/events/{id}`
- `GET /api/sports/competitions/{slug}`
- `GET /api/sports/providers`

### Why include more than the overview endpoints now
Even if the overview page starts with only live/today/upcoming, event detail and competition filters become much easier if the proxy surface already exists.

### Backend responsibilities
The new module should:
- define upstream deserialization structs
- define normalized API response structs
- map snake_case upstream fields to camelCase Euripus fields
- handle upstream non-200 responses gracefully
- return appropriate Euripus API errors

### Suggested error behavior
- missing config -> `503 Service Unavailable`
- upstream unavailable / timeout -> `502 Bad Gateway`
- malformed upstream payload -> `502 Bad Gateway`
- unknown event / competition -> pass through as `404` where reasonable

---

## 3. Normalize shared contracts
Add new shared TypeScript contracts in:

```text
packages/shared/src/index.ts
```

### Proposed types
- `SportsEvent`
- `SportsParticipantSet`
- `SportsWatch`
- `SportsAvailability`
- `SportsSearchMetadata`
- `SportsEventListResponse`
- `SportsCompetitionResponse`
- `SportsProviderCatalogResponse`

### Important normalized fields
Use camelCase on the client side:
- `startTime`
- `endTime`
- `roundLabel`
- `sourceUrl`
- `recommendedMarket`
- `recommendedProvider`
- `channelName`
- `watchType`
- `searchHints`

This keeps sports responses aligned with the rest of the frontend contracts.

---

## 4. Client API helpers
Extend:

```text
apps/client/src/lib/api.ts
```

### Add helpers
- `getSportsLiveEvents()`
- `getSportsTodayEvents()`
- `getSportsUpcomingEvents(hours?: number)`
- `getSportsEvent(id: string)`
- `getSportsCompetition(slug: string)`
- `getSportsProviders()`

These should hit the Euripus backend, not the external Sports API directly.

---

## 5. Routing and navigation

### Router
Add a new authenticated route in:

```text
apps/client/src/router.tsx
```

Route:
- `/sports`

### Navigation
Add a Sports item to the app shell navigation in:

```text
apps/client/src/components/layout/app-shell.tsx
```

Recommended placement:
- after `Guide`
- before `Search`

Recommended icon:
- `Trophy`
- or `Activity`

`Trophy` is probably the clearest.

---

## 6. Frontend page structure
Create a dedicated sports feature area:

```text
apps/client/src/features/sports/
  sports-page.tsx
  use-sports-page-state.ts
  sports-section.tsx
  sports-event-card.tsx
  sports-event-detail.tsx
```

Optional helper files if needed:

```text
  sports-formatting.ts
  sports-types.ts
```

### State hook
`use-sports-page-state.ts` should:
- load live/today/upcoming in parallel
- derive combined event counts
- derive available competition chips from fetched events
- manage active competition filter
- expose refresh action
- manage selected event for detail view

### Query strategy
Use independent parallel React Query queries for:
- live
- today
- upcoming

This avoids request waterfalls and keeps section-level loading and error states isolated.

### Suggested stale times
Based on the upstream integration guidance:
- live: 1 minute
- today: 5 minutes
- upcoming: 15 minutes
- event detail: 5 minutes
- providers: several hours

---

## 7. Page UX and visual direction

### Design direction
Follow Euripus’ existing dark, cinematic aesthetic, but make sports feel more like a live control room than a generic admin dashboard.

Goals:
- immediate scanability
- strong hierarchy for live events
- provider guidance visible without opening detail
- premium, broadcast-inspired feel

### Page layout

#### Header
Show:
- title: `Sports`
- short description
- refresh button
- meta badges with counts

Example meta:
- live count
- today count
- upcoming count
- active filter count or current filter label

#### Competition filters/chips
Add a horizontal chip row near the top.

Behavior:
- default chip: `All`
- chips derived from loaded event data and/or provider catalog if useful
- selecting a chip filters all sections at once
- chips should work across live/today/upcoming together

Important note:
The overview endpoints do not provide a standalone competition list, so the simplest v1 is to derive chips from returned events.

Potential enhancement:
- if a chip is selected and we want richer data later, we can back it with `/api/sports/competitions/{slug}`
- for v1, client-side filtering is likely enough and simpler

---

## 8. Event card design
Each event card should surface the most useful guidance immediately.

### Card contents
- title
- competition
- formatted start time or live state
- recommended provider
- top channel name if present
- optional market badge
- optional venue / round label
- optional participants layout when available

### Status treatment
- **Live** should have the strongest visual treatment
- **Upcoming** should emphasize scheduled time
- **Info-only** events should still look complete, just less urgent

### Suggested subtitle format
Examples:
- `TV4 Play · TV4 Fotboll`
- `Viaplay · V Sport Football`
- `PGA Tour · TV4 Play`

### Card actions
For v1:
- `View details`

No playback CTA yet.
No “search in Euripus” CTA for v1, per current scope.

---

## 9. Event detail UX
Event detail is included in v1.

### Recommendation
Implement detail as a **dialog or drawer** first.

Why:
- lower routing complexity
- keeps the user in the overview flow
- aligns well with browsing sports schedules quickly

A dedicated route can be added later if needed.

### Detail contents
#### Primary info
- title
- competition
- sport
- start time and end time
- venue
- round label
- participants
- status

#### Watch guidance block
Show prominently:
- recommended provider
- recommended market
- sorted availabilities list

Each availability row can show:
- provider label
- provider family if useful internally
- channel name
- watch type
- market
- confidence/source if present in response
- search hints as secondary metadata if useful

#### Metadata block
Secondary / collapsible if the UI gets too dense:
- source
- source URL
- search metadata queries
- search metadata keywords

### Detail loading strategy
The initial card list should come from the overview endpoints.
On card click:
- either use the already available event payload if sufficient
- or fetch `/api/sports/events/{id}` for the fuller detail object

I recommend fetching detail on demand if the API’s detail payload is richer.

---

## 10. Filtering behavior

### v1 competition filtering
Implement competition chips as a client-side filter over the events returned by:
- live
- today
- upcoming

This gives:
- simple UX
- fast interactions
- minimal backend complexity for first release

### Expected behavior
When a chip is active:
- all three sections are filtered by competition slug
- counts update to reflect filtered results
- empty states become filter-aware

Example empty state:
- `No live events for Premier League right now`

### Future enhancement path
Later we can deepen filter behavior by:
- loading dedicated competition pages from `/api/sports/competitions/{slug}`
- adding sport-level filters
- adding market/provider filters

---

## 11. Empty, loading, and error states
Design these explicitly.

### Loading
- skeleton cards for each section
- section-level loading states rather than blocking the entire page forever

### Empty states
Need distinct states for:
- no sports configuration
- no live events
- no today events
- no upcoming events
- no events for selected competition filter

### Error states
Need graceful handling for:
- Sports API unreachable
- one section failing while others succeed
- event detail fetch failing

The page should be resilient and avoid all-or-nothing failure.

---

## 12. Backend implementation notes

### Config changes
Update:

```text
apps/server/src/config.rs
apps/server/.env.example
```

Add optional config field:
- `sports_api_base_url: Option<Url>` or `Option<String>` parsed to `Url`

### App state
No new persistent storage is required for v1.
The server can call the Sports API directly per request.

Potential addition:
- a dedicated reqwest client for sports requests
- or reuse an existing client if appropriate

### Module wiring
Update server router wiring to merge the new `sports::shared_router()`.

---

## 13. Frontend implementation notes

### Recommended UI primitives
Use existing Euripus primitives where possible:
- `PageHeader`
- `Badge`
- `Button`
- `Card`
- `Separator`
- `Skeleton`
- existing dialog/sheet primitives if available

### Performance notes
Follow React best practices:
- fetch independent sections in parallel
- keep section state isolated
- avoid memoization unless it solves a real issue
- extract card/detail components instead of defining them inline
- use `content-visibility` or compact rendering patterns if lists get long

### Accessibility notes
- chips must be keyboard accessible
- live status should not rely on color only
- dialog/detail must have proper focus management
- provider/watch metadata should have clear labeling

---

## 14. Testing plan

## Backend tests
Add tests for:
- sports config missing
- successful upstream mapping
- upstream error mapping
- event detail endpoint behavior
- competition endpoint behavior

## Frontend tests
Add tests for:
- route renders
- nav item appears
- live/today/upcoming sections render independently
- competition chips filter correctly
- event detail opens and shows watch guidance
- configured/unconfigured error state
- partial failure behavior

---

## 15. Implementation order

### Phase 1: backend contract and proxy
1. Add server config for Sports API base URL
2. Add Rust sports module and proxy endpoints
3. Add shared TS sports types
4. Add frontend API helpers

### Phase 2: sports overview page
5. Add `/sports` route
6. Add navigation item
7. Build sports page state hook
8. Build header, sections, skeletons, empty/error states
9. Build competition chips
10. Build event cards

### Phase 3: event detail
11. Build event detail dialog/drawer
12. Add on-demand detail fetch if needed
13. Render watch availability list and metadata blocks

### Phase 4: tests and polish
14. Add backend tests
15. Add frontend tests
16. Refine spacing, responsive behavior, and accessibility

---

## 16. Final v1 definition
A v1-complete sports implementation means:
- authenticated `/sports` route exists
- Euripus backend proxies Sports API endpoints
- page shows live/today/upcoming sections
- competition chips filter the overview
- event cards surface provider/watch guidance
- event detail is available in v1
- no playback resolution or search CTA yet
- UI is resilient to empty/loading/error states

---

## Recommendation summary
Recommended v1 shape:
- route: `/sports`
- detail: dialog/drawer in v1
- filter model: competition chips derived from returned events
- integration model: Rust backend proxy, not direct browser calls
- action model: watch guidance only

This gives Euripus a strong first sports surface without prematurely solving exact playback mapping.
