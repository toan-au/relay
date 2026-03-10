FROM rust:slim AS builder

RUN apt-get update && apt-get install -y musl-tools && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src

RUN cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:3.20

RUN apk add --no-cache ca-certificates ffmpeg

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/worker /usr/local/bin/worker

CMD ["worker"]
