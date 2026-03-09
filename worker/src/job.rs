use aws_sdk_s3::{primitives::ByteStream, Client as S3Client};
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tracing::{error, info};

use crate::config::Config;

type Error = Box<dyn std::error::Error>;

pub async fn process(config: &Config, share_token: &str, s3_key: &str) -> Result<(), Error> {
    let raw_bytes = download(&config.s3, &config.bucket, s3_key).await?;
    info!("Downloaded {} bytes for {}", raw_bytes.len(), share_token);

    let tmp_dir = tempfile::tempdir()?;
    let extension = s3_key.rsplit('.').next().unwrap_or("mp4");
    let input_path = tmp_dir.path().join(format!("input.{}", extension));
    let output_dir = tmp_dir.path().join("hls");
    tokio::fs::create_dir(&output_dir).await?;

    let mut file = tokio::fs::File::create(&input_path).await?;
    file.write_all(&raw_bytes).await?;
    file.flush().await?;
    drop(file); // close before ffmpeg reads it

    let file_size = std::fs::metadata(&input_path)?.len();
    info!("Input file written: {} bytes at {:?}", file_size, input_path);

    transcode(&input_path, &output_dir).await?;
    info!("Transcoded {}", share_token);

    upload_hls(&config.s3, &config.bucket, share_token, &output_dir).await?;
    mark_ready(&config.db, share_token).await?;
    info!("Marked {} as ready", share_token);

    config.s3.delete_object().bucket(&config.bucket).key(s3_key).send().await?;
    info!("Deleted raw file: {}", s3_key);

    Ok(())
}

async fn download(s3: &S3Client, bucket: &str, key: &str) -> Result<bytes::Bytes, Error> {
    let object = s3.get_object().bucket(bucket).key(key).send().await?;
    Ok(object.body.collect().await?.into_bytes())
}

async fn transcode(input_path: &Path, output_dir: &Path) -> Result<(), Error> {
    let playlist_path = output_dir.join("playlist.m3u8");
    let segment_pattern = output_dir.join("segment%d.ts");

    let output = tokio::process::Command::new("ffmpeg")
        .args([
            "-i",
            input_path.to_str().unwrap(),
            "-codec:v",
            "libx264",
            "-codec:a",
            "aac",
            "-hls_time",
            "6",
            "-hls_playlist_type",
            "vod",
            "-hls_segment_filename",
            segment_pattern.to_str().unwrap(),
            playlist_path.to_str().unwrap(),
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("ffmpeg stderr:\n{}", stderr);
        return Err(format!("ffmpeg exited with status: {}", output.status).into());
    }

    Ok(())
}

async fn upload_hls(
    s3: &S3Client,
    bucket: &str,
    share_token: &str,
    output_dir: &Path,
) -> Result<(), Error> {
    let mut entries = tokio::fs::read_dir(output_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let file_name = entry.file_name();
        let key = format!("{}/{}", share_token, file_name.to_str().unwrap());
        let bytes = tokio::fs::read(entry.path()).await?;

        s3.put_object()
            .bucket(bucket)
            .key(&key)
            .body(ByteStream::from(bytes))
            .send()
            .await?;

        info!("Uploaded {}", key);
    }

    Ok(())
}

async fn mark_ready(db: &sqlx::PgPool, share_token: &str) -> Result<(), Error> {
    sqlx::query("UPDATE videos SET status = 'ready' WHERE share_token = $1")
        .bind(share_token)
        .execute(db)
        .await?;
    Ok(())
}
