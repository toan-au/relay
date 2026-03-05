use axum::{
    routing::{get, post},
    Router,
};

mod routes;

#[tokio::main]
async fn main() {
    let app: Router = Router::new()
        .route("/healthcheck", get(health_check))
        .route("/api/videos", post(routes::videos::upload_video));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Route handlers
async fn health_check() -> &'static str {
    "healthy"
}
