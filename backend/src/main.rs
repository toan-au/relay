use sqlx::PgPool;
use tracing::info;

mod errors;
mod models;
mod queue;
mod routes;
mod state;
mod storage;

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
    let bucket = std::env::var("S3_BUCKET_NAME").expect("S3_BUCKET_NAME must be set");

    let (sqs, queue_url) = queue::create_sqs_client().await;

    let app_state = AppState {
        db,
        s3,
        bucket,
        sqs,
        queue_url,
    };

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
