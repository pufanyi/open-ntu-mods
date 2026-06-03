use std::{convert::Infallible, time::Duration};

use axum::{
    Json,
    body::Body,
    extract::{FromRequestParts, State},
    http::{
        HeaderMap, Request, StatusCode,
        header::{COOKIE, SET_COOKIE},
        request::Parts,
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use cookie::{Cookie, SameSite, time::Duration as CookieDuration};
use rand::RngCore;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    AppState,
    config::Config,
    error::{ApiError, ApiResult},
    models::{
        AccountSession, DevLoginRequest, EmailLoginStartRequest, EmailLoginStartResponse,
        EmailLoginVerifyRequest, LoginResponse, LoginStartRequest, LoginVerifyRequest, MeResponse,
        RegisterStartRequest, RegisterVerifyRequest, UpdateAccountRequest, User,
    },
};

pub const SESSION_COOKIE: &str = "ntu_session";
const EMAIL_CODE_TTL_MINUTES: i64 = 10;
const EMAIL_CODE_MAX_REQUESTS_PER_WINDOW: i64 = 5;
const EMAIL_CODE_MAX_VERIFY_ATTEMPTS: i32 = 5;
const EMAIL_PURPOSE_LEGACY: &str = "email";
const EMAIL_PURPOSE_LOGIN: &str = "login";
const EMAIL_PURPOSE_REGISTER: &str = "register";

#[derive(Clone, Debug)]
pub struct CurrentUser(pub Option<User>);

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts
            .extensions
            .get::<CurrentUser>()
            .cloned()
            .unwrap_or(CurrentUser(None)))
    }
}

pub async fn load_current_user(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let user = match session_token_from_headers(request.headers()) {
        Some(token) => find_user_by_session(&state.pool, &state.config, &token)
            .await
            .ok()
            .flatten(),
        None => None,
    };

    request.extensions_mut().insert(CurrentUser(user));
    next.run(request).await
}

pub async fn origin_protection(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    if request.uri().path() == "/health" {
        return Ok(next.run(request).await);
    }

    if state.config.require_origin_secret {
        let supplied = request
            .headers()
            .get("X-Origin-Secret")
            .and_then(|value| value.to_str().ok());
        if supplied != Some(state.config.origin_secret.as_str()) {
            return Err(ApiError::Forbidden(
                "missing or invalid origin secret".into(),
            ));
        }
    }

    Ok(next.run(request).await)
}

#[utoipa::path(
    get,
    path = "/api/me",
    responses((status = 200, body = MeResponse))
)]
pub async fn me(current_user: CurrentUser) -> Json<MeResponse> {
    Json(MeResponse {
        user: current_user.0,
    })
}

#[utoipa::path(
    post,
    path = "/auth/dev-login",
    request_body = DevLoginRequest,
    responses((status = 200, body = LoginResponse), (status = 403, body = crate::error::ErrorEnvelope))
)]
pub async fn dev_login(
    State(state): State<AppState>,
    Json(request): Json<DevLoginRequest>,
) -> ApiResult<Response> {
    if !state.config.enable_dev_login {
        return Err(ApiError::Forbidden("dev login is disabled".into()));
    }
    if !state.config.email_domain_allowed(&request.email) {
        return Err(ApiError::Forbidden("email domain is not allowed".into()));
    }

    let role = request.role.unwrap_or_else(|| "verified_user".to_string());
    validate_role(&role)?;
    let user = upsert_user(
        &state.pool,
        "dev",
        None,
        None,
        &request.email,
        request.display_name.as_deref(),
        Some(&role),
    )
    .await?;
    let cookie = create_session_cookie(&state.pool, &state.config, user.id).await?;

    let mut response = (StatusCode::OK, Json(LoginResponse { user })).into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, cookie.to_string().parse().unwrap());
    Ok(response)
}

#[utoipa::path(
    post,
    path = "/auth/email/start",
    request_body = EmailLoginStartRequest,
    responses(
        (status = 200, body = EmailLoginStartResponse),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 429, body = crate::error::ErrorEnvelope),
        (status = 503, body = crate::error::ErrorEnvelope)
    )
)]
pub async fn email_login_start(
    State(state): State<AppState>,
    Json(request): Json<EmailLoginStartRequest>,
) -> ApiResult<Json<EmailLoginStartResponse>> {
    start_email_code(&state, &request.email, EMAIL_PURPOSE_LEGACY).await
}

#[utoipa::path(
    post,
    path = "/auth/email/verify",
    request_body = EmailLoginVerifyRequest,
    responses(
        (status = 200, body = LoginResponse),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 429, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope)
    )
)]
pub async fn email_login_verify(
    State(state): State<AppState>,
    Json(request): Json<EmailLoginVerifyRequest>,
) -> ApiResult<Response> {
    let email =
        consume_email_code(&state, &request.email, &request.code, EMAIL_PURPOSE_LEGACY).await?;
    let user = upsert_user(
        &state.pool,
        "email",
        Some("email"),
        Some(&email),
        &email,
        request.display_name.as_deref(),
        None,
    )
    .await?;
    let cookie = create_session_cookie(&state.pool, &state.config, user.id).await?;

    let mut response = (StatusCode::OK, Json(LoginResponse { user })).into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, cookie.to_string().parse().unwrap());
    Ok(response)
}

#[utoipa::path(
    post,
    path = "/auth/register/start",
    request_body = RegisterStartRequest,
    responses(
        (status = 200, body = EmailLoginStartResponse),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 409, body = crate::error::ErrorEnvelope),
        (status = 429, body = crate::error::ErrorEnvelope),
        (status = 503, body = crate::error::ErrorEnvelope)
    )
)]
pub async fn register_start(
    State(state): State<AppState>,
    Json(request): Json<RegisterStartRequest>,
) -> ApiResult<Json<EmailLoginStartResponse>> {
    let email = normalize_email(&request.email)?;
    ensure_email_auth_enabled_and_allowed(&state, &email)?;
    if email_account_exists(&state.pool, &email).await? {
        return Err(ApiError::Conflict {
            message: "account already exists; log in instead".into(),
            details: None,
        });
    }
    start_email_code_for_normalized_email(&state, &email, EMAIL_PURPOSE_REGISTER).await
}

#[utoipa::path(
    post,
    path = "/auth/register/verify",
    request_body = RegisterVerifyRequest,
    responses(
        (status = 200, body = LoginResponse),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 409, body = crate::error::ErrorEnvelope),
        (status = 429, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope)
    )
)]
pub async fn register_verify(
    State(state): State<AppState>,
    Json(request): Json<RegisterVerifyRequest>,
) -> ApiResult<Response> {
    let email = consume_email_code(
        &state,
        &request.email,
        &request.code,
        EMAIL_PURPOSE_REGISTER,
    )
    .await?;
    if email_account_exists(&state.pool, &email).await? {
        return Err(ApiError::Conflict {
            message: "account already exists; log in instead".into(),
            details: None,
        });
    }

    let user = upsert_user(
        &state.pool,
        "email",
        Some("email"),
        Some(&email),
        &email,
        request.display_name.as_deref(),
        None,
    )
    .await?;
    let cookie = create_session_cookie(&state.pool, &state.config, user.id).await?;

    let mut response = (StatusCode::OK, Json(LoginResponse { user })).into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, cookie.to_string().parse().unwrap());
    Ok(response)
}

#[utoipa::path(
    post,
    path = "/auth/login/start",
    request_body = LoginStartRequest,
    responses(
        (status = 200, body = EmailLoginStartResponse),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 429, body = crate::error::ErrorEnvelope),
        (status = 503, body = crate::error::ErrorEnvelope)
    )
)]
pub async fn login_start(
    State(state): State<AppState>,
    Json(request): Json<LoginStartRequest>,
) -> ApiResult<Json<EmailLoginStartResponse>> {
    let email = normalize_email(&request.email)?;
    ensure_email_auth_enabled_and_allowed(&state, &email)?;
    if !email_account_exists(&state.pool, &email).await? {
        return Err(ApiError::NotFound(
            "account not found; register first".into(),
        ));
    }
    start_email_code_for_normalized_email(&state, &email, EMAIL_PURPOSE_LOGIN).await
}

#[utoipa::path(
    post,
    path = "/auth/login/verify",
    request_body = LoginVerifyRequest,
    responses(
        (status = 200, body = LoginResponse),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 429, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope)
    )
)]
pub async fn login_verify(
    State(state): State<AppState>,
    Json(request): Json<LoginVerifyRequest>,
) -> ApiResult<Response> {
    let email =
        consume_email_code(&state, &request.email, &request.code, EMAIL_PURPOSE_LOGIN).await?;
    let Some(user) = find_email_user(&state.pool, &email).await? else {
        return Err(ApiError::NotFound(
            "account not found; register first".into(),
        ));
    };
    let cookie = create_session_cookie(&state.pool, &state.config, user.id).await?;

    let mut response = (StatusCode::OK, Json(LoginResponse { user })).into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, cookie.to_string().parse().unwrap());
    Ok(response)
}

#[utoipa::path(
    post,
    path = "/auth/logout",
    responses((status = 204), (status = 500, body = crate::error::ErrorEnvelope))
)]
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Response> {
    if let Some(token) = session_token_from_headers(&headers) {
        let hash = hash_session_token(&state.config, &token);
        sqlx::query("delete from sessions where session_token_hash = $1")
            .bind(hash)
            .execute(&state.pool)
            .await?;
    }

    let mut cookie = Cookie::new(SESSION_COOKIE, "");
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_secure(state.config.cookie_secure);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(CookieDuration::seconds(0));

    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, cookie.to_string().parse().unwrap());
    Ok(response)
}

#[utoipa::path(
    put,
    path = "/api/account/profile",
    request_body = UpdateAccountRequest,
    responses(
        (status = 200, body = User),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope)
    )
)]
pub async fn update_account_profile(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(request): Json<UpdateAccountRequest>,
) -> ApiResult<Json<User>> {
    let user = current_user.0.ok_or(ApiError::Unauthorized)?;
    let display_name = request
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let updated = sqlx::query_as::<_, User>(
        "update users
         set display_name = $2, updated_at = now()
         where id = $1
         returning *",
    )
    .bind(user.id)
    .bind(display_name)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(updated))
}

#[utoipa::path(
    get,
    path = "/api/account/sessions",
    responses(
        (status = 200, body = Vec<AccountSession>),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope)
    )
)]
pub async fn list_account_sessions(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> ApiResult<Json<Vec<AccountSession>>> {
    let user = current_user.0.ok_or(ApiError::Unauthorized)?;
    let sessions = sqlx::query_as::<_, AccountSession>(
        "select id, created_at, expires_at
         from sessions
         where user_id = $1 and expires_at > now()
         order by created_at desc",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(sessions))
}

#[utoipa::path(
    post,
    path = "/api/account/logout-all",
    responses(
        (status = 204),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope)
    )
)]
pub async fn logout_all_sessions(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> ApiResult<Response> {
    let user = current_user.0.ok_or(ApiError::Unauthorized)?;
    sqlx::query("delete from sessions where user_id = $1")
        .bind(user.id)
        .execute(&state.pool)
        .await?;

    let mut cookie = Cookie::new(SESSION_COOKIE, "");
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_secure(state.config.cookie_secure);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(CookieDuration::seconds(0));

    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, cookie.to_string().parse().unwrap());
    Ok(response)
}

pub fn require_role(current_user: &CurrentUser, minimum_role: &str) -> ApiResult<User> {
    let user = current_user.0.clone().ok_or(ApiError::Unauthorized)?;
    if role_rank(&user.role) < role_rank(minimum_role) {
        return Err(ApiError::Forbidden(format!(
            "requires role {minimum_role} or higher"
        )));
    }
    Ok(user)
}

pub fn validate_role(role: &str) -> ApiResult<()> {
    match role {
        "reader" | "verified_user" | "trusted_editor" | "moderator" | "admin" | "owner" => Ok(()),
        _ => Err(ApiError::BadRequest(format!("unknown role: {role}"))),
    }
}

pub fn role_rank(role: &str) -> i32 {
    match role {
        "reader" => 0,
        "verified_user" => 1,
        "trusted_editor" => 2,
        "moderator" => 3,
        "admin" => 4,
        "owner" => 5,
        _ => -1,
    }
}

pub fn hash_session_token(config: &Config, token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(config.session_secret.as_bytes());
    hasher.update(b":");
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub async fn create_session_cookie(
    pool: &PgPool,
    config: &Config,
    user_id: Uuid,
) -> ApiResult<Cookie<'static>> {
    let token = random_token();
    let token_hash = hash_session_token(config, &token);
    let expires_at = Utc::now() + chrono::Duration::days(30);

    sqlx::query(
        "insert into sessions (id, user_id, session_token_hash, expires_at, created_at)
         values ($1, $2, $3, $4, now())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;

    let mut cookie = Cookie::new(SESSION_COOKIE, token);
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_secure(config.cookie_secure);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(CookieDuration::days(30));
    Ok(cookie)
}

pub async fn find_user_by_session(
    pool: &PgPool,
    config: &Config,
    token: &str,
) -> ApiResult<Option<User>> {
    let token_hash = hash_session_token(config, token);
    let user = sqlx::query_as::<_, User>(
        "select u.*
         from sessions s
         join users u on u.id = s.user_id
         where s.session_token_hash = $1 and s.expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

async fn find_email_user(pool: &PgPool, email: &str) -> ApiResult<Option<User>> {
    let user = sqlx::query_as::<_, User>(
        "select *
         from users
         where provider = 'email'
           and provider_tenant_id = 'email'
           and provider_user_id = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

async fn email_account_exists(pool: &PgPool, email: &str) -> ApiResult<bool> {
    let exists: (bool,) = sqlx::query_as(
        "select exists(
           select 1 from users
           where provider = 'email'
             and provider_tenant_id = 'email'
             and provider_user_id = $1
         )",
    )
    .bind(email)
    .fetch_one(pool)
    .await?;
    Ok(exists.0)
}

pub async fn upsert_user(
    pool: &PgPool,
    provider: &str,
    provider_tenant_id: Option<&str>,
    provider_user_id: Option<&str>,
    email: &str,
    display_name: Option<&str>,
    role: Option<&str>,
) -> ApiResult<User> {
    let now = Utc::now();

    if provider == "dev" {
        let user = sqlx::query_as::<_, User>(
            "insert into users (id, provider, provider_tenant_id, provider_user_id, email, display_name, role, created_at, updated_at)
             values ($1, 'dev', null, null, $2, $3, $4, $5, $5)
             on conflict (lower(email)) where provider = 'dev'
             do update set display_name = excluded.display_name, role = excluded.role, updated_at = excluded.updated_at
             returning *",
        )
        .bind(Uuid::new_v4())
        .bind(email)
        .bind(display_name)
        .bind(role.unwrap_or("verified_user"))
        .bind(now)
        .fetch_one(pool)
        .await?;
        return Ok(user);
    }

    let tenant = provider_tenant_id.ok_or_else(|| {
        ApiError::BadRequest("provider_tenant_id is required for production auth".into())
    })?;
    let provider_user = provider_user_id.ok_or_else(|| {
        ApiError::BadRequest("provider_user_id is required for production auth".into())
    })?;

    let mut tx = pool.begin().await?;
    let existing = sqlx::query_as::<_, User>(
        "select * from users
         where provider = $1 and provider_tenant_id = $2 and provider_user_id = $3
         for update",
    )
    .bind(provider)
    .bind(tenant)
    .bind(provider_user)
    .fetch_optional(&mut *tx)
    .await?;

    let user = if let Some(existing) = existing {
        sqlx::query_as::<_, User>(
            "update users
             set email = $2, display_name = coalesce($3, display_name), updated_at = $4
             where id = $1
             returning *",
        )
        .bind(existing.id)
        .bind(email)
        .bind(display_name)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?
    } else {
        sqlx::query_as::<_, User>(
            "insert into users (id, provider, provider_tenant_id, provider_user_id, email, display_name, role, created_at, updated_at)
             values ($1, $2, $3, $4, $5, $6, 'verified_user', $7, $7)
             returning *",
        )
        .bind(Uuid::new_v4())
        .bind(provider)
        .bind(tenant)
        .bind(provider_user)
        .bind(email)
        .bind(display_name)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?
    };
    tx.commit().await?;
    Ok(user)
}

pub async fn create_moderation_action(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    target_type: &str,
    target_id: Uuid,
    action_type: &str,
    reason: Option<&str>,
    metadata: Option<Value>,
) -> ApiResult<()> {
    sqlx::query(
        "insert into moderation_actions
         (id, actor_user_id, target_type, target_id, action_type, reason, metadata, created_at)
         values ($1, $2, $3, $4, $5, $6, $7, now())",
    )
    .bind(Uuid::new_v4())
    .bind(actor_user_id)
    .bind(target_type)
    .bind(target_id)
    .bind(action_type)
    .bind(reason)
    .bind(metadata)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, SESSION_COOKIE)
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(COOKIE)?.to_str().ok()?;
    cookie_header.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key == name).then(|| value.to_string())
    })
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn random_email_code() -> String {
    let mut rng = rand::rng();
    let value = rng.next_u32() % 1_000_000;
    format!("{value:06}")
}

fn hash_email_code(config: &Config, email: &str, code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(config.session_secret.as_bytes());
    hasher.update(b":email-login:");
    hasher.update(email.as_bytes());
    hasher.update(b":");
    hasher.update(code.as_bytes());
    hex::encode(hasher.finalize())
}

fn normalize_email(email: &str) -> ApiResult<String> {
    let email = email.trim().to_ascii_lowercase();
    if email.len() > 254
        || !email.contains('@')
        || email.starts_with('@')
        || email.ends_with('@')
        || email.contains(char::is_whitespace)
    {
        return Err(ApiError::BadRequest("invalid email address".into()));
    }
    Ok(email)
}

fn normalize_code(code: &str) -> ApiResult<String> {
    let code = code.trim();
    if code.len() != 6 || !code.chars().all(|character| character.is_ascii_digit()) {
        return Err(ApiError::BadRequest("login code must be 6 digits".into()));
    }
    Ok(code.to_string())
}

fn ensure_email_auth_enabled_and_allowed(state: &AppState, email: &str) -> ApiResult<()> {
    if !state.config.email_login_enabled {
        return Err(ApiError::Forbidden("email login is disabled".into()));
    }
    if !state.config.email_login_domain_allowed(email) {
        return Err(ApiError::Forbidden("email domain is not allowed".into()));
    }
    Ok(())
}

async fn start_email_code(
    state: &AppState,
    email: &str,
    purpose: &str,
) -> ApiResult<Json<EmailLoginStartResponse>> {
    let email = normalize_email(email)?;
    ensure_email_auth_enabled_and_allowed(state, &email)?;
    start_email_code_for_normalized_email(state, &email, purpose).await
}

async fn start_email_code_for_normalized_email(
    state: &AppState,
    email: &str,
    purpose: &str,
) -> ApiResult<Json<EmailLoginStartResponse>> {
    let recent_count: (i64,) = sqlx::query_as(
        "select count(*)
         from email_login_codes
         where lower(email) = $1 and created_at > now() - interval '10 minutes'",
    )
    .bind(email)
    .fetch_one(&state.pool)
    .await?;
    if recent_count.0 >= EMAIL_CODE_MAX_REQUESTS_PER_WINDOW {
        return Err(ApiError::TooManyRequests(
            "too many login codes requested; try again later".into(),
        ));
    }

    sqlx::query("delete from email_login_codes where expires_at < now() - interval '1 day'")
        .execute(&state.pool)
        .await?;

    let code = random_email_code();
    let code_hash = hash_email_code(&state.config, email, &code);
    let expires_at = Utc::now() + chrono::Duration::minutes(EMAIL_CODE_TTL_MINUTES);
    sqlx::query(
        "insert into email_login_codes
         (id, email, purpose, code_hash, expires_at, created_at)
         values ($1, $2, $3, $4, $5, now())",
    )
    .bind(Uuid::new_v4())
    .bind(email)
    .bind(purpose)
    .bind(code_hash)
    .bind(expires_at)
    .execute(&state.pool)
    .await?;

    send_email_code(&state.config, email, &code).await?;

    Ok(Json(EmailLoginStartResponse {
        sent: true,
        expires_in_seconds: EMAIL_CODE_TTL_MINUTES * 60,
    }))
}

async fn consume_email_code(
    state: &AppState,
    email: &str,
    code: &str,
    purpose: &str,
) -> ApiResult<String> {
    let email = normalize_email(email)?;
    ensure_email_auth_enabled_and_allowed(state, &email)?;

    let code = normalize_code(code)?;
    let mut tx = state.pool.begin().await?;
    let latest_code = sqlx::query_as::<_, (Uuid, String, i32)>(
        "select id, code_hash, attempts
         from email_login_codes
         where lower(email) = $1
           and purpose = $2
           and consumed_at is null
           and expires_at > now()
         order by created_at desc
         limit 1
         for update",
    )
    .bind(&email)
    .bind(purpose)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((code_id, expected_hash, attempts)) = latest_code else {
        return Err(ApiError::BadRequest("invalid or expired login code".into()));
    };
    if attempts >= EMAIL_CODE_MAX_VERIFY_ATTEMPTS {
        return Err(ApiError::TooManyRequests(
            "too many incorrect login code attempts".into(),
        ));
    }

    if hash_email_code(&state.config, &email, &code) != expected_hash {
        sqlx::query("update email_login_codes set attempts = attempts + 1 where id = $1")
            .bind(code_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Err(ApiError::BadRequest("invalid or expired login code".into()));
    }

    sqlx::query("update email_login_codes set consumed_at = now() where id = $1")
        .bind(code_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(email)
}

async fn send_email_code(config: &Config, email: &str, code: &str) -> ApiResult<()> {
    match config.email_login_delivery.as_str() {
        "log" => {
            tracing::warn!(
                email = %email,
                code = %code,
                "email login code generated; use EMAIL_LOGIN_DELIVERY=resend to send real email"
            );
            Ok(())
        }
        "resend" => send_resend_email(config, email, code).await,
        delivery => Err(ApiError::ServiceUnavailable(format!(
            "unsupported EMAIL_LOGIN_DELIVERY: {delivery}"
        ))),
    }
}

async fn send_resend_email(config: &Config, email: &str, code: &str) -> ApiResult<()> {
    let api_key = config
        .resend_api_key
        .as_deref()
        .ok_or_else(|| ApiError::ServiceUnavailable("RESEND_API_KEY is not configured".into()))?;
    let from = config
        .email_from
        .as_deref()
        .ok_or_else(|| ApiError::ServiceUnavailable("EMAIL_FROM is not configured".into()))?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| ApiError::Internal(error.to_string()))?;

    client
        .post("https://api.resend.com/emails")
        .bearer_auth(api_key)
        .json(&json!({
            "from": from,
            "to": [email],
            "subject": "Your Open NTU Mods login code",
            "text": format!("Your Open NTU Mods login code is {code}. It expires in {EMAIL_CODE_TTL_MINUTES} minutes.")
        }))
        .send()
        .await
        .map_err(|error| ApiError::ServiceUnavailable(error.to_string()))?
        .error_for_status()
        .map_err(|error| ApiError::ServiceUnavailable(error.to_string()))?;
    Ok(())
}
