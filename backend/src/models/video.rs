#[derive(sqlx::FromRow)]
pub struct VideoRow {
    pub status: String,
}

impl VideoRow {
    pub async fn fetch_by_token(
        db: &sqlx::PgPool,
        share_token: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT status FROM videos WHERE share_token = $1")
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
