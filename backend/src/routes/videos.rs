use axum::{
    extract::{Multipart, Path, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};

use tracing::{debug, info};

use crate::errors::AppError;
use crate::models::video::VideoRow;
use crate::state::AppState;

pub async fn upload_video(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    if let Some(content_length) = headers.get(axum::http::header::CONTENT_LENGTH) {
        let len = content_length
            .to_str()
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        if len > super::MAX_VIDEO_UPLOAD_SIZE {
            return Err(AppError::BadRequest("File exceeds 1GB limit".into()));
        }
    }

    while let Some(mut field) = multipart.next_field().await.map_err(AppError::internal)? {
        if field.name() != Some("video") {
            continue;
        }

        let content_type = field.content_type().unwrap_or("").to_owned();
        let extension = match content_type.as_str() {
            "video/mp4" => "mp4",
            "video/quicktime" => "mov",
            "video/x-msvideo" => "avi",
            "video/webm" => "webm",
            "video/x-matroska" => "mkv",
            _ => return Err(AppError::BadRequest(format!("Unsupported video format: {}", content_type))),
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
        let share_token = uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_string();
        let s3_key = format!("raw/{}/input.{}", share_token, extension);

        VideoRow::insert(&state.db, video_id, &share_token).await?;
        info!("video inserted: {}", share_token);

        // Return immediately — S3 upload and SQS enqueue happen in the background
        let state = state.clone();
        let token = share_token.clone();
        tokio::spawn(async move {
            let failed = upload_and_enqueue(state.clone(), token.clone(), s3_key, buffer)
                .await
                .map_err(|e| e.to_string());
            if let Err(e) = failed {
                tracing::error!("background upload failed for {}: {}", token, e);
                let _ = VideoRow::update_status(&state.db, &token, "failed").await;
            }
        });

        return Ok(share_token.into_response());
    }

    Ok(axum::http::StatusCode::OK.into_response())
}

async fn upload_and_enqueue(
    state: crate::state::AppState,
    share_token: String,
    s3_key: String,
    buffer: Vec<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    state
        .s3
        .put_object()
        .bucket(&state.bucket)
        .key(&s3_key)
        .body(buffer.into())
        .send()
        .await?;

    info!("uploaded raw video to S3: {}", s3_key);

    VideoRow::update_status(&state.db, &share_token, "processing").await?;

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
        .await?;

    info!("SQS job enqueued for: {}", share_token);

    Ok(())
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
        [
            (axum::http::header::CONTENT_TYPE, "application/vnd.apple.mpegurl"),
            (axum::http::header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
        ],
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
