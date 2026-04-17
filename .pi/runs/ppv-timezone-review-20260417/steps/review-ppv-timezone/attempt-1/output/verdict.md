---
run_id: ppv-timezone-review-20260417
step_id: review-ppv-timezone
role: reviewer
attempt: 1
created_at: 2026-04-17T12:00:00Z
source_steps: []
repo_root: /home/oliver/src/Rust/Euripus
objective_digest: 471e5bc793f921f1
---

## Verdict
Request changes. The new helper is wired into the major client surfaces, but there are two correctness issues that can produce wrong localized event times.

## Findings
- `formatEventChannelTitle` is used in all reviewed surfaces: guide, favorites, search, player, receiver, and recents/settings.
- The helper covers the two expected title shapes and has basic tests for explicit TZ conversion, weekday/month parsing, `referenceStartAt` fallback, and no-op behavior.
- However, the fallback/precedence logic and part of the timezone token handling are incorrect for real event-title data.

## Blocking Issues
1. Explicit title timezones are ignored whenever `referenceStartAt` is present
   - File: `apps/client/src/lib/utils.ts`
   - `formatEventChannelTitle()` currently does `referenceDate ?? inferredFromTitleTimezone`, so any passed `referenceStartAt` wins even if the title already includes `ET`/`CEST`/etc.
   - Affected call sites: `apps/client/src/features/channels/favorites-page.tsx`, `apps/client/src/features/channels/guide-page-sections.tsx`, `apps/client/src/features/search/search-page.tsx`.
   - Impact: if the current/attached program start does not exactly match the event timestamp encoded in the channel title, the rendered local time is wrong. This also contradicts the intended fallback behavior already implied by the tests: program start should only be used when the title omits an explicit source timezone.

2. Season-specific timezone tokens are collapsed into generic regional zones
   - File: `apps/client/src/lib/utils.ts`
   - `resolveSourceTimeZone()` maps `EST/EDT`, `CST/CDT`, `PST/PDT`, `CET/CEST`, `EET/EEST`, and `BST` to IANA regions instead of honoring the explicit offset semantics of the token itself.
   - Impact: titles can be off by one hour when the abbreviation and the region’s DST state disagree for that date. Example class of failure: `EST` on a summer date is resolved using New York daylight time instead of fixed UTC-5.

## Non-Blocking Issues
- `apps/client/src/lib/utils.test.ts` is missing focused coverage for:
  - explicit timezone titles when `referenceStartAt` is also provided
  - EST vs EDT / CET vs CEST-style distinctions
  - malformed dates/times remaining unchanged
- Follow-up check: `apps/client/src/features/auth/settings-page.tsx`, `apps/client/src/features/player/player-view.tsx`, `apps/client/src/features/receiver/receiver-page.tsx`, and channel-only search rows call the helper without `referenceStartAt`, so titles without an explicit source TZ will remain unconverted there. That may be acceptable if no start time is available, but it should be confirmed against the intended “across surfaces” behavior.

## Recommended Disposition
Request changes before merge. Fix precedence so explicit title timezones win over `referenceStartAt`, handle season-specific TZ tokens with fixed offsets (or equivalent explicit-offset logic), and add targeted utility tests for both cases.