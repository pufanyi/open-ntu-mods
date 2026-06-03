use std::sync::Arc;

use axum::{Router, body::Body, http};
use chrono::Utc;
use http_body_util::BodyExt;
use open_ntu_mods_backend::{
    AppState, auth,
    config::Config,
    error::ApiError,
    models::{CreateReviewRequest, ReviewMutationResponse, UpdateReviewRequest},
    versioning,
};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const STUDENT_ID: &str = "00000000-0000-0000-0000-000000000001";
const ADMIN_ID: &str = "00000000-0000-0000-0000-000000000003";
const OFFERING_2025_ID: &str = "20000000-0000-0000-0000-000000000002";
const SECTION_2025_ASSESSMENT_ID: &str = "30000000-0000-0000-0000-000000000102";
const SECTION_2025_OVERVIEW_ID: &str = "30000000-0000-0000-0000-000000000101";

#[sqlx::test(migrations = "./migrations")]
async fn session_token_hashing_and_lookup(pool: PgPool) {
    let config = test_config();
    let token = "session-token";
    let hash_a = auth::hash_session_token(&config, token);
    let hash_b = auth::hash_session_token(&config, token);
    assert_eq!(hash_a, hash_b);
    assert_ne!(hash_a, token);

    let student_id = uuid(STUDENT_ID);
    let cookie = auth::create_session_cookie(&pool, &config, student_id)
        .await
        .unwrap();
    let found = auth::find_user_by_session(&pool, &config, cookie.value())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.id, student_id);

    let stored_token: (String,) =
        sqlx::query_as("select session_token_hash from sessions where user_id = $1")
            .bind(student_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(stored_token.0, cookie.value());
}

#[sqlx::test(migrations = "./migrations")]
async fn email_identity_is_case_insensitive_without_display_name_inference(pool: PgPool) {
    let created = auth::upsert_user(
        &pool,
        "email",
        Some("email"),
        Some("Alice.TAN@E.NTU.EDU.SG"),
        "Alice.TAN@E.NTU.EDU.SG",
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(created.email, "alice.tan@e.ntu.edu.sg");
    assert_eq!(
        created.provider_user_id.as_deref(),
        Some("alice.tan@e.ntu.edu.sg")
    );
    assert_eq!(created.display_name, None);

    let updated = auth::upsert_user(
        &pool,
        "email",
        Some("email"),
        Some("ALICE.TAN@E.NTU.EDU.SG"),
        "ALICE.TAN@E.NTU.EDU.SG",
        Some("Alice Tan"),
        None,
    )
    .await
    .unwrap();
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.email, "alice.tan@e.ntu.edu.sg");
    assert_eq!(updated.display_name.as_deref(), Some("Alice Tan"));

    let email_user_count: (i64,) =
        sqlx::query_as("select count(*) from users where provider = 'email'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(email_user_count.0, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn edit_section_creates_commit_version_change_and_detects_conflict(pool: PgPool) {
    let student_id = uuid(STUDENT_ID);
    let section_id = uuid(SECTION_2025_ASSESSMENT_ID);
    let current = versioning::get_visible_version(&pool, section_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.version.section_id, section_id);

    let edit = versioning::edit_section(
        &pool,
        student_id,
        section_id,
        Some(current.version.id),
        "Updated local AY2025/26 assessment note.".to_string(),
        None,
        "Update assessment".to_string(),
    )
    .await
    .unwrap();

    assert_eq!(edit.version.section_id, section_id);
    assert_eq!(edit.version.parent_version_id, Some(current.version.id));
    assert_eq!(edit.commit.commit_type, "edit");

    let change_count: (i64,) =
        sqlx::query_as("select count(*) from wiki_commit_changes where commit_id = $1")
            .bind(edit.commit.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(change_count.0, 1);

    let conflict = versioning::edit_section(
        &pool,
        student_id,
        section_id,
        Some(current.version.id),
        "Conflicting stale edit.".to_string(),
        None,
        "Stale edit".to_string(),
    )
    .await
    .unwrap_err();
    assert!(matches!(conflict, ApiError::Conflict { .. }));
}

#[sqlx::test(migrations = "./migrations")]
async fn restore_and_revert_create_new_versions_without_deleting_history(pool: PgPool) {
    let student_id = uuid(STUDENT_ID);
    let admin_id = uuid(ADMIN_ID);
    let section_id = uuid(SECTION_2025_OVERVIEW_ID);
    let base_version_id = versioning::get_visible_version(&pool, section_id)
        .await
        .unwrap()
        .unwrap()
        .version
        .id;

    let edit = versioning::edit_section(
        &pool,
        student_id,
        section_id,
        Some(base_version_id),
        "Temporary edit to be restored and reverted.".to_string(),
        None,
        "Temporary change".to_string(),
    )
    .await
    .unwrap();

    let restore = versioning::restore_version(
        &pool,
        admin_id,
        section_id,
        base_version_id,
        "restore seed wording".to_string(),
    )
    .await
    .unwrap();
    assert_eq!(restore.commit.commit_type, "restore");
    assert_ne!(restore.version.id, base_version_id);
    assert_eq!(restore.version.parent_version_id, Some(edit.version.id));

    let revert = versioning::revert_commit(
        &pool,
        admin_id,
        edit.commit.id,
        "undo temporary change".to_string(),
    )
    .await
    .unwrap();
    assert_eq!(revert.commit.commit_type, "revert");
    assert_ne!(revert.version.id, base_version_id);
    assert_eq!(
        revert.version.content_markdown,
        restore.version.content_markdown
    );

    let version_count: (i64,) =
        sqlx::query_as("select count(*) from wiki_versions where section_id = $1")
            .bind(section_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(version_count.0 >= 4);

    let moderation_count: (i64,) = sqlx::query_as("select count(*) from moderation_actions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(moderation_count.0 >= 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn review_author_only_edit_and_moderator_hide_are_enforced(pool: PgPool) {
    let app = test_app(pool.clone());

    let editor_cookie = dev_login_cookie(
        app.clone(),
        "editor2@e.ntu.edu.sg",
        "Second Editor",
        "verified_user",
    )
    .await;
    let create_response = request_json(
        app.clone(),
        http::Method::POST,
        "/api/reviews",
        Some(&editor_cookie),
        json!(CreateReviewRequest {
            offering_id: uuid(OFFERING_2025_ID),
            rating_difficulty: Some(3),
            rating_workload: Some(3),
            rating_usefulness: Some(4),
            rating_teaching: None,
            workload_hours_per_week: Some(7),
            body_markdown: "Author-owned review body.".to_string(),
        }),
    )
    .await;
    assert_eq!(create_response.status(), http::StatusCode::CREATED);
    let created: ReviewMutationResponse = response_json(create_response).await;

    let admin_cookie =
        dev_login_cookie(app.clone(), "admin@e.ntu.edu.sg", "Demo Admin", "admin").await;
    let edit_response = request_json(
        app.clone(),
        http::Method::PUT,
        &format!("/api/reviews/{}", created.review.id),
        Some(&admin_cookie),
        json!(UpdateReviewRequest {
            rating_difficulty: Some(5),
            rating_workload: Some(5),
            rating_usefulness: Some(5),
            rating_teaching: Some(5),
            workload_hours_per_week: Some(10),
            body_markdown: "Admins must not edit review text.".to_string(),
        }),
    )
    .await;
    assert_eq!(edit_response.status(), http::StatusCode::FORBIDDEN);

    let hide_response = request_json(
        app.clone(),
        http::Method::POST,
        &format!("/api/admin/reviews/{}/hide", created.review.id),
        Some(&admin_cookie),
        json!({ "reason": "test moderation" }),
    )
    .await;
    assert_eq!(hide_response.status(), http::StatusCode::OK);

    let visible_response = request_json(
        app,
        http::Method::GET,
        &format!("/api/offerings/{}/reviews", OFFERING_2025_ID),
        None,
        json!(null),
    )
    .await;
    let body = response_text(visible_response).await;
    assert!(!body.contains(&created.review.id.to_string()));

    let moderation_count: (i64,) =
        sqlx::query_as("select count(*) from moderation_actions where target_id = $1")
            .bind(created.review.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(moderation_count.0, 1);
}

fn test_config() -> Config {
    Config {
        database_url: "postgresql://postgres:postgres@localhost:5432/ntu_courses".into(),
        app_public_url: "http://localhost:5173".into(),
        backend_public_url: "http://localhost:3000".into(),
        session_secret: format!(
            "test-secret-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ),
        cookie_secure: false,
        require_origin_secret: false,
        origin_secret: "dev-origin-secret".into(),
        run_migrations_on_startup: false,
        email_login_enabled: true,
        email_login_delivery: "log".into(),
        email_login_allowed_domains: vec!["e.ntu.edu.sg".into(), "ntu.edu.sg".into()],
        email_from: None,
        resend_api_key: None,
        ntu_allowed_domains: vec!["e.ntu.edu.sg".into(), "ntu.edu.sg".into()],
        enable_dev_login: true,
        bind_addr: "127.0.0.1:3000".parse().unwrap(),
    }
}

fn test_app(pool: PgPool) -> Router {
    open_ntu_mods_backend::build_app(AppState {
        pool,
        config: Arc::new(test_config()),
    })
}

async fn dev_login_cookie(app: Router, email: &str, display_name: &str, role: &str) -> String {
    let response = request_json(
        app,
        http::Method::POST,
        "/auth/dev-login",
        None,
        json!({
            "email": email,
            "display_name": display_name,
            "role": role
        }),
    )
    .await;
    assert_eq!(response.status(), http::StatusCode::OK);
    response
        .headers()
        .get(http::header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

async fn request_json(
    app: Router,
    method: http::Method,
    uri: &str,
    cookie: Option<&str>,
    body: serde_json::Value,
) -> http::Response<Body> {
    let mut builder = http::Request::builder()
        .method(method.clone())
        .uri(uri)
        .header(http::header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        builder = builder.header(http::header::COOKIE, cookie);
    }
    let body = if method == http::Method::GET {
        Body::empty()
    } else {
        Body::from(body.to_string())
    };
    app.oneshot(builder.body(body).unwrap()).await.unwrap()
}

async fn response_json<T: serde::de::DeserializeOwned>(response: http::Response<Body>) -> T {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn response_text(response: http::Response<Body>) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn uuid(value: &str) -> Uuid {
    value.parse().unwrap()
}
