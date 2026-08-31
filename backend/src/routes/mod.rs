pub mod admin;
pub mod videos;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, patch, post},
    Router,
};

use crate::state::AppState;

const MAX_VIDEO_UPLOAD_SIZE: usize = 1024 * 1024 * 1024;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthcheck", get(health_check))
        .route(
            "/api/videos",
            post(videos::upload_video).layer(DefaultBodyLimit::max(MAX_VIDEO_UPLOAD_SIZE)),
        )
        .route("/api/videos/{share_token}", get(videos::get_video))
        .route("/api/videos/{share_token}/view", post(videos::record_view))
        .route(
            "/api/videos/{share_token}/playlist.m3u8",
            get(videos::get_playlist),
        )
        .route(
            "/api/videos/{share_token}/{segment}",
            get(videos::get_segment),
        )
        .route("/api/admin/login", post(admin::login))
        .route("/api/admin/logout", post(admin::logout))
        .route("/api/admin/videos", get(admin::list_videos))
        .route("/api/admin/stats", get(admin::get_stats))
        .route(
            "/api/admin/videos/{share_token}",
            patch(admin::update_title).delete(admin::delete_video),
        )
        .with_state(state)
}

async fn health_check() -> &'static str {
    "healthy"
}
