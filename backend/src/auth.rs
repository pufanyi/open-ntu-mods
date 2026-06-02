use std::{convert::Infallible, time::Duration};

use axum::{
    Json,
    body::Body,
    extract::{FromRequestParts, Query, State},
    http::{
        HeaderMap, Request, StatusCode,
        header::{COOKIE, SET_COOKIE},
        request::Parts,
    },
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use cookie::{Cookie, SameSite, time::Duration as CookieDuration};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    AppState,
    config::Config,
    error::{ApiError, ApiResult},
    models::{DevLoginRequest, LoginResponse, MeResponse, User},
};

pub const SESSION_COOKIE: &str = "ntu_session";
const OIDC_STATE_COOKIE: &str = "oidc_state";
const OIDC_NONCE_COOKIE: &str = "oidc_nonce";

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

#[derive(Debug, Deserialize)]
pub struct MicrosoftCallbackQuery {
    code: String,
    state: String,
}

#[derive(Debug, Deserialize)]
struct OpenIdConfiguration {
    token_endpoint: String,
    jwks_uri: String,
    issuer: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    id_token: String,
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: Option<String>,
    kty: String,
    n: Option<String>,
    e: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct IdTokenClaims {
    sub: String,
    aud: String,
    iss: String,
    exp: usize,
    nonce: Option<String>,
    tid: Option<String>,
    oid: Option<String>,
    email: Option<String>,
    preferred_username: Option<String>,
    name: Option<String>,
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
    get,
    path = "/auth/microsoft/login",
    responses(
        (status = 307, description = "Redirect to Microsoft Entra authorization endpoint"),
        (status = 503, body = crate::error::ErrorEnvelope)
    )
)]
pub async fn microsoft_login(State(state): State<AppState>) -> ApiResult<Response> {
    let client_id = state.config.microsoft_client_id.as_ref().ok_or_else(|| {
        ApiError::ServiceUnavailable("MICROSOFT_CLIENT_ID is not configured".into())
    })?;

    let state_token = random_token();
    let nonce = random_token();
    let authorize_url = format!(
        "{issuer}/oauth2/v2.0/authorize?client_id={client_id}&response_type=code&redirect_uri={redirect_uri}&scope=openid%20profile%20email&state={state_token}&nonce={nonce}",
        issuer = state.config.microsoft_issuer.trim_end_matches('/'),
        client_id = url_encode(client_id),
        redirect_uri = url_encode(&state.config.microsoft_redirect_uri()),
    );

    let mut response = Redirect::temporary(&authorize_url).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        transient_cookie(OIDC_STATE_COOKIE, &state_token, &state.config)
            .to_string()
            .parse()
            .unwrap(),
    );
    response.headers_mut().append(
        SET_COOKIE,
        transient_cookie(OIDC_NONCE_COOKIE, &nonce, &state.config)
            .to_string()
            .parse()
            .unwrap(),
    );
    Ok(response)
}

#[utoipa::path(
    get,
    path = "/auth/microsoft/callback",
    params(
        ("code" = String, Query, description = "OIDC authorization code"),
        ("state" = String, Query, description = "OIDC state value")
    ),
    responses(
        (status = 307, description = "Redirect to frontend after local session creation"),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 503, body = crate::error::ErrorEnvelope)
    )
)]
pub async fn microsoft_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<MicrosoftCallbackQuery>,
) -> ApiResult<Response> {
    let expected_state = cookie_value(&headers, OIDC_STATE_COOKIE)
        .ok_or_else(|| ApiError::BadRequest("missing OIDC state cookie".into()))?;
    let expected_nonce = cookie_value(&headers, OIDC_NONCE_COOKIE)
        .ok_or_else(|| ApiError::BadRequest("missing OIDC nonce cookie".into()))?;
    if expected_state != query.state {
        return Err(ApiError::BadRequest("OIDC state mismatch".into()));
    }

    let client_id = state.config.microsoft_client_id.as_ref().ok_or_else(|| {
        ApiError::ServiceUnavailable("MICROSOFT_CLIENT_ID is not configured".into())
    })?;
    let client_secret = state
        .config
        .microsoft_client_secret
        .as_ref()
        .ok_or_else(|| {
            ApiError::ServiceUnavailable("MICROSOFT_CLIENT_SECRET is not configured".into())
        })?;

    let discovery = discover_oidc(&state.config).await?;
    let token = exchange_code(
        &state.config,
        client_id,
        client_secret,
        &discovery,
        &query.code,
    )
    .await?;
    let claims = validate_id_token(&discovery, client_id, &token.id_token).await?;

    if claims.nonce.as_deref() != Some(expected_nonce.as_str()) {
        return Err(ApiError::BadRequest("OIDC nonce mismatch".into()));
    }

    if let Some(required_tenant) = &state.config.ntu_tenant_id
        && claims.tid.as_deref() != Some(required_tenant.as_str())
    {
        return Err(ApiError::Forbidden("token tenant is not allowed".into()));
    }

    let email = claims
        .email
        .clone()
        .or(claims.preferred_username.clone())
        .ok_or_else(|| ApiError::Forbidden("Microsoft token did not include an email".into()))?;
    if !state.config.email_domain_allowed(&email) {
        return Err(ApiError::Forbidden("email domain is not allowed".into()));
    }

    let tenant_id = claims.tid.as_deref();
    let provider_user_id = claims.oid.as_deref().unwrap_or(&claims.sub);
    let user = upsert_user(
        &state.pool,
        "microsoft",
        tenant_id,
        Some(provider_user_id),
        &email,
        claims.name.as_deref(),
        None,
    )
    .await?;
    let session_cookie = create_session_cookie(&state.pool, &state.config, user.id).await?;

    let mut response = Redirect::temporary(&state.config.app_public_url).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        session_cookie
            .to_string()
            .parse()
            .expect("valid session cookie"),
    );
    response.headers_mut().append(
        SET_COOKIE,
        expired_cookie(OIDC_STATE_COOKIE, &state.config)
            .to_string()
            .parse()
            .expect("valid state cookie"),
    );
    response.headers_mut().append(
        SET_COOKIE,
        expired_cookie(OIDC_NONCE_COOKIE, &state.config)
            .to_string()
            .parse()
            .expect("valid nonce cookie"),
    );
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
             set email = $2, display_name = $3, updated_at = $4
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

fn transient_cookie(name: &'static str, value: &str, config: &Config) -> Cookie<'static> {
    let mut cookie = Cookie::new(name, value.to_string());
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_secure(config.cookie_secure);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(CookieDuration::minutes(10));
    cookie
}

fn expired_cookie(name: &'static str, config: &Config) -> Cookie<'static> {
    let mut cookie = Cookie::new(name, "");
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_secure(config.cookie_secure);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(CookieDuration::seconds(0));
    cookie
}

fn url_encode(input: &str) -> String {
    input
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

async fn discover_oidc(config: &Config) -> ApiResult<OpenIdConfiguration> {
    let url = format!(
        "{}/.well-known/openid-configuration",
        config.microsoft_issuer.trim_end_matches('/')
    );
    reqwest::get(url)
        .await
        .map_err(|error| ApiError::ServiceUnavailable(error.to_string()))?
        .json::<OpenIdConfiguration>()
        .await
        .map_err(|error| ApiError::ServiceUnavailable(error.to_string()))
}

async fn exchange_code(
    config: &Config,
    client_id: &str,
    client_secret: &str,
    discovery: &OpenIdConfiguration,
    code: &str,
) -> ApiResult<TokenResponse> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| ApiError::Internal(error.to_string()))?;

    client
        .post(&discovery.token_endpoint)
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("redirect_uri", &config.microsoft_redirect_uri()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|error| ApiError::ServiceUnavailable(error.to_string()))?
        .error_for_status()
        .map_err(|error| ApiError::ServiceUnavailable(error.to_string()))?
        .json::<TokenResponse>()
        .await
        .map_err(|error| ApiError::ServiceUnavailable(error.to_string()))
}

async fn validate_id_token(
    discovery: &OpenIdConfiguration,
    client_id: &str,
    id_token: &str,
) -> ApiResult<IdTokenClaims> {
    let header =
        decode_header(id_token).map_err(|error| ApiError::BadRequest(error.to_string()))?;
    let kid = header
        .kid
        .ok_or_else(|| ApiError::BadRequest("id token does not include a key id".into()))?;
    let jwks = reqwest::get(&discovery.jwks_uri)
        .await
        .map_err(|error| ApiError::ServiceUnavailable(error.to_string()))?
        .json::<Jwks>()
        .await
        .map_err(|error| ApiError::ServiceUnavailable(error.to_string()))?;
    let jwk = jwks
        .keys
        .into_iter()
        .find(|key| key.kid.as_deref() == Some(kid.as_str()) && key.kty == "RSA")
        .ok_or_else(|| ApiError::BadRequest("id token signing key was not found".into()))?;
    let n = jwk
        .n
        .ok_or_else(|| ApiError::BadRequest("RSA key modulus missing".into()))?;
    let e = jwk
        .e
        .ok_or_else(|| ApiError::BadRequest("RSA key exponent missing".into()))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[client_id]);
    validation.set_issuer(&[discovery.issuer.as_str()]);

    let token = decode::<IdTokenClaims>(
        id_token,
        &DecodingKey::from_rsa_components(&n, &e)
            .map_err(|error| ApiError::BadRequest(error.to_string()))?,
        &validation,
    )
    .map_err(|error| ApiError::BadRequest(error.to_string()))?;
    Ok(token.claims)
}
