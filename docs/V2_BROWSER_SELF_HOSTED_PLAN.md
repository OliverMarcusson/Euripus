# Euripus V2 Browser And Self-Hosted Access Plan

This document captures the intended direction for a future Euripus v2 where the product can be accessed fully through a browser and self-hosted on a server.

## Product Goal

Allow users to access Euripus through a regular web browser against a self-hosted instance, while preserving the same account model, favorites, provider sync, search, and playback contracts introduced in v1.

## Core Architectural Direction

Keep the current Rust API as the central application backend and introduce a first-class web client that speaks the same contracts now used by the Tauri desktop app.

### What Stays Reusable From V1

- Username/password auth and session model
- Provider profile storage and encryption
- Xtreme Codes validation and sync pipeline
- PostgreSQL catalog and search indexes
- Favorites, recents, and playback source contracts
- Shared domain types in `packages/shared`

### What Changes In V2

- Add a browser-first frontend deployment target alongside the desktop app
- Replace Tauri keyring storage with browser-safe session handling for the web client
- Move from desktop-centric layout assumptions to responsive web navigation
- Introduce deployment topology for a public or LAN-accessible self-hosted instance

## Browser Client Plan

The safest direction is to keep the existing React app portable and add a browser mode rather than forking the product logic.

- Continue using React and the shared TypeScript contracts.
- Keep all auth, search, favorites, guide, and provider flows API-driven.
- Replace Tauri-only storage calls with an environment-aware token adapter.
- Support browser playback with HLS.js where the provider stream is browser-compatible.
- Surface unsupported stream formats explicitly, just as v1 does on desktop.

## Authentication Plan For Browser Access

The web client should not rely on the desktop refresh-token keyring model.

- Move refresh tokens to secure HTTP-only cookies for the browser client.
- Keep short-lived access tokens for API authorization if still useful for the frontend state model.
- Add CSRF protection for cookie-authenticated browser actions.
- Preserve session listing and revocation so users can manage desktop and browser sessions together.

## Self-Hosting Topology

Recommended v2 deployment shape:

1. Reverse proxy with TLS
2. Euripus Rust API
3. PostgreSQL
4. Static browser frontend build

Prefer hosting the static web frontend behind the same domain as the API to simplify auth, cookies, and CORS.

## Streaming Considerations

Playback remains the biggest constraint for browser access.

- Keep direct-to-provider playback as the default model.
- Continue returning typed playback source metadata from the server.
- For providers that do not expose browser-compatible streams, consider an optional media relay/transcoding component later, but do not make it part of the initial browser v2 scope.
- Catch-up playback should continue to use provider-generated archive URLs where available.

## Security Changes Needed For V2

- Replace permissive CORS with an explicit allow-list.
- Add trusted proxy configuration if deployed behind a reverse proxy.
- Add rate limiting for auth and provider endpoints.
- Use secure cookies and same-site policy for browser refresh flows.
- Document secrets management for self-hosted operators clearly.

## UX Changes Needed For V2

- Convert the desktop three-panel layout into a responsive web shell.
- Add a narrower mobile/tablet-friendly layout for administration and browsing.
- Keep the Android TV roadmap separate from the browser roadmap; the browser app should not be forced into a 10-foot TV UI.
- Maintain the Euripus branding across desktop and browser surfaces.

## Suggested V2 Milestones

1. Extract the token storage layer so browser and desktop can use different auth persistence.
2. Add cookie-based session support to the Rust API.
3. Make the current React app run cleanly as both Tauri frontend and browser SPA.
4. Harden deployment settings for reverse proxy and public hosting.
5. Add operator docs for self-hosting a full Euripus instance.
6. Evaluate whether any providers require optional stream relay support.

## Non-Goals For The First Browser Release

- Do not re-architect the backend into separate microservices.
- Do not proxy all media by default.
- Do not combine the browser release with Android TV delivery in the same milestone.
- Do not replace PostgreSQL search unless real production scale requires it.

