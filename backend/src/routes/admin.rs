use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    CookieJar,
};

use crate::auth::{AdminAuth, SESSION_COOKIE};
use crate::errors::AppError;
use crate::models::admin_session::AdminSession;
use crate::state::AppState;

#[derive(serde::Deserialize)]
pub struct LoginRequest {
    password: String,
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> Result<(CookieJar, StatusCode), AppError> {
    if !constant_time_eq(body.password.as_bytes(), state.admin_password.as_bytes()) {
        return Err(AppError::Unauthorized);
    }

    let token = AdminSession::create(&state.db).await?;

    let cookie = Cookie::build((SESSION_COOKIE, token))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::hours(24))
        .build();

    Ok((jar.add(cookie), StatusCode::OK))
}

pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), AppError> {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        AdminSession::delete(&state.db, cookie.value()).await?;
    }
    Ok((jar.remove(SESSION_COOKIE), StatusCode::OK))
}

pub async fn list_videos(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> Result<Response, AppError> {
    let videos = crate::models::video::VideoRow::list_all(&state.db).await?;
    Ok(Json(videos).into_response())
}

pub async fn get_stats(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> Result<Response, AppError> {
    let stats = crate::models::video::VideoRow::stats(&state.db).await?;
    Ok(Json(stats).into_response())
}

#[derive(serde::Deserialize)]
pub struct UpdateTitleRequest {
    title: String,
}

pub async fn update_title(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(share_token): Path<String>,
    Json(body): Json<UpdateTitleRequest>,
) -> Result<Response, AppError> {
    let title = body.title.trim();
    if title.is_empty() {
        return Err(AppError::BadRequest("Title is required".into()));
    }

    let updated =
        crate::models::video::VideoRow::update_title(&state.db, &share_token, title).await?;
    if !updated {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::OK.into_response())
}

pub async fn delete_video(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(share_token): Path<String>,
) -> Result<Response, AppError> {
    // The raw upload lives under raw/{token}/ until transcoding succeeds
    // or fails (both paths delete it); the HLS output lives directly
    // under {token}/. A video can have objects under either, or both,
    // depending on what stage it's at - clean up both prefixes.
    for prefix in [format!("raw/{share_token}/"), format!("{share_token}/")] {
        delete_s3_prefix(&state, &prefix).await?;
    }

    let deleted = crate::models::video::VideoRow::delete(&state.db, &share_token).await?;
    if !deleted {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn delete_s3_prefix(state: &AppState, prefix: &str) -> Result<(), AppError> {
    let listed = state
        .s3
        .list_objects_v2()
        .bucket(&state.bucket)
        .prefix(prefix)
        .send()
        .await
        .map_err(AppError::internal)?;

    let ids: Vec<_> = listed
        .contents()
        .iter()
        .filter_map(|o| o.key())
        .filter_map(|key| {
            aws_sdk_s3::types::ObjectIdentifier::builder()
                .key(key)
                .build()
                .ok()
        })
        .collect();

    if ids.is_empty() {
        return Ok(());
    }

    let delete = aws_sdk_s3::types::Delete::builder()
        .set_objects(Some(ids))
        .build()
        .map_err(AppError::internal)?;

    state
        .s3
        .delete_objects()
        .bucket(&state.bucket)
        .delete(delete)
        .send()
        .await
        .map_err(AppError::internal)?;

    Ok(())
}
