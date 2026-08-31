#![cfg(test)]

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use tower::ServiceExt;

use crate::routes;
use crate::state::AppState;
use crate::{queue, storage};

async fn test_state() -> AppState {
    dotenvy::dotenv().ok();

    let db = sqlx::PgPool::connect(
        &std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests"),
    )
    .await
    .expect("failed to connect to test database");
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("failed to run migrations");

    let s3 = storage::create_s3_client().await;
    let bucket = std::env::var("S3_BUCKET_NAME").expect("S3_BUCKET_NAME must be set for tests");
    // Tests run concurrently and can race to create the bucket - a
    // "someone already made it" response is success, not failure.
    if let Err(err) = s3.create_bucket().bucket(&bucket).send().await {
        let already_exists = err
            .as_service_error()
            .is_some_and(|e| e.is_bucket_already_owned_by_you() || e.is_bucket_already_exists());
        if !already_exists {
            panic!("failed to create test bucket: {err}");
        }
    }

    let (sqs, queue_url) = queue::create_sqs_client().await;

    let admin_password =
        std::env::var("ADMIN_PASSWORD").expect("ADMIN_PASSWORD must be set for tests");

    AppState {
        db,
        s3,
        bucket,
        sqs,
        queue_url,
        admin_password,
    }
}

/// A multipart body with a video field but no title - used to test
/// that a missing title is rejected.
fn multipart_video_body(boundary: &str) -> String {
    format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"video\"; filename=\"clip.mp4\"\r\n\
         Content-Type: video/mp4\r\n\r\n\
         not-really-a-video\r\n\
         --{boundary}--\r\n"
    )
}

/// A valid multipart body: title + video.
fn multipart_video_with_title_body(boundary: &str, title: &str) -> String {
    format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"title\"\r\n\r\n\
         {title}\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"video\"; filename=\"clip.mp4\"\r\n\
         Content-Type: video/mp4\r\n\r\n\
         not-really-a-video\r\n\
         --{boundary}--\r\n"
    )
}

#[tokio::test]
async fn admin_login_with_wrong_password_is_rejected() {
    let app = routes::router(test_state().await);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"password":"wrong"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_login_with_correct_password_grants_access_to_protected_routes() {
    let state = test_state().await;
    let password = state.admin_password.clone();
    let app = routes::router(state);

    let login_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "password": password }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_res.status(), StatusCode::OK);

    let cookie = login_res
        .headers()
        .get(axum::http::header::SET_COOKIE)
        .expect("login should set a session cookie")
        .to_str()
        .unwrap()
        .to_string();

    // Without the cookie, the protected route is rejected.
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/admin/videos")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // With the session cookie, it succeeds.
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/videos")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

async fn admin_login(app: &Router, state: &AppState) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "password": state.admin_password }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    res.headers()
        .get(axum::http::header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn admin_stats_reflects_uploaded_videos() {
    let state = test_state().await;
    let app = routes::router(state.clone());
    let cookie = admin_login(&app, &state).await;

    upload(&app).await;
    upload(&app).await;

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/stats")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["total_videos"].as_i64().unwrap() >= 2);
}

#[tokio::test]
async fn admin_can_update_a_videos_title() {
    let state = test_state().await;
    let app = routes::router(state.clone());
    let cookie = admin_login(&app, &state).await;
    let share_token = upload(&app).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/admin/videos/{share_token}"))
                .header(axum::http::header::COOKIE, &cookie)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"Renamed"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/videos/{share_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["title"].as_str().unwrap(), "Renamed");
}

#[tokio::test]
async fn admin_updating_title_of_unknown_video_returns_404() {
    let state = test_state().await;
    let app = routes::router(state.clone());
    let cookie = admin_login(&app, &state).await;

    let res = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/admin/videos/doesnotexist")
                .header(axum::http::header::COOKIE, cookie)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"Renamed"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn admin_can_delete_a_video() {
    let state = test_state().await;
    let app = routes::router(state.clone());
    let cookie = admin_login(&app, &state).await;
    let share_token = upload(&app).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/admin/videos/{share_token}"))
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/videos/{share_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn admin_deleting_unknown_video_returns_404() {
    let state = test_state().await;
    let app = routes::router(state.clone());
    let cookie = admin_login(&app, &state).await;

    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/admin/videos/doesnotexist")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn admin_logout_invalidates_the_session() {
    let state = test_state().await;
    let app = routes::router(state.clone());
    let cookie = admin_login(&app, &state).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/logout")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/videos")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn healthcheck_returns_ok() {
    let app = routes::router(test_state().await);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/healthcheck")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn unknown_share_token_returns_404() {
    let app = routes::router(test_state().await);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/videos/doesnotexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// Uploads a test video and returns its share token.
async fn upload(app: &Router) -> String {
    let boundary = "HotPotatoTestBoundary";
    let req = Request::builder()
        .method("POST")
        .uri("/api/videos")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(multipart_video_with_title_body(
            boundary,
            "Test video",
        )))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let share_token = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(share_token.len(), 8);
    share_token
}

#[tokio::test]
async fn uploaded_title_is_returned_by_status_endpoint() {
    let app = routes::router(test_state().await);

    let boundary = "HotPotatoTestBoundary";
    let req = Request::builder()
        .method("POST")
        .uri("/api/videos")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(multipart_video_with_title_body(
            boundary,
            "My cool video",
        )))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let share_token = String::from_utf8(body.to_vec()).unwrap();

    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/videos/{share_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["title"].as_str().unwrap(), "My cool video");
}

#[tokio::test]
async fn upload_without_title_is_rejected() {
    let app = routes::router(test_state().await);

    let boundary = "HotPotatoTestBoundary";
    let req = Request::builder()
        .method("POST")
        .uri("/api/videos")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(multipart_video_body(boundary)))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upload_reaches_processing() {
    let app = routes::router(test_state().await);
    let share_token = upload(&app).await;

    // The S3 upload + SQS enqueue happen in a background task, so poll
    // for the status to move past the initial "uploading" write.
    let mut status = "uploading".to_string();
    for _ in 0..50 {
        if status != "uploading" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/videos/{share_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        status = json["status"].as_str().unwrap().to_string();
    }

    assert_eq!(status, "processing");
}

#[tokio::test]
async fn upload_with_blank_title_is_rejected() {
    let app = routes::router(test_state().await);

    let boundary = "HotPotatoTestBoundary";
    let req = Request::builder()
        .method("POST")
        .uri("/api/videos")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(multipart_video_with_title_body(boundary, "   ")))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upload_rejects_unsupported_content_type() {
    let app = routes::router(test_state().await);

    let boundary = "HotPotatoTestBoundary";
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"video\"; filename=\"notes.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         hello\r\n\
         --{boundary}--\r\n"
    );

    let req = Request::builder()
        .method("POST")
        .uri("/api/videos")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

async fn view_count(app: &Router, share_token: &str) -> i64 {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/videos/{share_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    json["view_count"].as_i64().unwrap()
}

#[tokio::test]
async fn recording_a_view_increments_and_persists() {
    let app = routes::router(test_state().await);
    let share_token = upload(&app).await;

    assert_eq!(view_count(&app, &share_token).await, 0);

    for expected in 1..=2 {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/videos/{share_token}/view"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["view_count"].as_i64().unwrap(), expected);
        assert_eq!(view_count(&app, &share_token).await, expected);
    }
}

#[tokio::test]
async fn playlist_for_missing_s3_object_returns_404_not_500() {
    let app = routes::router(test_state().await);
    // A row exists, but no worker has run yet, so the S3 object doesn't.
    let share_token = upload(&app).await;

    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/videos/{share_token}/playlist.m3u8"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn recording_a_view_for_unknown_token_returns_404() {
    let app = routes::router(test_state().await);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/videos/doesnotexist/view")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
