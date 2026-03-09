use axum::{
    extract::{Multipart, Path, State},
    response::{IntoResponse, Response},
};

use tracing::{debug, info};

use crate::errors::AppError;
use crate::models::video::VideoRow;
use crate::state::AppState;

pub async fn upload_video(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    while let Some(mut field) = multipart.next_field().await.map_err(AppError::internal)? {
        if field.name() != Some("video") {
            continue;
        }

        let content_type = field.content_type().unwrap_or("video/mp4").to_owned();
        let extension = match content_type.as_str() {
            "video/mp4" => "mp4",
            "video/quicktime" => "mov",
            "video/x-msvideo" => "avi",
            "video/webm" => "webm",
            "video/x-matroska" => "mkv",
            _ => "mp4",
        };

        debug!(
            "upload field: {:?} {:?}",
            field.file_name(),
            field.content_type()
        );

        let mut buffer: Vec<u8> = Vec::new();
        while let Some(chunk) = field.chunk().await.map_err(AppError::internal)? {
            buffer.extend_from_slice(&chunk);
        }

        let video_id = uuid::Uuid::new_v4();
        let share_token = &uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_string();

        let s3_key = format!("raw/{}/input.{}", share_token, extension);

        state
            .s3
            .put_object()
            .bucket(&state.bucket)
            .key(&s3_key)
            .body(buffer.into())
            .send()
            .await
            .map_err(AppError::from_s3)?;

        info!("uploaded raw video to S3: {}", s3_key);

        VideoRow::insert(&state.db, video_id, share_token).await?;

        info!("video inserted: {}", share_token);

        let message = serde_json::json!({
            "share_token": share_token,
            "s3_key": s3_key,
        });

        state
            .sqs
            .send_message()
            .queue_url(&state.queue_url)
            .message_body(message.to_string())
            .send()
            .await
            .map_err(AppError::internal)?;

        info!("SQS job enqueued for: {}", share_token);

        return Ok(share_token.to_string().into_response());
    }

    Ok(axum::http::StatusCode::OK.into_response())
}

pub async fn get_video(
    State(state): State<AppState>,
    Path(share_token): Path<String>,
) -> Result<Response, AppError> {
    let video = VideoRow::fetch_by_token(&state.db, &share_token).await?;

    Ok(axum::Json(serde_json::json!({
        "status": video.status
    }))
    .into_response())
}

pub async fn get_playlist(
    State(state): State<AppState>,
    Path(share_token): Path<String>,
) -> Result<Response, AppError> {
    VideoRow::fetch_by_token(&state.db, &share_token).await?;

    let key = format!("{}/playlist.m3u8", share_token);
    debug!("fetching key: {}", key);

    let object = state
        .s3
        .get_object()
        .bucket(&state.bucket)
        .key(&key)
        .send()
        .await
        .map_err(AppError::from_s3)?;

    let bytes = object
        .body
        .collect()
        .await
        .map_err(AppError::internal)?
        .into_bytes();

    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            "application/vnd.apple.mpegurl",
        )],
        bytes,
    )
        .into_response())
}

pub async fn get_segment(
    State(state): State<AppState>,
    Path((share_token, segment)): Path<(String, String)>,
) -> Result<Response, AppError> {
    VideoRow::fetch_by_token(&state.db, &share_token).await?;

    let key = format!("{}/{}", share_token, segment);

    let object = state
        .s3
        .get_object()
        .bucket(&state.bucket)
        .key(&key)
        .send()
        .await
        .map_err(AppError::from_s3)?;

    let bytes = object
        .body
        .collect()
        .await
        .map_err(AppError::internal)?
        .into_bytes();

    Ok(([(axum::http::header::CONTENT_TYPE, "video/mp2t")], bytes).into_response())
}
