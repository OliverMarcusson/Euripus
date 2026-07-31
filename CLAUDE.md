# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

Bun workspaces monorepo (`apps/*`, `packages/*`). Run everything from the repo root.

```bash
bun install
bun run dev:start          # Postgres + API in Docker, Vite dev server on 0.0.0.0; waits for both
bun run dev:stop
bun run dev:reset          # wipes Postgres, then restarts

bun run check:client       # tsc -b
bun run test:client        # vitest run
bun run test:server        # cargo test
bun run build:client       # tsc -b && vite build
bun run build:shared
```

Single tests:

```bash
bun --cwd apps/client test src/lib/hls.test.ts
bun --cwd apps/client test src/lib/hls.test.ts -t "restarts loading on non-fatal network errors"
cargo test --manifest-path apps/server/Cargo.toml transcode        # substring filter
```

Deployment (`bun run publish` builds and pushes GHCR images; `prod:start` pulls them on the host) is documented in `README.md` and `docs/self-hosting.md`. `bun run build:apk` builds the Android TV receiver.

The Vite dev server proxies `/api` and `/health` to `127.0.0.1:8080`, so the client uses the same same-origin API shape in dev and prod. For Android TV or Cast testing, use the LAN URL `dev:start` prints, not `127.0.0.1`.

## Server architecture (`apps/server`)

Axum + PostgreSQL (sqlx), Rust edition 2024.

**`server_main.rs` is the shared core, and submodules glob-import it.** It holds one large `use` block plus cross-cutting helpers — `require_auth`, `require_receiver_auth`, `request_base_url`, `hash_password`, `encrypt_secret`, `generate_refresh_token`, channel-visibility classification. Every submodule under `server_main/` starts with `use super::*;` and inherits both the third-party imports and those helpers. When adding a module, follow that convention rather than re-importing; when adding a helper used by more than one module, it belongs in `server_main.rs`.

**Routing.** Each module exposes `browser_router()` and/or `shared_router()`. `app.rs` composes them into `browser_api_router()`, which is nested under `/api`. A module may split its handlers into a private child module that exposes no router of its own. `guide/favorites.rs` holds the favorites and PPV-favorites handlers, and `receiver/remote.rs` holds the `/remote/*` pairing and control handlers; in both cases the parent keeps the single `browser_router()`/`shared_router()` and mounts them as `favorites::list_favorites`, `remote::pair_receiver` and so on. Only the handlers the parent router names are `pub(super)` — query helpers stay private to the child. The child can read the parent's private items without any widening, so shared helpers belong in the parent. In `receiver.rs` that means `receiver_sender`, `is_receiver_online` and `require_paired_receiver_device` stay put; the last one is device-side despite the name. There is no separate top-level app for other clients — the Android TV and Cast receivers hit the same `/api` surface.

**Three separate identities**, each with its own guard:
- User — `require_auth`, Bearer JWT access token plus a refresh cookie, with a `euripus.csrf` cookie for mutations.
- Admin — `require_admin` in `admin.rs`, separate `euripus.admin.csrf` cookie.
- Receiver — `require_receiver_auth`, a receiver session token (also accepted as a query param for SSE), backed by a long-lived `receiver_credential` the device stores locally.

The client mirrors this exactly in `lib/api.ts` with three request helpers (`request`, `adminRequest`, `receiverRequest`). A receiver is not a logged-in user; it authenticates only as a device.

**Migrations are immutable.** Startup runs `sqlx::migrate!` and validates checksums. Never edit an applied migration in `apps/server/migrations`; add a new numbered one.

**`AppState`** (`state.rs`) carries two distinct reqwest clients — `provider_http_client` for sync/EPG and `relay_http_client` with different timeouts for streaming — plus several `DashMap` caches, `receiver_channels` (per-device broadcast senders for SSE), and the single-slot `cast_transcodes` mutex. Writes that must not interleave per user go through `state.user_database_lock(user_id)`.

## Playback and relay

This is the concept that spans the most files (`server_main/playback/`, `relay.rs`, `transcode.rs`).

Provider streams are either played direct from the client or proxied through the server. `playback_source_for_mode` decides: a signed relay token is issued and the client receives an `/api/relay/hls`, `/api/relay/raw`, or `/api/relay/asset` URL instead of provider credentials.

`PlaybackTarget` (`Browser`, `Cast`, `ReceiverWeb`, `ReceiverAndroidTv`) drives both the stream format and whether relaying is mandatory. Cast and Android TV **always** relay regardless of the provider's playback mode; Browser relays only when HTTPS would otherwise be mixed-content. `resolve.rs` maps target → `PlaybackStreamFormat` → `RelayAssetKind`, and the resulting `kind` (`hls`, `mpegts`, `progressive`, `unsupported`) tells the client which player path to take. Note that on-demand titles are resolved as `Progressive` even for Cast, so they arrive as `/api/relay/raw` and bypass hls.js entirely.

## Receiver and remote control

A receiver (browser tab, Cast device, or Android TV) registers via `POST /api/receiver/session`, gets a pairing code, and holds an SSE stream at `/api/receiver/events`. A signed-in sender pairs with the code, selects the device as its remote target, and issues `/api/remote/*` commands. Commands are fanned out over the per-device broadcast channel; the receiver acknowledges each one (`executing` → `succeeded`/`failed`) and separately reports playback state, which is what the sender's status banner reads.

Google Cast has extra startup constraints — the receiver framework is started by `apps/client/public/cast-receiver-bootstrap.js` *before* the app bundle, because senders time out waiting for `start()`. See `docs/GOOGLE_CAST_RECEIVER.md` before touching that path.

`transcode.rs` is an opt-in NVENC fallback (`APP_CAST_TRANSCODING_ENABLED`) used only after a Cast receiver reports a codec failure. One active transcode server-wide.

## Client architecture (`apps/client`)

React 19, TanStack Router, TanStack Query, Zustand, Tailwind v4, shadcn-style components in `components/ui`.

Routes are **defined in code** in `src/router.tsx`, not file-based. Most routes sit under an `authenticated` route guarded by `RequireAuth`; `/receiver` deliberately sits outside it, since a Cast device has no user session.

Playback goes `PlyrSurface` → `bindPlaybackSource` (`lib/plyr-player.ts`) → `createIptvHls` (`lib/hls.ts`) for HLS, or a plain `video.src` otherwise. Failures surface as a `PlaybackFailure` with a `reason` that callers switch on — the Cast transcode fallback keys off `reason === "codec"`, so classification changes there have real behavioural consequences.

The production build is a **single unsplit bundle** (~1.5 MB). That is why anything the Cast receiver needs early must live outside it.

`packages/shared` exports `src/index.ts` directly, so type changes are picked up without a build step in dev.

## Conventions

Commit messages are imperative and capitalised, no conventional-commit prefix (`Add on-demand playback history and controls`).

Tests live beside their subject, in a separate file, on both sides. The client pairs `hls.ts` with `hls.test.ts`. The server keeps the test module in the tree where it has always been, but sources it from a sibling file:

```rust
#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
```

`relay_tests.rs` then holds what used to sit inside `mod tests { ... }`. The module path is unchanged (`server_main::relay::tests::*`), so `use super::*` still reaches private items and test names are identical — the only difference is that reading `relay.rs` no longer pulls the tests in with it. That matters because the heaviest files were close to half tests by line count.

Name the file `<stem>_tests.rs` (`mod.rs` → `mod_tests.rs`). This is uniform — all 19 test modules in `apps/server` follow it, and there are no inline `mod tests { ... }` blocks left. New ones should match.

The `#[path]` is resolved relative to the directory holding the declaring file, so the test file is always a direct sibling of its subject.

Note that `apps/server` is not currently rustfmt-clean (`transcode.rs` has a pre-existing diff), so run `rustfmt --edition 2024` on specific files rather than `cargo fmt` across the crate, which would sweep in unrelated reformatting.
