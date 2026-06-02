# API

The backend exposes OpenAPI at `/openapi.json`. The frontend generates types with:

```bash
pnpm --filter frontend generate:api
```

## Auth

- `GET /auth/microsoft/login`: starts Microsoft Entra OIDC login.
- `GET /auth/microsoft/callback`: validates OIDC callback and creates a local session.
- `POST /auth/dev-login`: local-only login when `ENABLE_DEV_LOGIN=true`.
- `POST /auth/logout`: deletes the current session.
- `GET /api/me`: returns the current user or `null`.

Sessions are cookie-based. The cookie is httpOnly, SameSite=Lax, secure when `COOKIE_SECURE=true`, and only a hash of the session token is stored in PostgreSQL.

Dev login request:

```json
{
  "email": "student@e.ntu.edu.sg",
  "display_name": "Demo Student",
  "role": "verified_user"
}
```

## Public Endpoints

- `GET /api/courses`
- `GET /api/courses/:code`
- `GET /api/courses/:code/offerings`
- `GET /api/offerings/:offering_id`
- `GET /api/offerings/:offering_id/sections`
- `GET /api/sections/:section_id`
- `GET /api/sections/:section_id/history`
- `GET /api/versions/:old_version_id/diff/:new_version_id`
- `GET /api/offerings/:offering_id/reviews`

## Verified User Endpoints

- `POST /api/courses`
- `POST /api/courses/:course_id/offerings`
- `POST /api/sections/:section_id/edit`
- `POST /api/sections/:section_id/verify`
- `POST /api/reviews`
- `PUT /api/reviews/:review_id`
- `POST /api/reports`

Section edit:

```json
{
  "base_version_id": "50000000-0000-0000-0000-000000000101",
  "content_markdown": "Updated public section text.",
  "content_json": null,
  "message": "Updated assessment breakdown"
}
```

On stale `base_version_id`, the API returns `409 Conflict` with current version details.

## Admin and Moderation Endpoints

- `POST /api/admin/commits/:commit_id/revert`
- `POST /api/admin/sections/:section_id/restore-version/:version_id`
- `POST /api/admin/sections/:section_id/lock`
- `POST /api/admin/sections/:section_id/unlock`
- `POST /api/admin/reviews/:review_id/hide`
- `POST /api/admin/reviews/:review_id/restore`
- `GET /api/admin/audit-log`
- `GET /api/admin/reports`
- `POST /api/admin/reports/:report_id/resolve`
- `PUT /api/admin/users/:user_id/role`

Hide review:

```json
{
  "reason": "Contains private or inappropriate content"
}
```

Every moderation/admin action that changes state writes a `moderation_actions` row.

## Error Shape

```json
{
  "error": {
    "code": "conflict",
    "message": "section has changed since the editor loaded it",
    "details": {}
  }
}
```

