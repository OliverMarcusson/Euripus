---
run_id: ppv-timezone-explore2-20260417
step_id: explore-ppv-timezone
role: explorer
attempt: 1
created_at: 2026-04-17T00:00:00Z
source_steps: []
repo_root: /home/oliver/src/Rust/Euripus
objective_digest: d0f415b1e637fa65
---

## Goal
Trace where PPV/event channel titles originate, how time values flow from server to client, and where timezone handling already exists so the displayed PPV title time can be changed to the user’s local timezone.

## Relevant Files
- `packages/shared/src/index.ts`
  - `Channel`
  - `Program`
  - `PlaybackSource`
- `apps/server/src/xtreme.rs`
  - `XtreamChannel`
  - `fetch_live_streams`
- `apps/server/src/server_main/sync/persistence.rs`
  - `bulk_upsert_channels`
  - `bulk_insert_programmes`
  - `resolve_epg_programmes`
- `apps/server/src/xmltv.rs`
  - `XmltvProgramme`
  - `parse_xmltv_timestamp`
  - `finalize_programme`
- `apps/server/src/server_main/guide.rs`
  - `GuideCategoryEntryRow`
  - `map_guide_category_entry`
  - `map_guide_program_response`
  - `get_guide_category`
  - `get_channel_guide`
- `apps/server/src/server_main/search/indexing.rs`
  - `build_channel_search_document_row`
  - `build_meili_channel_doc`
  - `detect_event_channel`
- `apps/server/src/server_main/search/queries.rs`
  - `search_channels_postgres`
  - `search_programs_postgres`
- `apps/server/src/server_main/playback/mod.rs`
  - `resolve_channel_playback_source_for_target`
  - `resolve_program_playback_source_for_target`
  - `finalize_playback_source`
- `apps/server/src/server_main/playback/resolve.rs`
  - `PlaybackSourceResponse`
  - `playback_source_from_url`
  - `unsupported_playback`
- `apps/server/src/server_main.rs`
  - `classify_channel_visibility_at`
  - `extract_channel_event_date`
- `apps/client/src/lib/utils.ts`
  - `formatDateTime`
  - `formatTime`
  - `formatTimeRange`
- `apps/client/src/lib/api.ts`
  - `getGuide`
  - `getGuideCategory`
  - `getChannelGuide`
  - `searchChannels`
  - `searchPrograms`
  - `startChannelPlayback`
  - `startProgramPlayback`
- `apps/client/src/features/channels/guide-page-sections.tsx`
  - `GuideCategorySection`
- `apps/client/src/features/channels/favorites-page.tsx`
  - `FavoriteProgramDetails`
- `apps/client/src/features/search/search-page.tsx`
  - `ProgramSearchRow`
  - `ChannelSearchRow`
- `apps/client/src/features/player/player-view.tsx`
  - `PlayerView`
- `apps/client/src/hooks/use-playback-actions.ts`
  - `useChannelPlaybackMutation`
  - `useProgramPlaybackMutation`

## Findings
- PPV/event channel “titles” are not generated from EPG times; the main channel label comes directly from provider channel names.
  - `apps/server/src/xtreme.rs` `fetch_live_streams` maps upstream `XtreamChannelPayload.name` into `XtreamChannel.name` unchanged.
  - `apps/server/src/server_main/sync/persistence.rs` `bulk_upsert_channels` writes that value directly into `channels.name`.
  - Client channel UIs then render `channel.name` directly, e.g. `apps/client/src/features/channels/guide-page-sections.tsx` and `apps/client/src/features/player/player-view.tsx`.

- The server already treats PPV/event channel names as opaque text, with only parsing/classification heuristics.
  - `apps/server/src/server_main.rs` `classify_channel_visibility_at` calls `extract_channel_event_date(channel_name, year)` to hide stale PPV channels.
  - `extract_channel_event_date` parses month/day tokens embedded in channel names like `PSG vs Liverpool @ Apr 9 20:55 : ...`, but it does not rewrite the title.
  - Tests in `apps/server/src/server_main.rs` confirm this expected input shape.

- Search indexing also uses raw channel names as titles.
  - `apps/server/src/server_main/search/indexing.rs` `build_channel_search_document_row` sets `title: row.name.clone()`.
  - `build_meili_channel_doc` sets `channel_name: row.name.clone()`.
  - `detect_event_channel` explicitly flags names containing `@`, `vs`, or sports phrases, reinforcing that PPV/event timing is embedded in the stored channel name string.

- Program times from XMLTV are normalized to UTC on ingest.
  - `apps/server/src/xmltv.rs` `parse_xmltv_timestamp` parses XMLTV timestamps with offsets and returns `parsed.with_timezone(&Utc)`.
  - `finalize_programme` stores `start_at`/`end_at` as UTC-backed `DateTime<Utc>`.
  - `apps/server/src/server_main/sync/persistence.rs` `bulk_insert_programmes` persists those UTC timestamps to `programs.start_at` / `programs.end_at`.

- Server guide/search APIs expose program times as structured timestamps, not preformatted strings.
  - `apps/server/src/server_main/guide.rs` `map_guide_program_response` returns `ProgramResponse { start_at, end_at, ... }`.
  - `apps/server/src/server_main/search/queries.rs` maps DB rows into `ProgramResponse` with `start_at` and `end_at`.
  - Shared contract in `packages/shared/src/index.ts` exposes these as `Program.startAt` / `Program.endAt` strings.

- Client-side formatted program times already use the browser’s local timezone.
  - `apps/client/src/lib/utils.ts` `formatTime` and `formatTimeRange` use `new Intl.DateTimeFormat(undefined, ...)` with no explicit `timeZone`, so formatting uses the user agent’s local timezone.
  - These helpers are used in:
    - `apps/client/src/features/channels/guide-page-sections.tsx`
    - `apps/client/src/features/channels/favorites-page.tsx`
    - `apps/client/src/features/search/search-page.tsx`
  - Therefore normal EPG time badges are already local-time on the client.

- The playback title shown in the player is server-provided raw text, not locally reformatted.
  - Shared type: `packages/shared/src/index.ts` `PlaybackSource.title`.
  - Server:
    - `apps/server/src/server_main/playback/mod.rs` `resolve_channel_playback_source_for_target` passes `&record.name` into `finalize_playback_source`.
    - `resolve_program_playback_source_for_target` passes:
      - live program: `&row.channel_name`
      - catch-up program: `&row.title`
    - `apps/server/src/server_main/playback/resolve.rs` `playback_source_from_url` / `unsupported_playback` copy `title` verbatim into `PlaybackSourceResponse.title`.
  - Client:
    - `apps/client/src/hooks/use-playback-actions.ts` stores the returned `PlaybackSource`.
    - `apps/client/src/features/player/player-view.tsx` renders `source.title` directly.

- Current end-to-end data flow for PPV channel title text is:
  - provider response name
  - `apps/server/src/xtreme.rs` `XtreamChannel.name`
  - `apps/server/src/server_main/sync/persistence.rs` `channels.name`
  - server API/search/playback responses (`ChannelResponse.name`, `MeiliChannelDoc.channel_name`, `PlaybackSourceResponse.title`)
  - client rendering of `channel.name` or `source.title`

- Current end-to-end data flow for structured event/program times is:
  - XMLTV timestamp with offset
  - `apps/server/src/xmltv.rs` `parse_xmltv_timestamp` -> UTC
  - DB `programs.start_at` / `end_at`
  - server `ProgramResponse.start_at` / `end_at`
  - shared `Program.startAt` / `endAt`
  - client `formatTimeRange` -> local browser timezone

- Implication for the requested change:
  - Changing `formatTimeRange` will not affect PPV titles like `PSG vs Liverpool @ Apr 9 20:55 : TeliaPlay SE 26`, because that time is embedded in `channel.name` / `source.title`, not in `Program.startAt`.
  - The change point for “displayed PPV title time” must be wherever raw `channel.name` / `source.title` is rendered, or earlier when those values are produced.

## Open Questions
- Should the PPV title timezone conversion happen only in the client web UI, or also in server-returned `PlaybackSource.title` and any remote/receiver surfaces?
- What exact title patterns must be supported beyond the observed forms:
  - `... @ Apr 9 20:55 : ...`
  - `... Wed 08 Apr 15:00 CEST ...`
- For titles without an explicit timezone abbreviation/offset, what source timezone should be assumed:
  - provider/country-derived timezone,
  - server locale,
  - or no conversion at all?
- Is the desired scope only channel labels (`Channel.name`) and player labels (`PlaybackSource.title`), or also search result labels and favorites rows?
- The server has country/provider metadata but no user timezone field in shared contracts or request params. If server-side conversion is desired, where should user timezone come from?

## Recommended Next Step
Implement the timezone-adjusted PPV title display as a client-side presentation transform first, not as a server storage change.

Concrete likely touchpoints:
- Add a new client formatter in `apps/client/src/lib/utils.ts` for PPV/event title strings.
- Apply it where raw channel/player titles are rendered:
  - `apps/client/src/features/channels/guide-page-sections.tsx`
  - `apps/client/src/features/channels/favorites-page.tsx` if channel names need rewriting there
  - `apps/client/src/features/search/search-page.tsx` for channel rows if needed
  - `apps/client/src/features/player/player-view.tsx` for `source.title`
- Leave `Program.startAt` / `endAt` formatting alone unless UI requirements change, since those are already local-time via `Intl.DateTimeFormat(undefined, ...)`.