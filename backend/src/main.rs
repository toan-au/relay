use axum::{routing::get, Router};

#[tokio::main]
async fn main() {
    let app: Router = Router::new().route("/healthcheck", get(health_check));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Route handlers
async fn health_check() -> &'static str {
    "healthy"
}
