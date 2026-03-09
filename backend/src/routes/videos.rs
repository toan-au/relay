use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tokio::io::AsyncWriteExt;

use tracing::{debug, error, info};

use crate::transcoder;
use crate::AppState;

fn log_error(e: impl std::fmt::Display) -> Response {
    error!("internal error: {}", e);
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

fn sqlx_error(e: sqlx::Error) -> Response {
    match e {
        sqlx::Error::RowNotFound => StatusCode::NOT_FOUND.into_response(),
        _ => log_error(e),
    }
}

fn s3_error(e: impl std::fmt::Display) -> Response {
    let msg = e.to_string();
    if msg.contains("NoSuchKey") {
        StatusCode::NOT_FOUND.into_response()
    } else {
        error!("s3 error: {}", msg);
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

pub async fn upload_video(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Response, Response> {
    while let Some(mut field) = multipart.next_field().await.map_err(log_error)? {
        if field.name() != Some("video") {
            continue;
        }

        // Find the extension
        let content_type = field.content_type().unwrap_or("video/mp4").to_owned();
        let extension = match content_type.as_str() {
            "video/mp4" => "mp4",
            "video/quicktime" => "mov",
            "video/x-msvideo" => "avi",
            "video/webm" => "webm",
            "video/x-matroska" => "mkv",
            _ => "mp4",
        };

        debug!("upload field: {:?} {:?}", field.file_name(), field.content_type());
        let tmp_dir = tempfile::Builder::new()
            .prefix("hotpotato-")
            .tempdir_in(".")
            .map_err(log_error)?;
        let input_path = tmp_dir.path().join(format!("input.{}", extension));

        let mut tmp_file = tokio::fs::File::create(&input_path)
            .await
            .map_err(log_error)?;

        while let Some(chunk) = field.chunk().await.map_err(log_error)? {
            tmp_file.write_all(&chunk).await.map_err(log_error)?;
        }

        let output_dir = tmp_dir.path().join("hls");

        let input_str = input_path.to_str().unwrap();
        debug!("passing to ffmpeg: {}", input_str);
        debug!("input file exists: {}", input_path.exists());
        debug!(
            "input file size: {:?}",
            std::fs::metadata(&input_path).map(|m| m.len())
        );

        tmp_file.flush().await.map_err(log_error)?;
        drop(tmp_file); // close the file before FFmpeg reads it

        tokio::fs::create_dir(&output_dir)
            .await
            .map_err(log_error)?;

        transcoder::transcode(input_path.to_str().unwrap(), output_dir.to_str().unwrap())
            .await
            .map_err(log_error)?;

        // Generate video id and upload segments to bucket
        let video_id = uuid::Uuid::new_v4().to_string();
        let mut entries = tokio::fs::read_dir(&output_dir).await.map_err(log_error)?;

        while let Some(entry) = entries.next_entry().await.map_err(log_error)? {
            let file_name = entry.file_name();
            let file_name = file_name.to_str().unwrap();
            let bytes = tokio::fs::read(entry.path()).await.map_err(log_error)?;
            let key = format!("{}/{}", video_id, file_name);
            state
                .s3
                .put_object()
                .bucket(&state.bucket)
                .key(&key)
                .body(bytes.into())
                .send()
                .await
                .map_err(log_error)?;

            info!("uploaded: {}", key);
        }

        let share_token = &video_id[..8];

        sqlx::query!(
            "INSERT INTO videos (id, share_token, status) VALUES ($1, $2, $3)",
            uuid::Uuid::parse_str(&video_id).unwrap(),
            share_token,
            "ready"
        )
        .execute(&state.db)
        .await
        .map_err(log_error)?;

        info!("video inserted: {}", share_token);
        return Ok(share_token.to_string().into_response());
    }

    Ok(StatusCode::OK.into_response())
}

#[derive(sqlx::FromRow)]
struct VideoRow {
    id: uuid::Uuid,
    status: String,
}

pub async fn get_video(
    State(state): State<AppState>,
    Path(share_token): Path<String>,
) -> Result<Response, Response> {
    let video =
        sqlx::query_as::<_, VideoRow>("SELECT id, status FROM videos WHERE share_token = $1")
            .bind(share_token)
            .fetch_one(&state.db)
            .await
            .map_err(sqlx_error)?;

    Ok(axum::Json(serde_json::json!({
        "status": video.status
    }))
    .into_response())
}

pub async fn get_playlist(
    State(state): State<AppState>,
    Path(share_token): Path<String>,
) -> Result<Response, Response> {
    let video =
        sqlx::query_as::<_, VideoRow>("SELECT id, status FROM videos WHERE share_token = $1")
            .bind(share_token)
            .fetch_one(&state.db)
            .await
            .map_err(sqlx_error)?;

    let key = format!("{}/playlist.m3u8", video.id);
    debug!("fetching key: {}", key);

    let object = state
        .s3
        .get_object()
        .bucket(&state.bucket)
        .key(&key)
        .send()
        .await
        .map_err(s3_error)?;

    let bytes = object.body.collect().await.map_err(log_error)?.into_bytes();

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
) -> Result<Response, Response> {
    let video =
        sqlx::query_as::<_, VideoRow>("SELECT id, status FROM videos WHERE share_token = $1")
            .bind(&share_token)
            .fetch_one(&state.db)
            .await
            .map_err(sqlx_error)?;

    let key = format!("{}/{}", video.id, segment);

    let object = state
        .s3
        .get_object()
        .bucket(&state.bucket)
        .key(&key)
        .send()
        .await
        .map_err(s3_error)?;

    let bytes = object.body.collect().await.map_err(log_error)?.into_bytes();

    Ok(([(axum::http::header::CONTENT_TYPE, "video/mp2t")], bytes).into_response())
}
