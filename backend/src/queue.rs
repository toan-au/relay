use aws_sdk_sqs::{
    config::{Credentials, Region},
    Client,
};

pub async fn create_sqs_client() -> (Client, String) {
    let endpoint = std::env::var("SQS_ENDPOINT").expect("SQS_ENDPOINT must be set");
    let access_key_id = std::env::var("S3_ACCESS_KEY_ID").expect("S3_ACCESS_KEY_ID must be set");
    let secret_access_key =
        std::env::var("S3_SECRET_ACCESS_KEY").expect("S3_SECRET_ACCESS_KEY must be set");
    let region =
        Region::new(std::env::var("S3_REGION").unwrap_or_else(|_| "ap-northeast-1".to_string()));

    let credentials = Credentials::new(access_key_id, secret_access_key, None, None, "static");

    let config = aws_sdk_sqs::config::Builder::new()
        .endpoint_url(&endpoint)
        .credentials_provider(credentials)
        .region(region)
        .build();

    let queue_url = std::env::var("SQS_QUEUE_URL").expect("SQS_QUEUE_URL must be set");

    (Client::from_conf(config), queue_url)
}
