pub mod api;
pub mod auth;
pub mod config;
pub mod error;
pub mod models;
pub mod versioning;

use std::{path::PathBuf, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    http::{
        HeaderName, HeaderValue, Method, StatusCode,
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    },
    middleware,
    response::IntoResponse,
    routing::{get, post, put},
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    services::{ServeDir, ServeFile},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use utoipa::OpenApi;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Open NTU Mods API",
        version = "0.1.0",
        description = "Academic-year-aware NTU course wiki and review API"
    ),
    paths(
        api::health,
        auth::me,
        auth::email_login_start,
        auth::email_login_verify,
        auth::register_start,
        auth::register_verify,
        auth::login_start,
        auth::login_verify,
        auth::dev_login,
        auth::logout,
        auth::update_account_profile,
        auth::list_account_sessions,
        auth::logout_all_sessions,
        api::list_courses,
        api::create_course,
        api::get_course,
        api::list_course_offerings,
        api::create_offering,
        api::get_offering,
        api::list_sections,
        api::get_section,
        api::edit_section,
        api::verify_section,
        api::section_history,
        api::diff_versions,
        api::list_reviews,
        api::create_review,
        api::update_review,
        api::create_report,
        api::admin_revert_commit,
        api::admin_restore_version,
        api::admin_lock_section,
        api::admin_unlock_section,
        api::admin_hide_review,
        api::admin_restore_review,
        api::admin_audit_log,
        api::admin_reports,
        api::admin_resolve_report,
        api::admin_update_user_role
    ),
    components(schemas(
        api::HealthResponse,
        error::ErrorEnvelope,
        error::ErrorBody,
        models::User,
        models::Course,
        models::CourseOffering,
        models::WikiSection,
        models::WikiCommit,
        models::WikiVersion,
        models::WikiCommitChange,
        models::Review,
        models::ReviewVersion,
        models::ModerationAction,
        models::Report,
        models::MeResponse,
        models::DevLoginRequest,
        models::EmailLoginStartRequest,
        models::EmailLoginStartResponse,
        models::EmailLoginVerifyRequest,
        models::RegisterStartRequest,
        models::RegisterVerifyRequest,
        models::LoginStartRequest,
        models::LoginVerifyRequest,
        models::LoginResponse,
        models::UpdateAccountRequest,
        models::AccountSession,
        models::CreateCourseRequest,
        models::CreateOfferingRequest,
        models::OfferingWithCourse,
        models::VisibleVersion,
        models::SectionSummary,
        models::SectionDetail,
        models::EditSectionRequest,
        models::EditSectionResponse,
        models::VerifySectionRequest,
        models::VerificationResponse,
        models::HistoryItem,
        models::RestoreVersionRequest,
        models::RevertCommitRequest,
        models::DiffLine,
        models::DiffResponse,
        models::CreateReviewRequest,
        models::UpdateReviewRequest,
        models::ReviewResponse,
        models::ReviewMutationResponse,
        models::HideReviewRequest,
        models::ReportRequest,
        models::ResolveReportRequest,
        models::UpdateUserRoleRequest,
        models::LockSectionRequest
    ))
)]
pub struct ApiDoc;

pub async fn create_pool(config: &Config) -> anyhow::Result<PgPool> {
    Ok(PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await?)
}

pub fn build_app(state: AppState) -> Router {
    let config = state.config.clone();
    let app_public_url = HeaderValue::from_str(&config.app_public_url).ok();
    let backend_public_url = HeaderValue::from_str(&config.backend_public_url).ok();
    let mut origins = vec![
        HeaderValue::from_static("http://localhost:5173"),
        HeaderValue::from_static("http://127.0.0.1:5173"),
    ];
    origins.extend(app_public_url);
    origins.extend(backend_public_url);

    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            ACCEPT,
            AUTHORIZATION,
            CONTENT_TYPE,
            HeaderName::from_static("x-request-id"),
        ])
        .allow_credentials(true);

    let static_dir = std::env::var("FRONTEND_DIST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../frontend/dist"));
    let index = static_dir.join("index.html");
    let spa = ServeDir::new(static_dir).fallback(ServeFile::new(index));

    Router::new()
        .route("/health", get(api::health))
        .route("/openapi.json", get(openapi_json))
        .route("/auth/email/start", post(auth::email_login_start))
        .route("/auth/email/verify", post(auth::email_login_verify))
        .route("/auth/register/start", post(auth::register_start))
        .route("/auth/register/verify", post(auth::register_verify))
        .route("/auth/login/start", post(auth::login_start))
        .route("/auth/login/verify", post(auth::login_verify))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/dev-login", post(auth::dev_login))
        .route("/api/me", get(auth::me))
        .route("/api/account/profile", put(auth::update_account_profile))
        .route("/api/account/sessions", get(auth::list_account_sessions))
        .route("/api/account/logout-all", post(auth::logout_all_sessions))
        .route(
            "/api/courses",
            get(api::list_courses).post(api::create_course),
        )
        .route("/api/courses/{code}", get(api::get_course))
        .route(
            "/api/courses/{course_ref}/offerings",
            get(api::list_course_offerings).post(api::create_offering),
        )
        .route("/api/offerings/{offering_id}", get(api::get_offering))
        .route(
            "/api/offerings/{offering_id}/sections",
            get(api::list_sections),
        )
        .route(
            "/api/offerings/{offering_id}/reviews",
            get(api::list_reviews),
        )
        .route("/api/sections/{section_id}", get(api::get_section))
        .route("/api/sections/{section_id}/edit", post(api::edit_section))
        .route(
            "/api/sections/{section_id}/verify",
            post(api::verify_section),
        )
        .route(
            "/api/sections/{section_id}/history",
            get(api::section_history),
        )
        .route(
            "/api/versions/{old_version_id}/diff/{new_version_id}",
            get(api::diff_versions),
        )
        .route("/api/reviews", post(api::create_review))
        .route("/api/reviews/{review_id}", put(api::update_review))
        .route("/api/reports", post(api::create_report))
        .route(
            "/api/admin/commits/{commit_id}/revert",
            post(api::admin_revert_commit),
        )
        .route(
            "/api/admin/sections/{section_id}/restore-version/{version_id}",
            post(api::admin_restore_version),
        )
        .route(
            "/api/admin/sections/{section_id}/lock",
            post(api::admin_lock_section),
        )
        .route(
            "/api/admin/sections/{section_id}/unlock",
            post(api::admin_unlock_section),
        )
        .route(
            "/api/admin/reviews/{review_id}/hide",
            post(api::admin_hide_review),
        )
        .route(
            "/api/admin/reviews/{review_id}/restore",
            post(api::admin_restore_review),
        )
        .route("/api/admin/audit-log", get(api::admin_audit_log))
        .route("/api/admin/reports", get(api::admin_reports))
        .route(
            "/api/admin/reports/{report_id}/resolve",
            post(api::admin_resolve_report),
        )
        .route(
            "/api/admin/users/{user_id}/role",
            put(api::admin_update_user_role),
        )
        .route(
            "/api/{*path}",
            get(api_not_found).post(api_not_found).put(api_not_found),
        )
        .fallback_service(spa)
        .with_state(state.clone())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(15),
        ))
        .layer(cors)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::load_current_user,
        ))
        .layer(middleware::from_fn_with_state(
            state,
            auth::origin_protection,
        ))
}

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi())
}

async fn api_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(crate::error::ErrorEnvelope {
            error: crate::error::ErrorBody {
                code: "not_found".into(),
                message: "API route not found".into(),
                details: None,
            },
        }),
    )
}
