---
run_id: ppv-timezone-explore-20260417
step_id: explore-ppv-timezone
role: explorer
attempt: 1
created_at: 2026-04-17T00:00:00Z
source_steps: []
repo_root: /home/oliver/src/Rust/Euripus
objective_digest: 99b2889b7f8c663f
---

## Goal
Determine where Euripus generates PPV channel titles and where event times/time zones are formatted, so a later implementation can switch PPV display times from the source time zone to the user’s local time zone.

## Relevant Files
- `README.md`
  - Confirmed repository/workspace layout from the accessible files.
  - Indicates:
    - `apps/client`: React web client/PWA
    - `apps/server`: Axum + PostgreSQL Rust API
    - `packages/shared`: shared TypeScript contracts

## Findings
- I could only inspect `README.md` within the provided read allowlist as enforced by the tool sandbox.
- The requested allowlist (`Cargo.toml`, `README.md`, `src`) does not match the repository layout described by `README.md`:
  - There is no accessible root `src/` tree.
  - A root `Cargo.toml` was not accessible/present at the expected path.
  - The actual code appears to live under workspace directories such as `apps/server`, `apps/client`, and `packages/shared`, but those paths were blocked by the current read restrictions.
- Because of that mismatch, I could not verify:
  - the exact file(s) where PPV channel titles are generated,
  - the exact function/type symbols involved,
  - the exact date/time formatting path for PPV event display,
  - whether timezone conversion currently happens on the server, in shared contracts, or in the client UI.
- Based on `README.md` alone, the most likely future inspection targets are:
  - `apps/server/...` for Rust-side event/title generation and EPG/source-time handling,
  - `packages/shared/...` for serialized event/time contract types,
  - `apps/client/...` for browser-side formatting into the user’s local timezone.

## Open Questions
- Can the read allowlist be expanded to include:
  - `apps/server`
  - `apps/client`
  - `packages/shared`
- Is PPV title generation expected to happen:
  - server-side during channel/event ingestion,
  - server-side during API response shaping,
  - or client-side during rendering?
- Are PPV event timestamps stored as:
  - UTC instants,
  - local wall-clock times plus source timezone,
  - or already-formatted strings?
- Is the desired behavior:
  - always render in the browser/device local timezone,
  - or render in a user-profile-configured timezone?

## Recommended Next Step
Re-run the exploration with a corrected read allowlist that matches the workspace layout, especially:
- `apps/server`
- `apps/client`
- `packages/shared`

With that access, the next pass should:
1. search for `PPV`, channel title builders, and event/title formatting code,
2. identify the concrete Rust/TypeScript symbols and file paths,
3. trace how event timestamps are stored and serialized,
4. determine whether the timezone fix belongs in API shaping, shared contracts, or client rendering.