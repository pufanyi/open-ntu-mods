# Architecture

## System Overview

Open NTU Mods is a monorepo with three deployable parts:

- `backend/`: Axum JSON API, SQLx/PostgreSQL data model, cookie sessions, OpenAPI, and production static frontend serving.
- `frontend/`: React/Vite SPA using generated OpenAPI types and `openapi-fetch`.
- `worker/`: Cloudflare Worker reverse proxy in front of the Railway backend.

Railway hosts PostgreSQL and the Rust app. Cloudflare Workers adds security headers, forwards `X-Origin-Secret`, and caches public GET responses for course/wiki pages.

## Auth

Authentication uses email verification codes plus local cookie sessions. The
backend stores only hashed session tokens and hashed one-time email codes.
`EMAIL_LOGIN_DELIVERY=log` prints codes to backend logs for early testing;
`EMAIL_LOGIN_DELIVERY=resend` sends real email through Resend.

## Data Model

Courses are stable catalog entries. `course_offerings` represent an academic year and semester. Each offering has standalone `wiki_sections`. A section points at its current immutable version through `head_version_id`; older versions remain available in section history.

Wiki content is stored through:

- `wiki_commits`: immutable author/action record.
- `wiki_versions`: immutable section content snapshots.
- `wiki_commit_changes`: immutable mapping from commit to old/new section versions.

Reviews are separate from wiki content. Each `reviews` row is owned by one user for one offering and points to immutable `review_versions`.

## Immutable Versions

Wiki edits never update existing `wiki_versions`, `wiki_commits`, or `wiki_commit_changes`. Editing creates all three records in a transaction and moves only `wiki_sections.head_version_id`.

This gives the system a durable audit trail and makes revert/restore append-only.

## Edit, Restore, Revert

`POST /api/sections/:id/edit` requires `base_version_id`. The backend compares it with the section's current `head_version_id` and returns `409 Conflict` if the current version differs.

Restore creates a new `wiki_commit` of type `restore`, copies the target version content into a new version for that same section, and records a moderation action.

Revert creates a new `wiki_commit` of type `revert`. For MVP it supports normal section edits by copying each changed row's `old_version_id` into a new head version. It does not delete the reverted commit.

## Academic-Year Pages

Academic years and semesters are first-class through `course_offerings`, but the MVP does not use cross-year inheritance. A new offering starts with its own standalone section pages. Users can edit the current page and use history to view any prior version for that page.

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

The Worker proxies requests to `RAILWAY_ORIGIN`, adds `X-Origin-Secret`, and injects basic security headers. In production set `REQUIRE_ORIGIN_SECRET=true` on the backend so direct Railway requests without the secret are rejected. `/health` is the only exception so Railway can run platform health checks.

The Worker caches only public GET routes under `/api/courses`, `/api/offerings`, and `/api/sections`. It does not cache auth routes, `/api/me`, admin routes, or non-GET requests.
