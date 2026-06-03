use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct User {
    pub id: Uuid,
    pub provider: String,
    pub provider_tenant_id: Option<String>,
    pub provider_user_id: Option<String>,
    pub email: String,
    pub display_name: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Course {
    pub id: Uuid,
    pub code: String,
    pub title: String,
    pub school: Option<String>,
    pub au: Option<i32>,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct CourseOffering {
    pub id: Uuid,
    pub course_id: Uuid,
    pub academic_year: String,
    pub semester: String,
    pub status: String,
    #[serde(skip_serializing, skip_deserializing)]
    #[schema(ignore)]
    pub inherited_from_offering_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct WikiSection {
    pub id: Uuid,
    pub offering_id: Uuid,
    pub section_key: String,
    pub title: String,
    pub head_version_id: Option<Uuid>,
    #[serde(skip_serializing, skip_deserializing)]
    #[schema(ignore)]
    pub inherited_from_section_id: Option<Uuid>,
    pub locked: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct WikiCommit {
    pub id: Uuid,
    pub author_user_id: Uuid,
    pub message: String,
    pub commit_type: String,
    pub reverted_commit_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct WikiVersion {
    pub id: Uuid,
    pub section_id: Uuid,
    pub commit_id: Uuid,
    pub parent_version_id: Option<Uuid>,
    pub content_markdown: String,
    pub content_json: Option<Value>,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct WikiCommitChange {
    pub id: Uuid,
    pub commit_id: Uuid,
    pub section_id: Uuid,
    pub old_version_id: Option<Uuid>,
    pub new_version_id: Option<Uuid>,
    pub change_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Review {
    pub id: Uuid,
    pub offering_id: Uuid,
    pub user_id: Uuid,
    pub current_version_id: Option<Uuid>,
    pub hidden: bool,
    pub hidden_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ReviewVersion {
    pub id: Uuid,
    pub review_id: Uuid,
    pub author_user_id: Uuid,
    pub rating_difficulty: Option<i32>,
    pub rating_workload: Option<i32>,
    pub rating_usefulness: Option<i32>,
    pub rating_teaching: Option<i32>,
    pub workload_hours_per_week: Option<i32>,
    pub body_markdown: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ModerationAction {
    pub id: Uuid,
    pub actor_user_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub action_type: String,
    pub reason: Option<String>,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Report {
    pub id: Uuid,
    pub reporter_user_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub reason: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MeResponse {
    pub user: Option<User>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DevLoginRequest {
    pub email: String,
    pub display_name: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EmailLoginStartRequest {
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EmailLoginStartResponse {
    pub sent: bool,
    pub expires_in_seconds: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EmailLoginVerifyRequest {
    pub email: String,
    pub code: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterStartRequest {
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterVerifyRequest {
    pub email: String,
    pub code: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginStartRequest {
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginVerifyRequest {
    pub email: String,
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginResponse {
    pub user: User,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateAccountRequest {
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, ToSchema)]
pub struct AccountSession {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateCourseRequest {
    pub code: String,
    pub title: String,
    pub school: Option<String>,
    pub au: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateOfferingRequest {
    pub academic_year: String,
    pub semester: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OfferingWithCourse {
    pub offering: CourseOffering,
    pub course: Course,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VisibleVersion {
    pub version: WikiVersion,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SectionSummary {
    pub section: WikiSection,
    pub current_version: Option<WikiVersion>,
    pub verification_count: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SectionDetail {
    pub section: WikiSection,
    pub offering: CourseOffering,
    pub course: Course,
    pub current_version: Option<WikiVersion>,
    pub verification_count: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EditSectionRequest {
    pub base_version_id: Option<Uuid>,
    pub content_markdown: String,
    pub content_json: Option<Value>,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EditSectionResponse {
    pub version: WikiVersion,
    pub commit: WikiCommit,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VerifySectionRequest {
    pub version_id: Uuid,
    pub verification_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VerificationResponse {
    pub section_id: Uuid,
    pub version_id: Uuid,
    pub verification_count: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HistoryItem {
    pub version: WikiVersion,
    pub commit: WikiCommit,
    pub author: User,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RestoreVersionRequest {
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RevertCommitRequest {
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DiffLine {
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DiffResponse {
    pub old_version_id: Uuid,
    pub new_version_id: Uuid,
    pub old_content: String,
    pub new_content: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateReviewRequest {
    pub offering_id: Uuid,
    pub rating_difficulty: Option<i32>,
    pub rating_workload: Option<i32>,
    pub rating_usefulness: Option<i32>,
    pub rating_teaching: Option<i32>,
    pub workload_hours_per_week: Option<i32>,
    pub body_markdown: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateReviewRequest {
    pub rating_difficulty: Option<i32>,
    pub rating_workload: Option<i32>,
    pub rating_usefulness: Option<i32>,
    pub rating_teaching: Option<i32>,
    pub workload_hours_per_week: Option<i32>,
    pub body_markdown: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReviewResponse {
    pub review: Review,
    pub current_version: ReviewVersion,
    pub author: User,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReviewMutationResponse {
    pub review: Review,
    pub version: ReviewVersion,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HideReviewRequest {
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReportRequest {
    pub target_type: String,
    pub target_id: Uuid,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ResolveReportRequest {
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateUserRoleRequest {
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LockSectionRequest {
    pub reason: Option<String>,
}
