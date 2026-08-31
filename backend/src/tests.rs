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

    AppState {
        db,
        s3,
        bucket,
        sqs,
        queue_url,
    }
}

fn multipart_video_body(boundary: &str) -> String {
    format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"video\"; filename=\"clip.mp4\"\r\n\
         Content-Type: video/mp4\r\n\r\n\
         not-really-a-video\r\n\
         --{boundary}--\r\n"
    )
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
        .body(Body::from(multipart_video_body(boundary)))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let share_token = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(share_token.len(), 8);
    share_token
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
