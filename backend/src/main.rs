use sqlx::PgPool;
use tracing::info;

mod errors;
mod models;
mod routes;
mod state;
mod storage;
mod transcoder;

use state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Load .env in dev
    #[cfg(debug_assertions)]
    dotenvy::dotenv().ok();

    // Setup application state
    let database_url = std::env::var("DATABASE_URL").expect("Unable to find DB endpoint");
    let db = PgPool::connect(&database_url)
        .await
        .expect("Unable to connect to DB");

    let s3 = storage::create_s3_client().await;
    let bucket = "videos".to_string();

    let app_state = AppState { db, s3, bucket };

    let bucket_exists = app_state
        .s3
        .head_bucket()
        .bucket(&app_state.bucket)
        .send()
        .await
        .is_ok();

    if bucket_exists {
        info!("Connected to MinIO, bucket already exists");
    } else {
        app_state
            .s3
            .create_bucket()
            .bucket(&app_state.bucket)
            .send()
            .await
            .expect("Failed to create bucket");
        info!("Connected to MinIO, bucket created");
    }

    sqlx::migrate!("./migrations")
        .run(&app_state.db)
        .await
        .expect("Failed to run migrations");

    let app = routes::router(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
