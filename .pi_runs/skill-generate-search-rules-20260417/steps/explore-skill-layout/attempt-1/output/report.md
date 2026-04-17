---
run_id: skill-generate-search-rules-20260417
step_id: explore-skill-layout
role: explorer
attempt: 1
created_at: 2026-04-17T00:00:00Z
source_steps: []
repo_root: /home/oliver/src/Rust/Euripus
objective_digest: f44d044dfc143678
---

## Goal
Inspect first-party repository files for any existing PI skill layout, repository-specific agent instructions, and concrete conventions that would matter when adding a new skill for search rule generation.

## Relevant Files
- `README.md`
  - Workspace overview; no PI-specific or agent-specific guidance found.
- `docs/admin-search-rule-json.md`
  - Canonical JSON import format for admin search rule groups.
- `apps/client/src/features/admin/admin-page.tsx`
  - Admin UI for creating, editing, deleting, importing, and testing search pattern groups.
- `apps/client/src/lib/api.ts`
  - Client-side API bindings for admin search rule endpoints.
- `packages/shared/src/index.ts`
  - Shared TypeScript contracts for admin pattern groups, imports, and test responses.
- `apps/server/src/server_main/admin.rs`
  - Admin auth, CRUD/import/test endpoints, import validation, and reindex trigger behavior.
- `apps/server/src/server_main/search/rules.rs`
  - Rule parsing, normalization, evaluation, and loaded group shape.
- `apps/server/src/server_main/search/mod.rs`
  - Search routes and filter-option derivation from enabled admin rule groups.
- `apps/server/src/server_main/search/lexicon.rs`
  - Search query operator parsing (`country:`, `provider:`, `ppv`, `vip`, `epg`) and related search semantics.
- `apps/server/migrations/0019_admin_search_rules.sql`
  - Initial database schema for admin search pattern groups and patterns.

## Findings
1. **No existing PI skill structure was found in the allowed repo paths.**
   - `.pi` does not exist at the repo root.
   - `AGENTS.md` does not exist at the repo root.
   - No PI/skill-specific files were found under `docs`, `apps`, or `packages`.
   - Searches for `skill` in first-party allowed paths returned no PI-related results.

2. **There are no repository-local agent instructions in the inspected first-party paths.**
   - No `AGENTS.md` file exists.
   - No alternative agent-instruction file was found in the allowed paths.
   - As inspected, the repo does not currently advertise any conventions for PI agents, skills, prompts, or subagents.

3. **The repo already has a mature first-party search rule system that a future skill would likely target.**
   - Backend admin routes are defined in `apps/server/src/server_main/admin.rs`:
     - `POST /admin/auth/login`
     - `POST /admin/auth/logout`
     - `GET/POST/DELETE /admin/search/pattern-groups`
     - `POST /admin/search/pattern-group-import`
     - `PUT/DELETE /admin/search/pattern-groups/{id}`
     - `POST /admin/search/test`
     - `POST /admin/search/test-query`
   - These endpoints are already consumed by the web client in `apps/client/src/lib/api.ts`.

4. **The canonical payload shape for bulk-generated rules is already documented.**
   - `docs/admin-search-rule-json.md` defines the JSON import format the admin UI accepts.
   - Canonical item fields:
     - `kind`
     - `value`
     - `matchTarget`
     - `matchMode`
     - `priority`
     - `enabled`
     - `patterns`
     - `countryCodes` for provider groups
   - This doc is the strongest existing repo convention for machine-generated search rules.

5. **The admin UI already exposes a natural integration point for generated output: JSON import.**
   - `apps/client/src/features/admin/admin-page.tsx` includes an `Add JSON` modal.
   - The modal explicitly points users to `docs/admin-search-rule-json.md`.
   - Parsed JSON is sent as `{ groups: parsed.groups }` to `importAdminPatternGroups(...)`.
   - This implies a skill that generates search rules does not need a new storage format if it outputs the documented JSON array.

6. **Shared contracts reinforce the same import convention.**
   - `packages/shared/src/index.ts` defines:
     - `AdminPatternGroupImportInput`
     - `AdminPatternGroupImportRequest`
     - `AdminPatternGroupImportError`
   - The import input fields match the documentation and UI behavior.
   - This makes the JSON import schema a first-class repo-level contract, not just a docs note.

7. **Backend import validation is strict and useful for skill-output constraints.**
   - `apps/server/src/server_main/admin.rs` validates:
     - non-empty `value`
     - valid `kind`
     - valid `matchTarget`
     - valid `matchMode`
     - at least one pattern
     - provider groups require at least one related country code
     - provider country codes must exist already or be introduced by country groups in the same batch
   - Defaults:
     - `priority` defaults to `0`
     - `enabled` defaults to `true`
   - Import is all-or-nothing via `AppError::BadRequestDetailed`.

8. **Rule semantics are simple and deterministic.**
   - `apps/server/src/server_main/search/rules.rs` shows:
     - patterns are normalized by trimming and lowercasing
     - `patternsText` is comma-separated
     - `countryCodesText` is comma-separated and lowercased
     - match modes are `prefix`, `contains`, `exact`
     - match targets are `channel_name`, `category_name`, `program_title`, `channel_or_category`, `any_text`
   - Evaluation behavior:
     - only enabled groups are considered
     - country/provider winners are chosen by higher `priority`, then longer matched pattern
     - supported flag values with behavior are `ppv`, `vip`, and `force_epg`

9. **Search UI/filter behavior depends on enabled admin rules.**
   - `apps/server/src/server_main/search/mod.rs` builds search filter options from enabled groups only.
   - Enabled country groups become available country filters.
   - Enabled provider groups become provider filters, constrained to enabled country codes.
   - This means generated rules affect both metadata enrichment and search filter UX.

10. **Search query syntax relevant to generated rules is already fixed in code.**
    - `apps/server/src/server_main/search/lexicon.rs` parses:
      - `country:<code>`
      - `provider:<value>`
      - `ppv` / `!ppv`
      - `vip` / `!vip`
      - `epg`
    - A rule-generation skill should align generated `value` fields with these normalized search operators.

11. **Database schema confirms the current data model for admin rules.**
    - `apps/server/migrations/0019_admin_search_rules.sql` creates:
      - `admin_search_pattern_groups`
      - `admin_search_patterns`
   - The later runtime code also uses `admin_search_provider_countries`; that table is not created in `0019_admin_search_rules.sql`, so it must come from another migration not inspected here or from a later schema change outside the specifically read files.

12. **Most likely repo content convention for a future search-rule-generation skill is “produce importable JSON,” not “edit app code.”**
    - Evidence:
      - dedicated JSON import docs
      - dedicated import API
      - shared import types
      - existing admin UI entry point
    - There is no existing evidence of a repo-managed PI skill framework to plug into.

## Open Questions
1. If a PI skill is to be added, what repository location should host it, given:
   - no `.pi` directory exists,
   - no `AGENTS.md` exists,
   - and no skill conventions were found in allowed first-party paths?

2. Is there an uninspected migration that creates `admin_search_provider_countries`?
   - Runtime code in `apps/server/src/server_main/admin.rs` and `apps/server/src/server_main/search/rules.rs` depends on it.
   - `apps/server/migrations/0019_admin_search_rules.sql` does not define it.

3. Should a future skill generate:
   - only canonical import JSON arrays,
   - wrapped request payloads of shape `{ groups: [...] }`,
   - or both?

4. Should generated flag rules include `force_epg` in addition to `ppv` and `vip`?
   - `rules.rs` supports it at evaluation time.
   - `docs/admin-search-rule-json.md` examples only mention `ppv` and `vip`.

5. Should a future skill be purely repo-external/planning-oriented, or should the repo eventually gain PI-specific metadata/instructions for it?

## Recommended Next Step
Use the existing admin search rule import contract as the primary target format for planning the new skill:
- base the skill’s output on `docs/admin-search-rule-json.md`
- validate assumptions against:
  - `packages/shared/src/index.ts`
  - `apps/server/src/server_main/admin.rs`
  - `apps/server/src/server_main/search/rules.rs`

In parallel, clarify where PI-specific repo assets should live, because the current repository has no existing `.pi` layout or `AGENTS.md` convention to extend.