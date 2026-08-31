#[derive(sqlx::FromRow)]
pub struct VideoRow {
    pub status: String,
    pub view_count: i64,
    pub title: String,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct AdminVideoRow {
    pub share_token: String,
    pub title: String,
    pub status: String,
    pub view_count: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl VideoRow {
    pub async fn fetch_by_token(db: &sqlx::PgPool, share_token: &str) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT status, view_count, title FROM videos WHERE share_token = $1",
        )
        .bind(share_token)
        .fetch_one(db)
        .await
    }

    /// Atomically increments the view count and returns the new value.
    /// Returns `sqlx::Error::RowNotFound` if the share token doesn't exist.
    pub async fn increment_view_count(
        db: &sqlx::PgPool,
        share_token: &str,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "UPDATE videos SET view_count = view_count + 1 WHERE share_token = $1 RETURNING view_count",
        )
        .bind(share_token)
        .fetch_one(db)
        .await
    }

    pub async fn insert(
        db: &sqlx::PgPool,
        id: uuid::Uuid,
        share_token: &str,
        title: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO videos (id, share_token, status, title) VALUES ($1, $2, $3, $4)")
            .bind(id)
            .bind(share_token)
            .bind("uploading")
            .bind(title)
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn stats(db: &sqlx::PgPool) -> Result<serde_json::Value, sqlx::Error> {
        let total_videos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM videos")
            .fetch_one(db)
            .await?;
        let total_views: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(view_count), 0)::BIGINT FROM videos")
                .fetch_one(db)
                .await?;
        let by_status: Vec<(String, i64)> =
            sqlx::query_as("SELECT status, COUNT(*) FROM videos GROUP BY status")
                .fetch_all(db)
                .await?;

        Ok(serde_json::json!({
            "total_videos": total_videos,
            "total_views": total_views,
            "by_status": by_status.into_iter().collect::<std::collections::HashMap<_, _>>(),
        }))
    }

    pub async fn list_all(db: &sqlx::PgPool) -> Result<Vec<AdminVideoRow>, sqlx::Error> {
        sqlx::query_as::<_, AdminVideoRow>(
            "SELECT share_token, title, status, view_count, created_at \
             FROM videos ORDER BY created_at DESC",
        )
        .fetch_all(db)
        .await
    }

    /// Returns `false` if no video with that share token exists.
    pub async fn update_title(
        db: &sqlx::PgPool,
        share_token: &str,
        title: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE videos SET title = $1 WHERE share_token = $2")
            .bind(title)
            .bind(share_token)
            .execute(db)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Returns `false` if no video with that share token existed.
    pub async fn delete(db: &sqlx::PgPool, share_token: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM videos WHERE share_token = $1")
            .bind(share_token)
            .execute(db)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn update_status(
        db: &sqlx::PgPool,
        share_token: &str,
        status: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET status = $1 WHERE share_token = $2")
            .bind(status)
            .bind(share_token)
            .execute(db)
            .await?;
        Ok(())
    }
}
