# API

The backend exposes OpenAPI at `/openapi.json`. The frontend generates types with:

```bash
pnpm --filter frontend generate:api
```

## Auth

- `POST /auth/register/start`: sends a 6-digit registration code.
- `POST /auth/register/verify`: verifies the registration code, creates the account, and creates a local session.
- `POST /auth/login/start`: sends a 6-digit login code for an existing account.
- `POST /auth/login/verify`: verifies the login code and creates a local session.
- `POST /auth/email/start`: legacy combined email-code start endpoint.
- `POST /auth/email/verify`: legacy combined email-code verify endpoint.
- `POST /auth/dev-login`: local-only login when `ENABLE_DEV_LOGIN=true`.
- `POST /auth/logout`: deletes the current session.
- `GET /api/me`: returns the current user or `null`.
- `PUT /api/account/profile`: updates the current user's display name.
- `GET /api/account/sessions`: lists active sessions for the current account.
- `POST /api/account/logout-all`: deletes all sessions for the current account.

Sessions are cookie-based. The cookie is httpOnly, SameSite=Lax, secure when `COOKIE_SECURE=true`, and only a hash of the session token is stored in PostgreSQL.

Email-code accounts use normalized lowercase email addresses as their stable
identity, so `Student@E.NTU.EDU.SG` and `student@e.ntu.edu.sg` are the same
account. `display_name` is optional on registration; if omitted, the backend may
infer a conservative default from readable email local-parts such as
`first.last@e.ntu.edu.sg`.

Register start request:

```json
{
  "email": "student@e.ntu.edu.sg"
}
```

Register verify request:

```json
{
  "email": "student@e.ntu.edu.sg",
  "code": "123456",
  "display_name": "Demo Student"
}
```

Login start request:

```json
{
  "email": "student@e.ntu.edu.sg"
}
```

Login verify request:

```json
{
  "email": "student@e.ntu.edu.sg",
  "code": "123456"
}
```

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
