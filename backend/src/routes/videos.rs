use axum::{
    body::Bytes,
    extract::Multipart,
    http::StatusCode,
    response::{IntoResponse, Response},
};

struct File {
    file_name: String,
    content_type: String,
    bytes: Bytes,
}

pub async fn upload_video(mut multipart: Multipart) -> Result<Response, Response> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
    {
        if let Some("video") = field.name() {
            println!("{:?} {:?}", field.file_name(), field.content_type());
        }

        let file_name = field.file_name().unwrap_or_default().to_owned();
        let content_type = field.content_type().unwrap_or_default().to_owned();

        let bytes = field
            .bytes()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    }

    Ok(StatusCode::OK.into_response())
}
