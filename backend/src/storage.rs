use aws_sdk_s3::{
    config::{Credentials, Region},
    Client,
};

pub async fn create_s3_client() -> Client {
    let endpoint = std::env::var("S3_ENDPOINT").expect("S3_ENDPOINT must be set");
    let access_key_id = std::env::var("S3_ACCESS_KEY_ID").expect("S3_ACCESS_KEY_ID must be set");
    let secret_access_key =
        std::env::var("S3_SECRET_ACCESS_KEY").expect("S3_SECRET_ACCESS_KEY must be set");
    let region =
        Region::new(std::env::var("S3_REGION").unwrap_or_else(|_| "ap-northeast-1".to_string()));

    let credentials = Credentials::new(access_key_id, secret_access_key, None, None, "static");

    let config = aws_sdk_s3::config::Builder::new()
        .endpoint_url(endpoint)
        .credentials_provider(credentials)
        .region(region)
        .force_path_style(true)
        .build();

    Client::from_conf(config)
}
