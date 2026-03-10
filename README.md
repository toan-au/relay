# Relay

Anonymous video sharing. Upload a video, get a shareable link instantly.

## Stack

- **Frontend** — Svelte 5 + TypeScript, served by nginx
- **Backend** — Rust / Axum
- **Worker** — Rust, transcodes video to HLS via ffmpeg
- **Queue** — SQS (ElasticMQ locally)
- **Storage** — S3 (MinIO locally)
- **Database** — PostgreSQL

## How it works

1. User uploads a video → backend streams it to S3 and returns a share token immediately
2. Backend enqueues a transcoding job to SQS
3. Worker picks up the job, transcodes to HLS with ffmpeg, uploads segments to S3, marks video as ready
4. Viewer polls until ready, then streams via hls.js

## Getting started

### Containerized (recommended)

Requires: Docker

```bash
cp .env.example .env
# fill in passwords for POSTGRES_PASSWORD and MINIO_ROOT_PASSWORD
docker compose up
```

Open `http://localhost`.

### Local development

Requires: Rust, Node.js, ffmpeg

Spin up just the infrastructure:

```bash
docker compose up postgres minio elasticmq
```

Set up local env files:

generate `S3_ACCESS_KEY_ID` and `S3_SECRET_ACCESS_KEY` at http://localhost:9001 after starting the infra

```bash
cp backend/.env.example backend/.env
cp worker/.env.example  worker/.env
```

Then in separate terminals:

```bash
cd backend  && cargo run
cd worker   && cargo run
cd frontend && npm install && npm run dev
```

## Root Environment Variables

| Variable              | Description                                       |
| --------------------- | ------------------------------------------------- |
| `POSTGRES_DB`         | Database name                                     |
| `POSTGRES_USER`       | Database user                                     |
| `POSTGRES_PASSWORD`   | Database password                                 |
| `MINIO_ROOT_USER`     | MinIO admin user (also used as S3 access key)     |
| `MINIO_ROOT_PASSWORD` | MinIO admin password (also used as S3 secret key) |
| `S3_ENDPOINT`         | S3-compatible endpoint                            |
| `S3_REGION`           | S3 region                                         |
| `S3_BUCKET_NAME`      | Bucket for video storage                          |
| `SQS_ENDPOINT`          | SQS-compatible endpoint                                        |
| `SQS_QUEUE_URL`         | Full URL of the transcoding queue                              |
| `SQS_ACCESS_KEY_ID`     | SQS access key (any value works locally, required for AWS SQS) |
| `SQS_SECRET_ACCESS_KEY` | SQS secret key (any value works locally, required for AWS SQS) |
