use sqlx::PgPool;
use tracing::{info, warn};

mod errors;
mod models;
mod routes;
mod state;
mod storage;
mod transcoder;

use state::AppState;

#[tokio::main]
async fn main() {
    // Setup logging
    tracing_subscriber::fmt::init();

    // Load .env in dev
    #[cfg(debug_assertions)]
    dotenvy::dotenv().ok();

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

    sqlx::migrate!("./migrations")
        .run(&app_state.db)
        .await
        .expect("Failed to run migrations");

    let app = routes::router(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
