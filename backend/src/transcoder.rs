use tokio::process::Command;

pub async fn transcode(input_path: &str, output_dir: &str) -> Result<(), anyhow::Error> {
    let playlist_path = format!("{}/playlist.m3u8", output_dir);
    let segment_pattern = format!("{}/segment_%03d.ts", output_dir);

    let status = Command::new("ffmpeg")
        .args([
            "-i",
            input_path,
            "-codec:v",
            "libx264",
            "-codec:a",
            "aac",
            "-hls_time",
            "6",
            "-hls_playlist_type",
            "vod",
            "-hls_segment_filename",
            &segment_pattern,
            &playlist_path,
        ])
        .status()
        .await?;

    if !status.success() {
        return Err(anyhow::anyhow!("FFmpeg failed"));
    }

    Ok(())
}
