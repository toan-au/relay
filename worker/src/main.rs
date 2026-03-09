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
                warn!("Malformed message, skipping");
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
                    // visibility timeout expires → message is retried automatically
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
