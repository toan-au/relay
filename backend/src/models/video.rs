#[derive(sqlx::FromRow)]
pub struct VideoRow {
    pub status: String,
    pub view_count: i64,
}

impl VideoRow {
    pub async fn fetch_by_token(db: &sqlx::PgPool, share_token: &str) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT status, view_count FROM videos WHERE share_token = $1")
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
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO videos (id, share_token, status) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(share_token)
            .bind("uploading")
            .execute(db)
            .await?;
        Ok(())
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
