mod config;
mod job;

use tracing::{error, info, warn};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    #[cfg(debug_assertions)]
    dotenvy::dotenv().ok();

    let config = config::init().await;

    info!("Worker started, polling for jobs...");

    loop {
        let output = match config
            .sqs
            .receive_message()
            .queue_url(&config.queue_url)
            .max_number_of_messages(1)
            .wait_time_seconds(20)
            .send()
            .await
        {
            Ok(o) => o,
            Err(e) => {
                error!("SQS receive error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        for message in output.messages() {
            let Some(receipt_handle) = message.receipt_handle() else {
                warn!("Message missing receipt handle, skipping");
                continue;
            };

            let Some((share_token, s3_key)) = parse_message(message.body()) else {
                warn!("Malformed message, deleting");
                let _ = config
                    .sqs
                    .delete_message()
                    .queue_url(&config.queue_url)
                    .receipt_handle(receipt_handle)
                    .send()
                    .await;
                continue;
            };

            info!("Processing job: {}", share_token);

            match job::process(&config, &share_token, &s3_key).await {
                Ok(_) => {
                    info!("Job complete: {}", share_token);
                    let _ = config
                        .sqs
                        .delete_message()
                        .queue_url(&config.queue_url)
                        .receipt_handle(receipt_handle)
                        .send()
                        .await;
                }
                Err(e) => {
                    error!("Job failed for {}: {}", share_token, e);
                    let _ = job::mark_error(&config.db, &share_token).await;
                    let _ = config
                        .s3
                        .delete_object()
                        .bucket(&config.bucket)
                        .key(&s3_key)
                        .send()
                        .await;
                    let _ = config
                        .sqs
                        .delete_message()
                        .queue_url(&config.queue_url)
                        .receipt_handle(receipt_handle)
                        .send()
                        .await;
                }
            }
        }
    }
}

fn parse_message(body: Option<&str>) -> Option<(String, String)> {
    let payload: serde_json::Value = serde_json::from_str(body?).ok()?;
    let share_token = payload["share_token"].as_str()?.to_string();
    let s3_key = payload["s3_key"].as_str()?.to_string();
    Some((share_token, s3_key))
}

#[cfg(test)]
mod tests {
    use super::parse_message;

    #[test]
    fn parses_share_token_and_s3_key() {
        let body = r#"{"share_token": "abc12345", "s3_key": "raw/abc12345/input.mp4"}"#;
        let (share_token, s3_key) = parse_message(Some(body)).unwrap();
        assert_eq!(share_token, "abc12345");
        assert_eq!(s3_key, "raw/abc12345/input.mp4");
    }

    #[test]
    fn returns_none_for_missing_body() {
        assert!(parse_message(None).is_none());
    }

    #[test]
    fn returns_none_for_invalid_json() {
        assert!(parse_message(Some("not json")).is_none());
    }

    #[test]
    fn returns_none_when_fields_missing() {
        assert!(parse_message(Some(r#"{"share_token": "abc12345"}"#)).is_none());
        assert!(parse_message(Some(r#"{"s3_key": "raw/abc12345/input.mp4"}"#)).is_none());
    }
}
