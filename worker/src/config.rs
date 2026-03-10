use aws_sdk_s3::{
    config::{Credentials as S3Credentials, Region as S3Region},
    Client as S3Client,
};
use aws_sdk_sqs::{
    config::{Credentials as SqsCredentials, Region as SqsRegion},
    Client as SqsClient,
};

pub struct Config {
    pub s3: S3Client,
    pub bucket: String,
    pub sqs: SqsClient,
    pub queue_url: String,
    pub db: sqlx::PgPool,
}

pub async fn init() -> Config {
    let access_key_id = std::env::var("S3_ACCESS_KEY_ID").expect("S3_ACCESS_KEY_ID must be set");
    let secret_access_key =
        std::env::var("S3_SECRET_ACCESS_KEY").expect("S3_SECRET_ACCESS_KEY must be set");
    let region = std::env::var("S3_REGION").unwrap_or_else(|_| "ap-northeast-1".to_string());

    let s3 = create_s3_client(&access_key_id, &secret_access_key, &region);
    let sqs = create_sqs_client(&access_key_id, &secret_access_key, &region);

    let db =
        sqlx::PgPool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"))
            .await
            .expect("Unable to connect to DB");

    Config {
        s3,
        bucket: std::env::var("S3_BUCKET_NAME").expect("S3_BUCKET_NAME must be set"),
        sqs,
        queue_url: std::env::var("SQS_QUEUE_URL").expect("SQS_QUEUE_URL must be set"),
        db,
    }
}

fn create_s3_client(access_key_id: &str, secret_access_key: &str, region: &str) -> S3Client {
    let credentials = S3Credentials::new(access_key_id, secret_access_key, None, None, "static");
    let config = aws_sdk_s3::config::Builder::new()
        .endpoint_url(std::env::var("S3_ENDPOINT").expect("S3_ENDPOINT must be set"))
        .credentials_provider(credentials)
        .region(S3Region::new(region.to_string()))
        .force_path_style(true)
        .build();
    S3Client::from_conf(config)
}

fn create_sqs_client(access_key_id: &str, secret_access_key: &str, region: &str) -> SqsClient {
    let credentials = SqsCredentials::new(access_key_id, secret_access_key, None, None, "static");
    let config = aws_sdk_sqs::config::Builder::new()
        .endpoint_url(std::env::var("SQS_ENDPOINT").expect("SQS_ENDPOINT must be set"))
        .credentials_provider(credentials)
        .region(SqsRegion::new(region.to_string()))
        .build();
    SqsClient::from_conf(config)
}
