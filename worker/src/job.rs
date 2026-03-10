use aws_sdk_s3::{primitives::ByteStream, Client as S3Client};
use std::collections::HashSet;
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, Duration};
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
    drop(file);

    let file_size = std::fs::metadata(&input_path)?.len();
    info!("Input file written: {} bytes at {:?}", file_size, input_path);

    transcode_progressive(&config.s3, &config.bucket, &config.db, share_token, &input_path, &output_dir).await?;

    config.s3.delete_object().bucket(&config.bucket).key(s3_key).send().await?;
    info!("Deleted raw file: {}", s3_key);

    Ok(())
}

async fn download(s3: &S3Client, bucket: &str, key: &str) -> Result<bytes::Bytes, Error> {
    let object = s3.get_object().bucket(bucket).key(key).send().await?;
    Ok(object.body.collect().await?.into_bytes())
}

async fn transcode_progressive(
    s3: &S3Client,
    bucket: &str,
    db: &sqlx::PgPool,
    share_token: &str,
    input_path: &Path,
    output_dir: &Path,
) -> Result<(), Error> {
    let playlist_path = output_dir.join("playlist.m3u8");
    let segment_pattern = output_dir.join("segment%d.ts");

    let mut child = tokio::process::Command::new("ffmpeg")
        .args([
            "-i", input_path.to_str().unwrap(),
            "-codec:v", "libx264",
            "-codec:a", "aac",
            "-hls_time", "6",
            "-hls_playlist_type", "event",
            "-hls_segment_filename", segment_pattern.to_str().unwrap(),
            playlist_path.to_str().unwrap(),
        ])
        .stderr(std::process::Stdio::inherit())
        .spawn()?;

    let mut uploaded: HashSet<String> = HashSet::new();
    let mut marked_ready = false;

    loop {
        let mut entries = tokio::fs::read_dir(output_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".ts") || uploaded.contains(&name) {
                continue;
            }

            let key = format!("{}/{}", share_token, name);
            let bytes = tokio::fs::read(entry.path()).await?;
            upload(s3, bucket, &key, bytes).await?;
            info!("Uploaded segment {}", key);
            uploaded.insert(name);

            // Upload the playlist after each new segment so the player can advance
            if playlist_path.exists() {
                let bytes = tokio::fs::read(&playlist_path).await?;
                upload(s3, bucket, &format!("{}/playlist.m3u8", share_token), bytes).await?;
            }

            // Mark ready after the first segment — player can start immediately
            if !marked_ready {
                mark_ready(db, share_token).await?;
                info!("Marked {} as ready (first segment available)", share_token);
                marked_ready = true;
            }
        }

        match child.try_wait()? {
            Some(status) if status.success() => break,
            Some(status) => {
                return Err(format!("ffmpeg exited with status: {}", status).into());
            }
            None => sleep(Duration::from_secs(1)).await,
        }
    }

    // Final playlist upload — now contains #EXT-X-ENDLIST
    let bytes = tokio::fs::read(&playlist_path).await?;
    upload(s3, bucket, &format!("{}/playlist.m3u8", share_token), bytes).await?;
    info!("Transcoding complete for {}", share_token);

    Ok(())
}

async fn upload(s3: &S3Client, bucket: &str, key: &str, bytes: Vec<u8>) -> Result<(), Error> {
    s3.put_object()
        .bucket(bucket)
        .key(key)
        .body(ByteStream::from(bytes))
        .send()
        .await?;
    Ok(())
}

async fn mark_ready(db: &sqlx::PgPool, share_token: &str) -> Result<(), Error> {
    sqlx::query("UPDATE videos SET status = 'ready' WHERE share_token = $1")
        .bind(share_token)
        .execute(db)
        .await?;
    Ok(())
}
