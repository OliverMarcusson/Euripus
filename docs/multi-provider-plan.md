# Multi-provider support plan

## Goal
Allow a single Euripus user to connect and manage multiple IPTV provider profiles instead of being limited to exactly one `provider_profile` per user.

## Current state
- `provider_profiles.user_id` is `UNIQUE`, so each user can only store one provider.
- The provider API and client UI are singleton-oriented:
  - `GET /provider`
  - `POST /provider/validate`
  - `PUT /provider/xtreme`
  - `POST /provider/sync`
  - `GET /provider/sync-status`
- Most synced domain data is already profile-scoped (`channels`, `programs`, `sync_jobs`, `epg_sources` all reference `profile_id`), so the main limitation is the profile management layer.

## Implementation plan

### 1. Database
- Add a migration that removes the one-provider-per-user restriction on `provider_profiles.user_id`.
- Replace it with a non-unique index on `provider_profiles(user_id)` for efficient per-user lookups.
- Keep existing `profile_id` foreign-key relationships unchanged.

### 2. Shared API types
- Update shared provider payload/types so save/validate requests can optionally target an existing provider profile by `id`.
- Add list-oriented typing where the client now expects `ProviderProfile[]` instead of a single `ProviderProfile | null`.

### 3. Server API
Refactor the provider API to be profile-list aware:
- `GET /providers` → list all provider profiles for the authenticated user.
- `POST /providers/validate` → validate submitted credentials; if `id` is present, allow blank password to reuse the stored encrypted password for that specific profile.
- `POST /providers/xtreme` → create a new provider when `id` is absent, update an existing provider when `id` is present.
- `POST /providers/{id}/sync` → trigger sync for a specific provider profile.
- `GET /providers/{id}/sync-status` → fetch the latest sync job for a specific provider profile.

Server helpers to adjust:
- Load all provider profiles for a user.
- Load one provider profile by `user_id + profile_id`.
- Save EPG sources for the targeted profile only.
- Return provider lists ordered predictably (most recently updated first).

### 4. Client API layer
- Replace singleton provider API helpers with list/profile-aware helpers:
  - `getProviders`
  - `validateProvider`
  - `saveProvider`
  - `triggerProviderSync(providerId)`
  - `getSyncStatus(providerId)`

### 5. Provider settings UI
Refactor the provider settings experience so users can switch between profiles and add new ones:
- Fetch and display the user’s provider list.
- Add a provider picker/list in the settings page.
- Support an explicit “Add provider” flow that resets the form into create mode.
- When an existing provider is selected, load it into the form for editing.
- Keep sync, validation, health, and EPG source editing scoped to the selected provider.
- Update summary badges/cards to reflect multiple providers (for example, provider count instead of assuming one provider).

### 6. Tests
Update or add tests for:
- Saving a second provider for the same user.
- Editing one provider without affecting another.
- Triggering sync for the selected provider only.
- Client settings/provider UI switching between multiple providers.
- Existing single-provider flows still working as a subset of the new multi-provider behavior.

## Notes / expected behavior
- Channel/guide/search/playback flows should continue to work because content is already stored with `profile_id` and queried by `user_id`.
- This change will merge content from multiple synced providers into the same user-visible catalog, which matches the requested capability.
- A follow-up enhancement could add user-defined provider labels, but it is not required for the initial implementation because profiles can still be distinguished via username/base URL.
