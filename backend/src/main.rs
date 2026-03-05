use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use sqlx::PgPool;

mod routes;

// 1 gb max file upload
const MAX_VIDEO_UPLOAD_SIZE: usize = 1024 * 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub s3: aws_sdk_s3::Client,
    pub bucket: String,
}

#[tokio::main]
async fn main() {
    // Load .env in dev
    #[cfg(debug_assertions)]
    dotenvy::dotenv().ok();

    // Setup AppState
    let database_url = std::env::var("DATABASE_URL").expect("Unable to find DB endpoint");
    let db = PgPool::connect(&database_url)
        .await
        .expect("Unable to connect to DB");

    let config = aws_config::load_from_env().await;
    let s3 = aws_sdk_s3::Client::new(&config);
    let bucket = "videos".to_string();

    let app_state = AppState { db, s3, bucket };

    // Setup routes
    let app: Router = Router::new()
        .route("/healthcheck", get(health_check))
        .route(
            "/api/videos",
            post(routes::videos::upload_video).layer(DefaultBodyLimit::max(MAX_VIDEO_UPLOAD_SIZE)),
        )
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Route handlers
async fn health_check() -> &'static str {
    "healthy"
}
