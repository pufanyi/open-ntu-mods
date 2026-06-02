use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{CurrentUser, create_moderation_action, require_role, role_rank, validate_role},
    error::{ApiError, ApiResult},
    models::*,
    versioning,
};

#[utoipa::path(get, path = "/health", responses((status = 200, body = HealthResponse)))]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct HealthResponse {
    pub ok: bool,
}

#[utoipa::path(
    get,
    path = "/api/courses",
    responses((status = 200, body = [Course]))
)]
pub async fn list_courses(State(state): State<AppState>) -> ApiResult<Json<Vec<Course>>> {
    let courses = sqlx::query_as::<_, Course>(
        "select * from courses where archived = false order by code asc",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(courses))
}

#[utoipa::path(
    post,
    path = "/api/courses",
    request_body = CreateCourseRequest,
    responses((status = 201, body = Course), (status = 401, body = crate::error::ErrorEnvelope))
)]
pub async fn create_course(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(request): Json<CreateCourseRequest>,
) -> ApiResult<(StatusCode, Json<Course>)> {
    require_role(&current_user, "verified_user")?;
    let code = request.code.trim().to_ascii_uppercase();
    if code.is_empty() || request.title.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "course code and title are required".into(),
        ));
    }
    let course = sqlx::query_as::<_, Course>(
        "insert into courses (id, code, title, school, au, archived, created_at, updated_at)
         values ($1, $2, $3, $4, $5, false, now(), now())
         returning *",
    )
    .bind(Uuid::new_v4())
    .bind(code)
    .bind(request.title.trim())
    .bind(request.school.as_deref())
    .bind(request.au)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(course)))
}

#[utoipa::path(
    get,
    path = "/api/courses/{code}",
    params(("code" = String, Path)),
    responses((status = 200, body = Course), (status = 404, body = crate::error::ErrorEnvelope))
)]
pub async fn get_course(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> ApiResult<Json<Course>> {
    let course = sqlx::query_as::<_, Course>("select * from courses where lower(code) = lower($1)")
        .bind(code)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("course not found".into()))?;
    Ok(Json(course))
}

#[utoipa::path(
    get,
    path = "/api/courses/{course_ref}/offerings",
    params(("course_ref" = String, Path)),
    responses((status = 200, body = [CourseOffering]))
)]
pub async fn list_course_offerings(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> ApiResult<Json<Vec<CourseOffering>>> {
    let offerings = sqlx::query_as::<_, CourseOffering>(
        "select o.*
         from course_offerings o
         join courses c on c.id = o.course_id
         where lower(c.code) = lower($1)
         order by o.academic_year desc, o.semester desc",
    )
    .bind(code)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(offerings))
}

#[utoipa::path(
    post,
    path = "/api/courses/{course_ref}/offerings",
    params(("course_ref" = Uuid, Path)),
    request_body = CreateOfferingRequest,
    responses((status = 201, body = CourseOffering))
)]
pub async fn create_offering(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(course_id): Path<Uuid>,
    Json(request): Json<CreateOfferingRequest>,
) -> ApiResult<(StatusCode, Json<CourseOffering>)> {
    require_role(&current_user, "verified_user")?;
    let mut tx = state.pool.begin().await?;
    let course_exists: Option<(Uuid,)> = sqlx::query_as("select id from courses where id = $1")
        .bind(course_id)
        .fetch_optional(&mut *tx)
        .await?;
    if course_exists.is_none() {
        return Err(ApiError::NotFound("course not found".into()));
    }

    if let Some(parent_id) = request.inherited_from_offering_id {
        let parent_course: Option<(Uuid,)> =
            sqlx::query_as("select course_id from course_offerings where id = $1")
                .bind(parent_id)
                .fetch_optional(&mut *tx)
                .await?;
        if parent_course.map(|row| row.0) != Some(course_id) {
            return Err(ApiError::BadRequest(
                "inherited offering must belong to the same course".into(),
            ));
        }
    }

    let offering = sqlx::query_as::<_, CourseOffering>(
        "insert into course_offerings
         (id, course_id, academic_year, semester, status, inherited_from_offering_id, created_at, updated_at)
         values ($1, $2, $3, $4, 'active', $5, now(), now())
         returning *",
    )
    .bind(Uuid::new_v4())
    .bind(course_id)
    .bind(request.academic_year.trim())
    .bind(request.semester.trim())
    .bind(request.inherited_from_offering_id)
    .fetch_one(&mut *tx)
    .await?;

    create_initial_sections(&mut tx, &offering).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(offering)))
}

#[utoipa::path(
    get,
    path = "/api/offerings/{offering_id}",
    params(("offering_id" = Uuid, Path)),
    responses((status = 200, body = OfferingWithCourse))
)]
pub async fn get_offering(
    State(state): State<AppState>,
    Path(offering_id): Path<Uuid>,
) -> ApiResult<Json<OfferingWithCourse>> {
    let row = sqlx::query_as::<_, OfferingCourseRow>(
        "select
           o.id as offering_id, o.course_id, o.academic_year, o.semester, o.status,
           o.inherited_from_offering_id, o.created_at as offering_created_at, o.updated_at as offering_updated_at,
           c.id as c_id, c.code, c.title, c.school, c.au, c.archived,
           c.created_at as course_created_at, c.updated_at as course_updated_at
         from course_offerings o
         join courses c on c.id = o.course_id
         where o.id = $1",
    )
    .bind(offering_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("offering not found".into()))?;
    Ok(Json(row.into_response()))
}

#[utoipa::path(
    get,
    path = "/api/offerings/{offering_id}/sections",
    params(("offering_id" = Uuid, Path)),
    responses((status = 200, body = [SectionSummary]))
)]
pub async fn list_sections(
    State(state): State<AppState>,
    Path(offering_id): Path<Uuid>,
) -> ApiResult<Json<Vec<SectionSummary>>> {
    let sections = sqlx::query_as::<_, WikiSection>(
        "select * from wiki_sections
         where offering_id = $1
         order by case section_key
           when 'overview' then 1
           when 'assessment' then 2
           when 'workload' then 3
           when 'project' then 4
           when 'exam' then 5
           when 'tips' then 6
           else 100
         end, section_key",
    )
    .bind(offering_id)
    .fetch_all(&state.pool)
    .await?;

    let mut summaries = Vec::with_capacity(sections.len());
    for section in sections {
        summaries.push(section_summary(&state.pool, section).await?);
    }
    Ok(Json(summaries))
}

#[utoipa::path(
    get,
    path = "/api/sections/{section_id}",
    params(("section_id" = Uuid, Path)),
    responses((status = 200, body = SectionDetail))
)]
pub async fn get_section(
    State(state): State<AppState>,
    Path(section_id): Path<Uuid>,
) -> ApiResult<Json<SectionDetail>> {
    let row = sqlx::query_as::<_, SectionCourseRow>(
        "select
           s.id as section_id, s.offering_id, s.section_key, s.title as section_title,
           s.head_version_id, s.inherited_from_section_id, s.locked,
           s.created_at as section_created_at, s.updated_at as section_updated_at,
           o.id as offering_row_id, o.course_id, o.academic_year, o.semester, o.status,
           o.inherited_from_offering_id, o.created_at as offering_created_at, o.updated_at as offering_updated_at,
           c.id as c_id, c.code, c.title as course_title, c.school, c.au, c.archived,
           c.created_at as course_created_at, c.updated_at as course_updated_at
         from wiki_sections s
         join course_offerings o on o.id = s.offering_id
         join courses c on c.id = o.course_id
         where s.id = $1",
    )
    .bind(section_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("section not found".into()))?;
    let visible = versioning::get_visible_version(&state.pool, row.section_id).await?;
    let verification_count = match &visible {
        Some(visible) => {
            versioning::verification_count(&state.pool, row.section_id, visible.version.id).await?
        }
        None => 0,
    };

    Ok(Json(SectionDetail {
        section: row.section(),
        offering: row.offering(),
        course: row.course(),
        current_version: visible.as_ref().map(|visible| visible.version.clone()),
        source_section_id: visible.as_ref().map(|visible| visible.source_section_id),
        inherited: visible.as_ref().is_some_and(|visible| visible.inherited),
        verification_count,
    }))
}

#[utoipa::path(
    post,
    path = "/api/sections/{section_id}/edit",
    params(("section_id" = Uuid, Path)),
    request_body = EditSectionRequest,
    responses((status = 200, body = EditSectionResponse), (status = 409, body = crate::error::ErrorEnvelope))
)]
pub async fn edit_section(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(section_id): Path<Uuid>,
    Json(request): Json<EditSectionRequest>,
) -> ApiResult<Json<EditSectionResponse>> {
    let user = require_role(&current_user, "verified_user")?;
    let response = versioning::edit_section(
        &state.pool,
        user.id,
        section_id,
        request.base_version_id,
        request.content_markdown,
        request.content_json,
        request.message,
    )
    .await?;
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/sections/{section_id}/verify",
    params(("section_id" = Uuid, Path)),
    request_body = VerifySectionRequest,
    responses((status = 200, body = VerificationResponse))
)]
pub async fn verify_section(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(section_id): Path<Uuid>,
    Json(request): Json<VerifySectionRequest>,
) -> ApiResult<Json<VerificationResponse>> {
    let user = require_role(&current_user, "verified_user")?;
    let response = versioning::verify_section(
        &state.pool,
        user.id,
        section_id,
        request.version_id,
        request
            .verification_type
            .unwrap_or_else(|| "still_accurate".into()),
    )
    .await?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/sections/{section_id}/history",
    params(("section_id" = Uuid, Path)),
    responses((status = 200, body = [HistoryItem]))
)]
pub async fn section_history(
    State(state): State<AppState>,
    Path(section_id): Path<Uuid>,
) -> ApiResult<Json<Vec<HistoryItem>>> {
    Ok(Json(
        versioning::list_history(&state.pool, section_id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/versions/{old_version_id}/diff/{new_version_id}",
    params(("old_version_id" = Uuid, Path), ("new_version_id" = Uuid, Path)),
    responses((status = 200, body = DiffResponse))
)]
pub async fn diff_versions(
    State(state): State<AppState>,
    Path((old_version_id, new_version_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<DiffResponse>> {
    Ok(Json(
        versioning::diff_versions(&state.pool, old_version_id, new_version_id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/offerings/{offering_id}/reviews",
    params(("offering_id" = Uuid, Path)),
    responses((status = 200, body = [ReviewResponse]))
)]
pub async fn list_reviews(
    State(state): State<AppState>,
    Path(offering_id): Path<Uuid>,
) -> ApiResult<Json<Vec<ReviewResponse>>> {
    let rows = sqlx::query_as::<_, ReviewRow>(
        "select
           r.id as review_id, r.offering_id, r.user_id, r.current_version_id, r.hidden, r.hidden_reason,
           r.created_at as review_created_at, r.updated_at as review_updated_at,
           rv.id as version_id, rv.review_id as version_review_id, rv.author_user_id,
           rv.rating_difficulty, rv.rating_workload, rv.rating_usefulness, rv.rating_teaching,
           rv.workload_hours_per_week, rv.body_markdown, rv.created_at as version_created_at,
           u.id as u_id, u.provider, u.provider_tenant_id, u.provider_user_id, u.email,
           u.display_name, u.role, u.created_at as user_created_at, u.updated_at as user_updated_at
         from reviews r
         join review_versions rv on rv.id = r.current_version_id
         join users u on u.id = r.user_id
         where r.offering_id = $1 and r.hidden = false
         order by rv.created_at desc",
    )
    .bind(offering_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(
        rows.into_iter().map(ReviewRow::into_response).collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/reviews",
    request_body = CreateReviewRequest,
    responses((status = 201, body = ReviewMutationResponse))
)]
pub async fn create_review(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(request): Json<CreateReviewRequest>,
) -> ApiResult<(StatusCode, Json<ReviewMutationResponse>)> {
    let user = require_role(&current_user, "verified_user")?;
    validate_review_body(
        request.rating_difficulty,
        request.rating_workload,
        request.rating_usefulness,
        request.rating_teaching,
        request.workload_hours_per_week,
        &request.body_markdown,
    )?;

    let mut tx = state.pool.begin().await?;
    let existing: Option<(Uuid,)> =
        sqlx::query_as("select id from reviews where offering_id = $1 and user_id = $2")
            .bind(request.offering_id)
            .bind(user.id)
            .fetch_optional(&mut *tx)
            .await?;
    if existing.is_some() {
        return Err(ApiError::Conflict {
            message: "user already has a review for this offering".into(),
            details: None,
        });
    }

    let review = sqlx::query_as::<_, Review>(
        "insert into reviews
         (id, offering_id, user_id, current_version_id, hidden, hidden_reason, created_at, updated_at)
         values ($1, $2, $3, null, false, null, now(), now())
         returning *",
    )
    .bind(Uuid::new_v4())
    .bind(request.offering_id)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    let version = insert_review_version(
        &mut tx,
        review.id,
        user.id,
        ReviewFields::from_create(&request),
    )
    .await?;
    let review = set_current_review_version(&mut tx, review.id, version.id).await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(ReviewMutationResponse { review, version }),
    ))
}

#[utoipa::path(
    put,
    path = "/api/reviews/{review_id}",
    params(("review_id" = Uuid, Path)),
    request_body = UpdateReviewRequest,
    responses((status = 200, body = ReviewMutationResponse), (status = 403, body = crate::error::ErrorEnvelope))
)]
pub async fn update_review(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(review_id): Path<Uuid>,
    Json(request): Json<UpdateReviewRequest>,
) -> ApiResult<Json<ReviewMutationResponse>> {
    let user = require_role(&current_user, "verified_user")?;
    validate_review_body(
        request.rating_difficulty,
        request.rating_workload,
        request.rating_usefulness,
        request.rating_teaching,
        request.workload_hours_per_week,
        &request.body_markdown,
    )?;
    let mut tx = state.pool.begin().await?;
    let review = sqlx::query_as::<_, Review>("select * from reviews where id = $1 for update")
        .bind(review_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::NotFound("review not found".into()))?;
    if review.user_id != user.id {
        return Err(ApiError::Forbidden(
            "only the original author may edit review text".into(),
        ));
    }
    let version = insert_review_version(
        &mut tx,
        review.id,
        user.id,
        ReviewFields::from_update(&request),
    )
    .await?;
    let review = set_current_review_version(&mut tx, review.id, version.id).await?;
    tx.commit().await?;
    Ok(Json(ReviewMutationResponse { review, version }))
}

#[utoipa::path(
    post,
    path = "/api/reports",
    request_body = ReportRequest,
    responses((status = 201, body = Report))
)]
pub async fn create_report(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(request): Json<ReportRequest>,
) -> ApiResult<(StatusCode, Json<Report>)> {
    let user = require_role(&current_user, "verified_user")?;
    validate_target_type(&request.target_type)?;
    if request.reason.trim().is_empty() {
        return Err(ApiError::BadRequest("reason is required".into()));
    }
    let report = sqlx::query_as::<_, Report>(
        "insert into reports (id, reporter_user_id, target_type, target_id, reason, status, created_at)
         values ($1, $2, $3, $4, $5, 'open', now())
         returning *",
    )
    .bind(Uuid::new_v4())
    .bind(user.id)
    .bind(request.target_type)
    .bind(request.target_id)
    .bind(request.reason)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(report)))
}

#[utoipa::path(
    post,
    path = "/api/admin/commits/{commit_id}/revert",
    params(("commit_id" = Uuid, Path)),
    request_body = RevertCommitRequest,
    responses((status = 200, body = EditSectionResponse))
)]
pub async fn admin_revert_commit(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(commit_id): Path<Uuid>,
    Json(request): Json<RevertCommitRequest>,
) -> ApiResult<Json<EditSectionResponse>> {
    let user = require_role(&current_user, "admin")?;
    Ok(Json(
        versioning::revert_commit(&state.pool, user.id, commit_id, request.reason).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/admin/sections/{section_id}/restore-version/{version_id}",
    params(("section_id" = Uuid, Path), ("version_id" = Uuid, Path)),
    request_body = RestoreVersionRequest,
    responses((status = 200, body = EditSectionResponse))
)]
pub async fn admin_restore_version(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path((section_id, version_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<RestoreVersionRequest>,
) -> ApiResult<Json<EditSectionResponse>> {
    let user = require_role(&current_user, "admin")?;
    Ok(Json(
        versioning::restore_version(&state.pool, user.id, section_id, version_id, request.reason)
            .await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/admin/sections/{section_id}/lock",
    params(("section_id" = Uuid, Path)),
    request_body = LockSectionRequest,
    responses((status = 200, body = WikiSection))
)]
pub async fn admin_lock_section(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(section_id): Path<Uuid>,
    Json(request): Json<LockSectionRequest>,
) -> ApiResult<Json<WikiSection>> {
    let user = require_role(&current_user, "moderator")?;
    let section = set_section_lock(
        &state.pool,
        user.id,
        section_id,
        true,
        request.reason.as_deref(),
    )
    .await?;
    Ok(Json(section))
}

#[utoipa::path(
    post,
    path = "/api/admin/sections/{section_id}/unlock",
    params(("section_id" = Uuid, Path)),
    responses((status = 200, body = WikiSection))
)]
pub async fn admin_unlock_section(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(section_id): Path<Uuid>,
) -> ApiResult<Json<WikiSection>> {
    let user = require_role(&current_user, "moderator")?;
    let section = set_section_lock(&state.pool, user.id, section_id, false, None).await?;
    Ok(Json(section))
}

#[utoipa::path(
    post,
    path = "/api/admin/reviews/{review_id}/hide",
    params(("review_id" = Uuid, Path)),
    request_body = HideReviewRequest,
    responses((status = 200, body = Review))
)]
pub async fn admin_hide_review(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(review_id): Path<Uuid>,
    Json(request): Json<HideReviewRequest>,
) -> ApiResult<Json<Review>> {
    let user = require_role(&current_user, "moderator")?;
    let review =
        set_review_hidden(&state.pool, user.id, review_id, true, Some(&request.reason)).await?;
    Ok(Json(review))
}

#[utoipa::path(
    post,
    path = "/api/admin/reviews/{review_id}/restore",
    params(("review_id" = Uuid, Path)),
    responses((status = 200, body = Review))
)]
pub async fn admin_restore_review(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(review_id): Path<Uuid>,
) -> ApiResult<Json<Review>> {
    let user = require_role(&current_user, "moderator")?;
    let review = set_review_hidden(&state.pool, user.id, review_id, false, None).await?;
    Ok(Json(review))
}

#[utoipa::path(
    get,
    path = "/api/admin/audit-log",
    responses((status = 200, body = [ModerationAction]))
)]
pub async fn admin_audit_log(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> ApiResult<Json<Vec<ModerationAction>>> {
    require_role(&current_user, "admin")?;
    let actions = sqlx::query_as::<_, ModerationAction>(
        "select * from moderation_actions order by created_at desc limit 100",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(actions))
}

#[utoipa::path(
    get,
    path = "/api/admin/reports",
    responses((status = 200, body = [Report]))
)]
pub async fn admin_reports(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> ApiResult<Json<Vec<Report>>> {
    require_role(&current_user, "trusted_editor")?;
    let reports =
        sqlx::query_as::<_, Report>("select * from reports order by created_at desc limit 100")
            .fetch_all(&state.pool)
            .await?;
    Ok(Json(reports))
}

#[utoipa::path(
    post,
    path = "/api/admin/reports/{report_id}/resolve",
    params(("report_id" = Uuid, Path)),
    request_body = ResolveReportRequest,
    responses((status = 200, body = Report))
)]
pub async fn admin_resolve_report(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(report_id): Path<Uuid>,
    Json(request): Json<ResolveReportRequest>,
) -> ApiResult<Json<Report>> {
    let user = require_role(&current_user, "trusted_editor")?;
    let mut tx = state.pool.begin().await?;
    let report = sqlx::query_as::<_, Report>(
        "update reports set status = 'resolved' where id = $1 returning *",
    )
    .bind(report_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("report not found".into()))?;
    create_moderation_action(
        &mut tx,
        user.id,
        "report",
        report_id,
        "resolve_report",
        Some(&request.reason),
        Some(json!({ "target_type": report.target_type, "target_id": report.target_id })),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(report))
}

#[utoipa::path(
    put,
    path = "/api/admin/users/{user_id}/role",
    params(("user_id" = Uuid, Path)),
    request_body = UpdateUserRoleRequest,
    responses((status = 200, body = User))
)]
pub async fn admin_update_user_role(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(user_id): Path<Uuid>,
    Json(request): Json<UpdateUserRoleRequest>,
) -> ApiResult<Json<User>> {
    let actor = require_role(&current_user, "admin")?;
    validate_role(&request.role)?;
    if role_rank(&request.role) >= role_rank("owner") && actor.role != "owner" {
        return Err(ApiError::Forbidden(
            "only owner can assign owner role".into(),
        ));
    }
    let mut tx = state.pool.begin().await?;
    let user = sqlx::query_as::<_, User>(
        "update users set role = $1, updated_at = now() where id = $2 returning *",
    )
    .bind(&request.role)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    create_moderation_action(
        &mut tx,
        actor.id,
        "user",
        user_id,
        "update_role",
        None,
        Some(json!({ "role": request.role })),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(user))
}

async fn section_summary(pool: &PgPool, section: WikiSection) -> ApiResult<SectionSummary> {
    let visible = versioning::get_visible_version(pool, section.id).await?;
    let verification_count = match &visible {
        Some(visible) => {
            versioning::verification_count(pool, section.id, visible.version.id).await?
        }
        None => 0,
    };
    Ok(SectionSummary {
        section,
        current_version: visible.as_ref().map(|visible| visible.version.clone()),
        source_section_id: visible.as_ref().map(|visible| visible.source_section_id),
        inherited: visible.as_ref().is_some_and(|visible| visible.inherited),
        verification_count,
    })
}

async fn create_initial_sections(
    tx: &mut Transaction<'_, Postgres>,
    offering: &CourseOffering,
) -> ApiResult<()> {
    if let Some(parent_id) = offering.inherited_from_offering_id {
        let parent_sections = sqlx::query_as::<_, WikiSection>(
            "select * from wiki_sections where offering_id = $1 order by section_key",
        )
        .bind(parent_id)
        .fetch_all(&mut **tx)
        .await?;
        for section in parent_sections {
            sqlx::query(
                "insert into wiki_sections
                 (id, offering_id, section_key, title, head_version_id, inherited_from_section_id, locked, created_at, updated_at)
                 values ($1, $2, $3, $4, null, $5, false, now(), now())",
            )
            .bind(Uuid::new_v4())
            .bind(offering.id)
            .bind(section.section_key)
            .bind(section.title)
            .bind(section.id)
            .execute(&mut **tx)
            .await?;
        }
    } else {
        for (key, title) in [
            ("overview", "Overview"),
            ("assessment", "Assessment"),
            ("workload", "Workload"),
            ("project", "Project"),
            ("exam", "Exam"),
            ("tips", "Tips"),
        ] {
            sqlx::query(
                "insert into wiki_sections
                 (id, offering_id, section_key, title, head_version_id, inherited_from_section_id, locked, created_at, updated_at)
                 values ($1, $2, $3, $4, null, null, false, now(), now())",
            )
            .bind(Uuid::new_v4())
            .bind(offering.id)
            .bind(key)
            .bind(title)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

async fn insert_review_version(
    tx: &mut Transaction<'_, Postgres>,
    review_id: Uuid,
    author_user_id: Uuid,
    fields: ReviewFields<'_>,
) -> ApiResult<ReviewVersion> {
    let version = sqlx::query_as::<_, ReviewVersion>(
        "insert into review_versions
         (id, review_id, author_user_id, rating_difficulty, rating_workload, rating_usefulness,
          rating_teaching, workload_hours_per_week, body_markdown, created_at)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, now())
         returning *",
    )
    .bind(Uuid::new_v4())
    .bind(review_id)
    .bind(author_user_id)
    .bind(fields.rating_difficulty)
    .bind(fields.rating_workload)
    .bind(fields.rating_usefulness)
    .bind(fields.rating_teaching)
    .bind(fields.workload_hours_per_week)
    .bind(fields.body_markdown)
    .fetch_one(&mut **tx)
    .await?;
    Ok(version)
}

async fn set_current_review_version(
    tx: &mut Transaction<'_, Postgres>,
    review_id: Uuid,
    version_id: Uuid,
) -> ApiResult<Review> {
    let review = sqlx::query_as::<_, Review>(
        "update reviews set current_version_id = $1, updated_at = now() where id = $2 returning *",
    )
    .bind(version_id)
    .bind(review_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(review)
}

async fn set_review_hidden(
    pool: &PgPool,
    actor_user_id: Uuid,
    review_id: Uuid,
    hidden: bool,
    reason: Option<&str>,
) -> ApiResult<Review> {
    let mut tx = pool.begin().await?;
    let review = sqlx::query_as::<_, Review>(
        "update reviews
         set hidden = $1, hidden_reason = case when $1 then $2 else null end, updated_at = now()
         where id = $3
         returning *",
    )
    .bind(hidden)
    .bind(reason)
    .bind(review_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("review not found".into()))?;
    create_moderation_action(
        &mut tx,
        actor_user_id,
        "review",
        review_id,
        if hidden {
            "hide_review"
        } else {
            "restore_review"
        },
        reason,
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(review)
}

async fn set_section_lock(
    pool: &PgPool,
    actor_user_id: Uuid,
    section_id: Uuid,
    locked: bool,
    reason: Option<&str>,
) -> ApiResult<WikiSection> {
    let mut tx = pool.begin().await?;
    let section = sqlx::query_as::<_, WikiSection>(
        "update wiki_sections set locked = $1, updated_at = now() where id = $2 returning *",
    )
    .bind(locked)
    .bind(section_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("section not found".into()))?;
    create_moderation_action(
        &mut tx,
        actor_user_id,
        "section",
        section_id,
        if locked {
            "lock_section"
        } else {
            "unlock_section"
        },
        reason,
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(section)
}

fn validate_review_body(
    difficulty: Option<i32>,
    workload: Option<i32>,
    usefulness: Option<i32>,
    teaching: Option<i32>,
    workload_hours: Option<i32>,
    body: &str,
) -> ApiResult<()> {
    for rating in [difficulty, workload, usefulness, teaching]
        .into_iter()
        .flatten()
    {
        if !(1..=5).contains(&rating) {
            return Err(ApiError::BadRequest(
                "ratings must be between 1 and 5".into(),
            ));
        }
    }
    if let Some(hours) = workload_hours
        && !(0..=80).contains(&hours)
    {
        return Err(ApiError::BadRequest(
            "workload hours must be between 0 and 80".into(),
        ));
    }
    if body.trim().is_empty() {
        return Err(ApiError::BadRequest("review body is required".into()));
    }
    Ok(())
}

fn validate_target_type(target_type: &str) -> ApiResult<()> {
    match target_type {
        "section" | "review" | "commit" | "course" => Ok(()),
        _ => Err(ApiError::BadRequest(
            "target_type must be section, review, commit, or course".into(),
        )),
    }
}

struct ReviewFields<'a> {
    rating_difficulty: Option<i32>,
    rating_workload: Option<i32>,
    rating_usefulness: Option<i32>,
    rating_teaching: Option<i32>,
    workload_hours_per_week: Option<i32>,
    body_markdown: &'a str,
}

impl<'a> ReviewFields<'a> {
    fn from_create(request: &'a CreateReviewRequest) -> Self {
        Self {
            rating_difficulty: request.rating_difficulty,
            rating_workload: request.rating_workload,
            rating_usefulness: request.rating_usefulness,
            rating_teaching: request.rating_teaching,
            workload_hours_per_week: request.workload_hours_per_week,
            body_markdown: &request.body_markdown,
        }
    }

    fn from_update(request: &'a UpdateReviewRequest) -> Self {
        Self {
            rating_difficulty: request.rating_difficulty,
            rating_workload: request.rating_workload,
            rating_usefulness: request.rating_usefulness,
            rating_teaching: request.rating_teaching,
            workload_hours_per_week: request.workload_hours_per_week,
            body_markdown: &request.body_markdown,
        }
    }
}

#[derive(sqlx::FromRow)]
struct OfferingCourseRow {
    offering_id: Uuid,
    course_id: Uuid,
    academic_year: String,
    semester: String,
    status: String,
    inherited_from_offering_id: Option<Uuid>,
    offering_created_at: chrono::DateTime<chrono::Utc>,
    offering_updated_at: chrono::DateTime<chrono::Utc>,
    c_id: Uuid,
    code: String,
    title: String,
    school: Option<String>,
    au: Option<i32>,
    archived: bool,
    course_created_at: chrono::DateTime<chrono::Utc>,
    course_updated_at: chrono::DateTime<chrono::Utc>,
}

impl OfferingCourseRow {
    fn into_response(self) -> OfferingWithCourse {
        OfferingWithCourse {
            offering: CourseOffering {
                id: self.offering_id,
                course_id: self.course_id,
                academic_year: self.academic_year,
                semester: self.semester,
                status: self.status,
                inherited_from_offering_id: self.inherited_from_offering_id,
                created_at: self.offering_created_at,
                updated_at: self.offering_updated_at,
            },
            course: Course {
                id: self.c_id,
                code: self.code,
                title: self.title,
                school: self.school,
                au: self.au,
                archived: self.archived,
                created_at: self.course_created_at,
                updated_at: self.course_updated_at,
            },
        }
    }
}

#[derive(sqlx::FromRow)]
struct SectionCourseRow {
    section_id: Uuid,
    offering_id: Uuid,
    section_key: String,
    section_title: String,
    head_version_id: Option<Uuid>,
    inherited_from_section_id: Option<Uuid>,
    locked: bool,
    section_created_at: chrono::DateTime<chrono::Utc>,
    section_updated_at: chrono::DateTime<chrono::Utc>,
    offering_row_id: Uuid,
    course_id: Uuid,
    academic_year: String,
    semester: String,
    status: String,
    inherited_from_offering_id: Option<Uuid>,
    offering_created_at: chrono::DateTime<chrono::Utc>,
    offering_updated_at: chrono::DateTime<chrono::Utc>,
    c_id: Uuid,
    code: String,
    course_title: String,
    school: Option<String>,
    au: Option<i32>,
    archived: bool,
    course_created_at: chrono::DateTime<chrono::Utc>,
    course_updated_at: chrono::DateTime<chrono::Utc>,
}

impl SectionCourseRow {
    fn section(&self) -> WikiSection {
        WikiSection {
            id: self.section_id,
            offering_id: self.offering_id,
            section_key: self.section_key.clone(),
            title: self.section_title.clone(),
            head_version_id: self.head_version_id,
            inherited_from_section_id: self.inherited_from_section_id,
            locked: self.locked,
            created_at: self.section_created_at,
            updated_at: self.section_updated_at,
        }
    }

    fn offering(&self) -> CourseOffering {
        CourseOffering {
            id: self.offering_row_id,
            course_id: self.course_id,
            academic_year: self.academic_year.clone(),
            semester: self.semester.clone(),
            status: self.status.clone(),
            inherited_from_offering_id: self.inherited_from_offering_id,
            created_at: self.offering_created_at,
            updated_at: self.offering_updated_at,
        }
    }

    fn course(&self) -> Course {
        Course {
            id: self.c_id,
            code: self.code.clone(),
            title: self.course_title.clone(),
            school: self.school.clone(),
            au: self.au,
            archived: self.archived,
            created_at: self.course_created_at,
            updated_at: self.course_updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ReviewRow {
    review_id: Uuid,
    offering_id: Uuid,
    user_id: Uuid,
    current_version_id: Option<Uuid>,
    hidden: bool,
    hidden_reason: Option<String>,
    review_created_at: chrono::DateTime<chrono::Utc>,
    review_updated_at: chrono::DateTime<chrono::Utc>,
    version_id: Uuid,
    version_review_id: Uuid,
    author_user_id: Uuid,
    rating_difficulty: Option<i32>,
    rating_workload: Option<i32>,
    rating_usefulness: Option<i32>,
    rating_teaching: Option<i32>,
    workload_hours_per_week: Option<i32>,
    body_markdown: String,
    version_created_at: chrono::DateTime<chrono::Utc>,
    u_id: Uuid,
    provider: String,
    provider_tenant_id: Option<String>,
    provider_user_id: Option<String>,
    email: String,
    display_name: Option<String>,
    role: String,
    user_created_at: chrono::DateTime<chrono::Utc>,
    user_updated_at: chrono::DateTime<chrono::Utc>,
}

impl ReviewRow {
    fn into_response(self) -> ReviewResponse {
        ReviewResponse {
            review: Review {
                id: self.review_id,
                offering_id: self.offering_id,
                user_id: self.user_id,
                current_version_id: self.current_version_id,
                hidden: self.hidden,
                hidden_reason: self.hidden_reason,
                created_at: self.review_created_at,
                updated_at: self.review_updated_at,
            },
            current_version: ReviewVersion {
                id: self.version_id,
                review_id: self.version_review_id,
                author_user_id: self.author_user_id,
                rating_difficulty: self.rating_difficulty,
                rating_workload: self.rating_workload,
                rating_usefulness: self.rating_usefulness,
                rating_teaching: self.rating_teaching,
                workload_hours_per_week: self.workload_hours_per_week,
                body_markdown: self.body_markdown,
                created_at: self.version_created_at,
            },
            author: User {
                id: self.u_id,
                provider: self.provider,
                provider_tenant_id: self.provider_tenant_id,
                provider_user_id: self.provider_user_id,
                email: self.email,
                display_name: self.display_name,
                role: self.role,
                created_at: self.user_created_at,
                updated_at: self.user_updated_at,
            },
        }
    }
}
