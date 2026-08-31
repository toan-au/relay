pub struct AdminSession;

impl AdminSession {
    /// Creates a new session and returns its token.
    pub async fn create(db: &sqlx::PgPool) -> Result<String, sqlx::Error> {
        let token = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO admin_sessions (token, expires_at) VALUES ($1, now() + interval '24 hours')",
        )
        .bind(&token)
        .execute(db)
        .await?;
        Ok(token)
    }

    pub async fn is_valid(db: &sqlx::PgPool, token: &str) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM admin_sessions WHERE token = $1 AND expires_at > now())",
        )
        .bind(token)
        .fetch_one(db)
        .await
    }

    pub async fn delete(db: &sqlx::PgPool, token: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM admin_sessions WHERE token = $1")
            .bind(token)
            .execute(db)
            .await?;
        Ok(())
    }
}
