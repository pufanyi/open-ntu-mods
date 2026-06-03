use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    auth::create_moderation_action,
    error::{ApiError, ApiResult},
    models::{
        DiffLine, DiffResponse, EditSectionResponse, HistoryItem, VerificationResponse,
        VisibleVersion, WikiCommit, WikiCommitChange, WikiSection, WikiVersion,
    },
};

pub async fn get_visible_version(
    pool: &PgPool,
    section_id: Uuid,
) -> ApiResult<Option<VisibleVersion>> {
    let mut conn = pool.acquire().await?;
    get_visible_version_conn(&mut conn, section_id).await
}

pub async fn get_visible_version_conn(
    conn: &mut PgConnection,
    section_id: Uuid,
) -> ApiResult<Option<VisibleVersion>> {
    let section = sqlx::query_as::<_, WikiSection>("select * from wiki_sections where id = $1")
        .bind(section_id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| ApiError::NotFound("section not found".into()))?;

    let Some(head_version_id) = section.head_version_id else {
        return Ok(None);
    };

    let version = sqlx::query_as::<_, WikiVersion>("select * from wiki_versions where id = $1")
        .bind(head_version_id)
        .fetch_one(&mut *conn)
        .await?;
    Ok(Some(VisibleVersion { version }))
}

pub async fn edit_section(
    pool: &PgPool,
    user_id: Uuid,
    section_id: Uuid,
    base_version_id: Option<Uuid>,
    content_markdown: String,
    content_json: Option<Value>,
    message: String,
) -> ApiResult<EditSectionResponse> {
    if message.trim().is_empty() {
        return Err(ApiError::BadRequest("edit message is required".into()));
    }

    let mut tx = pool.begin().await?;
    let section = lock_section(&mut tx, section_id).await?;
    if section.locked {
        return Err(ApiError::Forbidden("section is locked".into()));
    }

    let current = get_visible_version_conn(&mut tx, section_id).await?;
    let current_id = current.as_ref().map(|visible| visible.version.id);
    if current_id != base_version_id {
        return Err(ApiError::conflict_with_details(
            "section has changed since the editor loaded it",
            json!({ "current_version": current }),
        ));
    }

    let commit = create_commit(&mut tx, user_id, &message, "edit", None).await?;
    let version = create_version(
        &mut tx,
        section_id,
        commit.id,
        current_id,
        &content_markdown,
        content_json.as_ref(),
    )
    .await?;
    create_commit_change(
        &mut tx,
        commit.id,
        section_id,
        current_id,
        Some(version.id),
        "edit",
    )
    .await?;
    set_section_head(&mut tx, section_id, version.id).await?;
    tx.commit().await?;

    Ok(EditSectionResponse { version, commit })
}

pub async fn verify_section(
    pool: &PgPool,
    user_id: Uuid,
    section_id: Uuid,
    version_id: Uuid,
    verification_type: String,
) -> ApiResult<VerificationResponse> {
    let mut tx = pool.begin().await?;
    let offering = sqlx::query_as::<_, (String, String)>(
        "select o.academic_year, o.semester
         from wiki_sections s
         join course_offerings o on o.id = s.offering_id
         where s.id = $1",
    )
    .bind(section_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("section not found".into()))?;

    let version_exists: Option<(Uuid,)> =
        sqlx::query_as("select id from wiki_versions where id = $1 and section_id = $2")
            .bind(version_id)
            .bind(section_id)
            .fetch_optional(&mut *tx)
            .await?;
    if version_exists.is_none() {
        return Err(ApiError::NotFound("version not found".into()));
    }

    sqlx::query(
        "insert into section_verifications
         (id, section_id, version_id, user_id, academic_year, semester, verification_type, created_at)
         values ($1, $2, $3, $4, $5, $6, $7, now())
         on conflict do nothing",
    )
    .bind(Uuid::new_v4())
    .bind(section_id)
    .bind(version_id)
    .bind(user_id)
    .bind(&offering.0)
    .bind(&offering.1)
    .bind(verification_type)
    .execute(&mut *tx)
    .await?;

    let verification_count = verification_count_tx(&mut tx, section_id, version_id).await?;
    tx.commit().await?;
    Ok(VerificationResponse {
        section_id,
        version_id,
        verification_count,
    })
}

pub async fn restore_version(
    pool: &PgPool,
    admin_user_id: Uuid,
    section_id: Uuid,
    version_id: Uuid,
    reason: String,
) -> ApiResult<EditSectionResponse> {
    if reason.trim().is_empty() {
        return Err(ApiError::BadRequest("reason is required".into()));
    }

    let mut tx = pool.begin().await?;
    let section = lock_section(&mut tx, section_id).await?;
    if section.locked {
        return Err(ApiError::Forbidden("section is locked".into()));
    }

    let target = sqlx::query_as::<_, WikiVersion>("select * from wiki_versions where id = $1")
        .bind(version_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::NotFound("version not found".into()))?;
    if target.section_id != section_id {
        return Err(ApiError::BadRequest(
            "version does not belong to this section".into(),
        ));
    }

    let current = get_visible_version_conn(&mut tx, section_id).await?;
    let current_id = current.as_ref().map(|visible| visible.version.id);
    let commit = create_commit(
        &mut tx,
        admin_user_id,
        &format!("Restore version {version_id}: {reason}"),
        "restore",
        None,
    )
    .await?;
    let version = create_version(
        &mut tx,
        section_id,
        commit.id,
        current_id,
        &target.content_markdown,
        target.content_json.as_ref(),
    )
    .await?;
    create_commit_change(
        &mut tx,
        commit.id,
        section_id,
        current_id,
        Some(version.id),
        "restore",
    )
    .await?;
    set_section_head(&mut tx, section_id, version.id).await?;
    create_moderation_action(
        &mut tx,
        admin_user_id,
        "section",
        section_id,
        "restore_version",
        Some(&reason),
        Some(json!({ "source_version_id": version_id, "new_version_id": version.id })),
    )
    .await?;
    tx.commit().await?;

    Ok(EditSectionResponse { version, commit })
}

pub async fn revert_commit(
    pool: &PgPool,
    admin_user_id: Uuid,
    commit_id: Uuid,
    reason: String,
) -> ApiResult<EditSectionResponse> {
    if reason.trim().is_empty() {
        return Err(ApiError::BadRequest("reason is required".into()));
    }

    let mut tx = pool.begin().await?;
    let changes = sqlx::query_as::<_, WikiCommitChange>(
        "select * from wiki_commit_changes where commit_id = $1 order by id",
    )
    .bind(commit_id)
    .fetch_all(&mut *tx)
    .await?;
    if changes.is_empty() {
        return Err(ApiError::NotFound("commit has no recorded changes".into()));
    }

    let revert = create_commit(
        &mut tx,
        admin_user_id,
        &format!("Revert commit {commit_id}: {reason}"),
        "revert",
        Some(commit_id),
    )
    .await?;
    let mut last_version = None;

    // Revert is intentionally append-only: the previous content becomes a new head
    // version, so the original commit and the reverted state both remain auditable.
    for change in changes {
        let old_version_id = change.old_version_id.ok_or_else(|| {
            ApiError::BadRequest("this commit creates content and cannot be auto-reverted".into())
        })?;
        let section = lock_section(&mut tx, change.section_id).await?;
        if section.locked {
            return Err(ApiError::Forbidden("section is locked".into()));
        }
        let old_version =
            sqlx::query_as::<_, WikiVersion>("select * from wiki_versions where id = $1")
                .bind(old_version_id)
                .fetch_one(&mut *tx)
                .await?;
        let current = get_visible_version_conn(&mut tx, change.section_id).await?;
        let current_id = current.as_ref().map(|visible| visible.version.id);
        let new_version = create_version(
            &mut tx,
            change.section_id,
            revert.id,
            current_id,
            &old_version.content_markdown,
            old_version.content_json.as_ref(),
        )
        .await?;
        create_commit_change(
            &mut tx,
            revert.id,
            change.section_id,
            current_id,
            Some(new_version.id),
            "revert",
        )
        .await?;
        set_section_head(&mut tx, change.section_id, new_version.id).await?;
        last_version = Some(new_version);
    }

    create_moderation_action(
        &mut tx,
        admin_user_id,
        "commit",
        commit_id,
        "revert_commit",
        Some(&reason),
        Some(json!({ "revert_commit_id": revert.id })),
    )
    .await?;
    tx.commit().await?;

    Ok(EditSectionResponse {
        version: last_version.expect("changes are non-empty"),
        commit: revert,
    })
}

pub async fn list_history(pool: &PgPool, section_id: Uuid) -> ApiResult<Vec<HistoryItem>> {
    let mut conn = pool.acquire().await?;
    let section_exists: Option<(Uuid,)> =
        sqlx::query_as("select id from wiki_sections where id = $1")
            .bind(section_id)
            .fetch_optional(&mut *conn)
            .await?;
    if section_exists.is_none() {
        return Err(ApiError::NotFound("section not found".into()));
    }

    let history = sqlx::query_as::<_, HistoryRow>(
        "select
           v.id as version_id,
           v.section_id,
           v.commit_id,
           v.parent_version_id,
           v.content_markdown,
           v.content_json,
           v.content_hash,
           v.created_at as version_created_at,
           c.id as c_id,
           c.author_user_id,
           c.message,
           c.commit_type,
           c.reverted_commit_id,
           c.created_at as commit_created_at,
           u.id as u_id,
           u.provider,
           u.provider_tenant_id,
           u.provider_user_id,
           u.email,
           u.display_name,
           u.role,
           u.created_at as user_created_at,
           u.updated_at as user_updated_at
         from wiki_versions v
         join wiki_commits c on c.id = v.commit_id
         join users u on u.id = c.author_user_id
         where v.section_id = $1
         order by v.created_at desc",
    )
    .bind(section_id)
    .fetch_all(&mut *conn)
    .await?;

    Ok(history
        .into_iter()
        .map(|row| HistoryItem {
            version: row.version(),
            commit: row.commit(),
            author: row.author(),
        })
        .collect())
}

pub async fn diff_versions(
    pool: &PgPool,
    old_version_id: Uuid,
    new_version_id: Uuid,
) -> ApiResult<DiffResponse> {
    let old = sqlx::query_as::<_, WikiVersion>("select * from wiki_versions where id = $1")
        .bind(old_version_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("old version not found".into()))?;
    let new = sqlx::query_as::<_, WikiVersion>("select * from wiki_versions where id = $1")
        .bind(new_version_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("new version not found".into()))?;
    let lines = line_diff(&old.content_markdown, &new.content_markdown);
    Ok(DiffResponse {
        old_version_id,
        new_version_id,
        old_content: old.content_markdown,
        new_content: new.content_markdown,
        lines,
    })
}

pub fn compute_content_hash(content_markdown: &str, content_json: Option<&Value>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content_markdown.as_bytes());
    hasher.update(b"\0");
    if let Some(content_json) = content_json {
        hasher.update(
            serde_json::to_vec(content_json)
                .unwrap_or_else(|_| b"null".to_vec())
                .as_slice(),
        );
    }
    hex::encode(hasher.finalize())
}

pub fn line_diff(old: &str, new: &str) -> Vec<DiffLine> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut table = vec![vec![0usize; new_lines.len() + 1]; old_lines.len() + 1];

    for (i, old_line) in old_lines.iter().enumerate() {
        for (j, new_line) in new_lines.iter().enumerate() {
            table[i + 1][j + 1] = if old_line == new_line {
                table[i][j] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }

    let mut lines = Vec::new();
    let (mut i, mut j) = (old_lines.len(), new_lines.len());
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            lines.push(DiffLine {
                kind: "unchanged".into(),
                text: old_lines[i - 1].to_string(),
            });
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || table[i][j - 1] >= table[i - 1][j]) {
            lines.push(DiffLine {
                kind: "added".into(),
                text: new_lines[j - 1].to_string(),
            });
            j -= 1;
        } else if i > 0 {
            lines.push(DiffLine {
                kind: "removed".into(),
                text: old_lines[i - 1].to_string(),
            });
            i -= 1;
        }
    }
    lines.reverse();
    lines
}

pub async fn verification_count(
    pool: &PgPool,
    section_id: Uuid,
    version_id: Uuid,
) -> ApiResult<i64> {
    let count = sqlx::query_as::<_, (i64,)>(
        "select count(*) from section_verifications where section_id = $1 and version_id = $2",
    )
    .bind(section_id)
    .bind(version_id)
    .fetch_one(pool)
    .await?
    .0;
    Ok(count)
}

async fn verification_count_tx(
    tx: &mut Transaction<'_, Postgres>,
    section_id: Uuid,
    version_id: Uuid,
) -> ApiResult<i64> {
    let count = sqlx::query_as::<_, (i64,)>(
        "select count(*) from section_verifications where section_id = $1 and version_id = $2",
    )
    .bind(section_id)
    .bind(version_id)
    .fetch_one(&mut **tx)
    .await?
    .0;
    Ok(count)
}

async fn lock_section(
    tx: &mut Transaction<'_, Postgres>,
    section_id: Uuid,
) -> ApiResult<WikiSection> {
    sqlx::query_as::<_, WikiSection>("select * from wiki_sections where id = $1 for update")
        .bind(section_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| ApiError::NotFound("section not found".into()))
}

async fn create_commit(
    tx: &mut Transaction<'_, Postgres>,
    author_user_id: Uuid,
    message: &str,
    commit_type: &str,
    reverted_commit_id: Option<Uuid>,
) -> ApiResult<WikiCommit> {
    let commit = sqlx::query_as::<_, WikiCommit>(
        "insert into wiki_commits
         (id, author_user_id, message, commit_type, reverted_commit_id, created_at)
         values ($1, $2, $3, $4, $5, now())
         returning *",
    )
    .bind(Uuid::new_v4())
    .bind(author_user_id)
    .bind(message)
    .bind(commit_type)
    .bind(reverted_commit_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(commit)
}

async fn create_version(
    tx: &mut Transaction<'_, Postgres>,
    section_id: Uuid,
    commit_id: Uuid,
    parent_version_id: Option<Uuid>,
    content_markdown: &str,
    content_json: Option<&Value>,
) -> ApiResult<WikiVersion> {
    let hash = compute_content_hash(content_markdown, content_json);
    let version = sqlx::query_as::<_, WikiVersion>(
        "insert into wiki_versions
         (id, section_id, commit_id, parent_version_id, content_markdown, content_json, content_hash, created_at)
         values ($1, $2, $3, $4, $5, $6, $7, now())
         returning *",
    )
    .bind(Uuid::new_v4())
    .bind(section_id)
    .bind(commit_id)
    .bind(parent_version_id)
    .bind(content_markdown)
    .bind(content_json)
    .bind(hash)
    .fetch_one(&mut **tx)
    .await?;
    Ok(version)
}

async fn create_commit_change(
    tx: &mut Transaction<'_, Postgres>,
    commit_id: Uuid,
    section_id: Uuid,
    old_version_id: Option<Uuid>,
    new_version_id: Option<Uuid>,
    change_type: &str,
) -> ApiResult<()> {
    sqlx::query(
        "insert into wiki_commit_changes
         (id, commit_id, section_id, old_version_id, new_version_id, change_type)
         values ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::new_v4())
    .bind(commit_id)
    .bind(section_id)
    .bind(old_version_id)
    .bind(new_version_id)
    .bind(change_type)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn set_section_head(
    tx: &mut Transaction<'_, Postgres>,
    section_id: Uuid,
    version_id: Uuid,
) -> ApiResult<()> {
    sqlx::query("update wiki_sections set head_version_id = $1, updated_at = now() where id = $2")
        .bind(version_id)
        .bind(section_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct HistoryRow {
    version_id: Uuid,
    section_id: Uuid,
    commit_id: Uuid,
    parent_version_id: Option<Uuid>,
    content_markdown: String,
    content_json: Option<Value>,
    content_hash: String,
    version_created_at: chrono::DateTime<chrono::Utc>,
    c_id: Uuid,
    author_user_id: Uuid,
    message: String,
    commit_type: String,
    reverted_commit_id: Option<Uuid>,
    commit_created_at: chrono::DateTime<chrono::Utc>,
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

impl HistoryRow {
    fn version(&self) -> WikiVersion {
        WikiVersion {
            id: self.version_id,
            section_id: self.section_id,
            commit_id: self.commit_id,
            parent_version_id: self.parent_version_id,
            content_markdown: self.content_markdown.clone(),
            content_json: self.content_json.clone(),
            content_hash: self.content_hash.clone(),
            created_at: self.version_created_at,
        }
    }

    fn commit(&self) -> WikiCommit {
        WikiCommit {
            id: self.c_id,
            author_user_id: self.author_user_id,
            message: self.message.clone(),
            commit_type: self.commit_type.clone(),
            reverted_commit_id: self.reverted_commit_id,
            created_at: self.commit_created_at,
        }
    }

    fn author(&self) -> crate::models::User {
        crate::models::User {
            id: self.u_id,
            provider: self.provider.clone(),
            provider_tenant_id: self.provider_tenant_id.clone(),
            provider_user_id: self.provider_user_id.clone(),
            email: self.email.clone(),
            display_name: self.display_name.clone(),
            role: self.role.clone(),
            created_at: self.user_created_at,
            updated_at: self.user_updated_at,
        }
    }
}
