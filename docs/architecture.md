# Architecture

## System Overview

Open NTU Mods is a monorepo with three deployable parts:

- `backend/`: Axum JSON API, SQLx/PostgreSQL data model, cookie sessions, OpenAPI, and production static frontend serving.
- `frontend/`: React/Vite SPA using generated OpenAPI types and `openapi-fetch`.
- `worker/`: Cloudflare Worker reverse proxy in front of the Railway backend.

Railway hosts PostgreSQL and the Rust app. Cloudflare Workers adds security headers, forwards `X-Origin-Secret`, and caches public GET responses for course/wiki pages.

## Data Model

Courses are stable catalog entries. `course_offerings` represent an academic year and semester. Each offering has `wiki_sections`. A section may inherit from a previous offering's section and may have no local `head_version_id` until it is edited.

Wiki content is stored through:

- `wiki_commits`: immutable author/action record.
- `wiki_versions`: immutable section content snapshots.
- `wiki_commit_changes`: immutable mapping from commit to old/new section versions.

Reviews are separate from wiki content. Each `reviews` row is owned by one user for one offering and points to immutable `review_versions`.

## Immutable Versions

Wiki edits never update existing `wiki_versions`, `wiki_commits`, or `wiki_commit_changes`. Editing creates all three records in a transaction and moves only `wiki_sections.head_version_id`.

This gives the system a durable audit trail and makes revert/restore append-only.

## Edit, Restore, Revert

`POST /api/sections/:id/edit` requires `base_version_id`. The backend resolves the visible version, including inherited content, and returns `409 Conflict` if the visible version differs.

Restore creates a new `wiki_commit` of type `restore`, copies the target version content into a new local version, and records a moderation action.

Revert creates a new `wiki_commit` of type `revert`. For MVP it supports normal section edits by copying each changed row's `old_version_id` into a new head version. It does not delete the reverted commit.

## Academic-Year Inheritance

New offerings can point to `inherited_from_offering_id`. Their sections point to previous sections using `inherited_from_section_id`.

Visible content resolution:

1. Use the local section `head_version_id` when present.
2. Otherwise walk `inherited_from_section_id`.
3. Display whether content is local or inherited.

Editing an inherited section creates a local version for the new academic-year section, which is the copy-on-write behavior.

## Reviews

A user can create one review per offering. Editing a review creates a new `review_versions` row and updates `reviews.current_version_id`.

Only the original author may edit review text. Moderators/admins can hide or restore reviews, and those actions create `moderation_actions`. Moderators/admins do not have an endpoint that edits review text.

## Permissions

Roles are ranked:

`reader < verified_user < trusted_editor < moderator < admin < owner`

- Public unauthenticated users can read course/wiki/review pages.
- `verified_user` can edit wiki sections, verify sections, create/edit own reviews, and report content.
- `trusted_editor` can resolve low-risk reports.
- `moderator` can hide/restore reviews and lock/unlock sections.
- `admin` can revert commits, restore versions, manage roles, and view audit logs.
- `owner` is reserved for full system administration.

## Worker and Railway

The Worker proxies requests to `RAILWAY_ORIGIN`, adds `X-Origin-Secret`, and injects basic security headers. In production set `REQUIRE_ORIGIN_SECRET=true` on the backend so direct Railway requests without the secret are rejected.

The Worker caches only public GET routes under `/api/courses`, `/api/offerings`, and `/api/sections`. It does not cache auth routes, `/api/me`, admin routes, or non-GET requests.

