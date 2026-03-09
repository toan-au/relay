#[derive(sqlx::FromRow)]
pub struct VideoRow {
    pub id: uuid::Uuid,
    pub status: String,
}
