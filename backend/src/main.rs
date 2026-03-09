use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use tracing::{info, warn};

mod routes;
mod storage;
mod transcoder;

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
    tracing_subscriber::fmt::init();

    // Load .env in dev
    #[cfg(debug_assertions)]
    dotenvy::dotenv().ok();

    // Setup AppState
    let database_url = std::env::var("DATABASE_URL").expect("Unable to find DB endpoint");
    let db = PgPool::connect(&database_url)
        .await
        .expect("Unable to connect to DB");

    let s3 = storage::create_s3_client().await;
    let bucket = "videos".to_string();

    let app_state = AppState { db, s3, bucket };

    match app_state
        .s3
        .create_bucket()
        .bucket(&app_state.bucket)
        .send()
        .await
    {
        Ok(_) => info!("Connected to MinIO, bucket created"),
        Err(e) => warn!("Bucket error (may already exist): {}", e),
    }

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&app_state.db)
        .await
        .expect("Failed to run migrations");

    // Setup routes
    let app: Router = Router::new()
        .route("/healthcheck", get(health_check))
        .route(
            "/api/videos",
            post(routes::videos::upload_video).layer(DefaultBodyLimit::max(MAX_VIDEO_UPLOAD_SIZE)),
        )
        .route("/api/videos/{share_token}", get(routes::videos::get_video))
        .route(
            "/api/videos/{share_token}/playlist.m3u8",
            get(routes::videos::get_playlist),
        )
        .route(
            "/api/videos/{share_token}/segment/{segment}",
            get(routes::videos::get_segment),
        )
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Route handlers
async fn health_check() -> &'static str {
    "healthy"
}
