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
3. Worker picks up the job, transcodes to HLS with ffmpeg, uploads segments to S3, marks video as ready (or as errored, and cleans up, if transcoding fails)
4. Viewer polls until ready, then streams via hls.js

## Getting started

### Containerized (recommended)

Requires: Docker

```bash
cp .env.example .env
# fill in passwords for POSTGRES_PASSWORD and MINIO_ROOT_PASSWORD
docker compose up
```

Open `http://127.0.0.1`. not localhost

### Local development

Requires: Rust, Node.js, pnpm, ffmpeg

Spin up just the infrastructure:

```bash
docker compose up postgres minio elasticmq
```

Set up local env files:

generate `S3_ACCESS_KEY_ID` and `S3_SECRET_ACCESS_KEY` at http://localhost:9001 after starting the infra

`backend` and `worker` need identical values here, so `worker/.env` is a symlink to `backend/.env` rather than a second copy - keeps the two from drifting out of sync:

```bash
cp backend/.env.example backend/.env
ln -s ../backend/.env worker/.env
```

Then in separate terminals:

```bash
cd backend  && cargo run
cd worker   && cargo run
cd frontend && pnpm install && pnpm run dev
```

## Root Environment Variables

| Variable                | Description                                                    |
| ----------------------- | -------------------------------------------------------------- |
| `POSTGRES_DB`           | Database name                                                  |
| `POSTGRES_USER`         | Database user                                                  |
| `POSTGRES_PASSWORD`     | Database password                                              |
| `MINIO_ROOT_USER`       | MinIO admin user (also used as S3 access key)                  |
| `MINIO_ROOT_PASSWORD`   | MinIO admin password (also used as S3 secret key)              |
| `S3_ENDPOINT`           | S3-compatible endpoint                                         |
| `S3_REGION`             | S3 region                                                      |
| `S3_BUCKET_NAME`        | Bucket for video storage                                       |
| `SQS_ENDPOINT`          | SQS-compatible endpoint                                        |
| `SQS_QUEUE_URL`         | Full URL of the transcoding queue                              |
| `SQS_ACCESS_KEY_ID`     | SQS access key (any value works locally, required for AWS SQS) |
| `SQS_SECRET_ACCESS_KEY` | SQS secret key (any value works locally, required for AWS SQS) |
| `ADMIN_PASSWORD`        | Shared password for the `/admin` dashboard                     |

## Admin dashboard

`/admin` — log in with `ADMIN_PASSWORD` to view all videos (title, status, view count, upload date), edit titles, delete videos, and see aggregate stats. Single shared password, no per-admin accounts — login sets a session cookie backed by an `admin_sessions` table (24h expiry); logging out or letting it expire is the only way to revoke it.

## Testing

```bash
cd worker && cargo test    # pure unit tests, no infra needed
```

Backend tests hit a real Postgres/MinIO/ElasticMQ, so start the infra first (same as local dev):

```bash
docker compose up postgres minio elasticmq
cd backend && cargo test
```

## CI/CD

`.github/workflows/ci-cd.yml` runs on every push/PR:

1. **Checks** — `cargo fmt` + `clippy` + `build` + `test` for `backend` and `worker`, `svelte-check` + `vite build` for `frontend`.
2. **Build & push** (push to `main` only) — builds `backend`, `worker`, `frontend`, `elasticmq` images and pushes them to GHCR, tagged with the commit SHA.
3. **Deploy** (push to `main` only, skipped if deploy secrets aren't set) — copies `docker-compose.prod.yaml` to the target host over SSH and runs `docker compose pull && up -d`.

Deploying to a host is optional — the pipeline builds and tests happily with none of the secrets below configured; the deploy job just no-ops until they're added.

### Deploy secrets (GitHub repo → Settings → Secrets and variables → Actions)

| Secret            | Description                                                                             |
| ------------------ | ---------------------------------------------------------------------------------------- |
| `PROD_ENV_FILE`    | The full contents of a production `.env` (same shape as `.env.example`, real values)     |
| `DEPLOY_HOST`      | SSH host/IP of the target server (e.g. a DigitalOcean droplet)                           |
| `DEPLOY_USER`      | SSH user on the target server                                                            |
| `DEPLOY_SSH_KEY`   | Private SSH key for that user                                                            |
| `DEPLOY_PORT`      | SSH port (optional, defaults to 22)                                                      |

The target server needs Docker + the Compose plugin installed, and a `~/relay` directory for the pipeline to write to. Adminer and the MinIO console are bound to `127.0.0.1` only in production (`docker-compose.prod.yaml`) — reach them over an SSH tunnel, not directly over the internet.
